use crate::database::submission::{SubmissionId, TestingResult};
use crate::database::test::TestId;
use crate::database::Database;
use crate::tester::execute_test;
use anyhow::Result;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio::sync::mpsc::{Receiver, Sender};

const BUFFER_SIZE: usize = 255;

async fn worker_do_test(database: &Database, submission_id: SubmissionId, test_id: TestId, executable: &Path, worker_id: i32) -> Result<()> {
    database.set_test_result(submission_id, test_id, TestingResult::Testing).await?;

    let (input, expected_output) = database.get_test_data(test_id).await?;
    let problem = database.get_submission_problem(submission_id).await?;
    let time_limit = database.get_problem_time_limit(problem).await?;

    let (result, time) = execute_test(&input, &expected_output, executable, time_limit, worker_id).await?;

    database.set_test_result(submission_id, test_id, result).await?;

    database.set_test_time(submission_id, test_id, time).await?;

    Ok(())
}

async fn worker_test_is_done(database: &Database, submission_id: SubmissionId, executable: &Path, queue_size: &Arc<AtomicI32>) -> Result<()> {
    queue_size.fetch_sub(1, Ordering::SeqCst);
    database.increment_submission_tests_done(submission_id).await?;
    let tests_done = database.get_submission_tests_done(submission_id).await?;
    let total_tests = database.get_tests_for_submission(submission_id).await?.len() as i32;
    if tests_done == total_tests {
        database.update_submission_result(submission_id).await?;
        // delete the executable if it exists
        tokio::fs::remove_file(executable).await.ok();
    }
    Ok(())
}

async fn worker(mut receiver: Receiver<(SubmissionId, TestId, PathBuf)>, queue_size: Arc<AtomicI32>, database: Database, worker_id: i32) -> ! {
    loop {
        if let Some((submission_id, test_id, executable)) = receiver.recv().await {
            // execute the test

            let res = worker_do_test(&database, submission_id, test_id, &executable, worker_id).await;
            if let Err(e) = res {
                eprintln!("Error while testing: {e}");
                database.set_test_result(submission_id, test_id, TestingResult::InternalError).await.ok();
                // ignore errors
            }
            worker_test_is_done(&database, submission_id, &executable, &queue_size).await.ok();
        }
    }
}

fn spawn_worker(database: &Database, worker_id: i32) -> (Sender<(SubmissionId, TestId, PathBuf)>, Arc<AtomicI32>) {
    let (sender, receiver) = mpsc::channel(BUFFER_SIZE);
    let queue_size = Arc::new(AtomicI32::new(0));
    let queue_size_clone = queue_size.clone();

    let database = database.clone();
    tokio::spawn(async move {
        worker(receiver, queue_size_clone, database, worker_id).await;
    });

    (sender, queue_size)
}

async fn compile_code(code: &str) -> Result<PathBuf> {
    // compile the code with gcc and return the executable
    // g++ -o /dev/stdout -O2 -std=c++17 -x c++ -

    let mut output_name = "temp/compiled_".to_owned();
    // output should consist of 10 random characters
    for _ in 0..10 {
        let c = (b'a' + rand::random::<u8>() % 26) as char;
        output_name.push(c);
    }

    // create temp directory if it doesn't exist
    tokio::fs::create_dir_all("temp").await?;

    let mut proc = Command::new("g++")
        .arg("-o")
        .arg(output_name.clone())
        .arg("-O2")
        .arg("-std=c++17")
        .arg("-x")
        .arg("c++")
        .arg("-")
        .arg("-DONLINE_JUDGE")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;

    let mut stdin = proc.stdin.take().ok_or_else(|| anyhow::anyhow!("Failed to open stdin"))?;
    stdin.write_all(code.as_bytes()).await?;
    drop(stdin);

    let output = proc.wait_with_output().await?;
    if !output.status.success() {
        return Err(anyhow::anyhow!("Compilation failed: {}", output.status));
    }

    Ok(Path::new(&output_name).to_owned())
}

#[derive(Clone)]
#[allow(clippy::type_complexity)]
pub struct WorkerManager {
    workers: Arc<Vec<(Sender<(SubmissionId, TestId, PathBuf)>, Arc<AtomicI32>)>>,
}

impl WorkerManager {
    pub fn new(worker_count: usize, database: &Database) -> Self {
        let mut workers = Vec::new();
        for worker_id in 0..worker_count {
            workers.push(spawn_worker(database, worker_id as i32));
        }
        Self { workers: Arc::new(workers) }
    }

    async fn execute_test(&self, submission_id: SubmissionId, test_id: TestId, executable: PathBuf) -> Result<()> {
        let mut min_queue_size = i32::MAX;
        let mut min_queue_index = 0;
        for (i, (_sender, queue_size)) in self.workers.iter().enumerate() {
            let queue_size = queue_size.load(Ordering::SeqCst);
            if queue_size < min_queue_size {
                min_queue_size = queue_size;
                min_queue_index = i;
            }
        }

        let (sender, queue_size) = &self.workers.get(min_queue_index).ok_or_else(|| anyhow::anyhow!("No workers"))?;
        queue_size.fetch_add(1, Ordering::SeqCst);
        sender.send((submission_id, test_id, executable)).await?;

        Ok(())
    }

    pub async fn test_submission(&self, submission_id: SubmissionId, database: &Database) -> Result<()> {
        database.set_submission_result(submission_id, TestingResult::Compiling).await?;

        let code = database.get_submission_code(submission_id).await?;
        let exe = compile_code(&code).await;

        if let Err(e) = exe {
            database.set_submission_result(submission_id, TestingResult::CompilationError).await?;
            eprintln!("Error while compiling: {e}");
            return Ok(());
        }

        let exe = exe?;

        database.set_submission_result(submission_id, TestingResult::Testing).await?;
        for subtask in database.get_subtasks_for_submission(submission_id).await? {
            database.set_subtask_result(submission_id, subtask, TestingResult::Testing).await?;
        }

        let tests = database.get_tests_for_submission(submission_id).await?;
        for test in tests {
            self.execute_test(submission_id, test, exe.clone()).await?;
        }

        Ok(())
    }
}

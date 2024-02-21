use crate::database::submission::{SubmissionId, TestingResult};
use crate::database::test::TestId;
use crate::database::Database;
use crate::tester::execute_test;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio::sync::mpsc::{Receiver, Sender};

const BUFFER_SIZE: usize = 255;

async fn worker(
    mut receiver: Receiver<(SubmissionId, TestId, Arc<PathBuf>)>,
    queue_size: Arc<AtomicI32>,
    database: Database,
) {
    loop {
        let (submission_id, test_id, executable) = receiver.recv().await.unwrap();
        // execute the test

        database
            .set_test_result(submission_id, test_id, TestingResult::Testing)
            .await;

        let (input, expected_output) = database.get_test_data(test_id).await;
        let problem = database.get_submission_problem(submission_id).await;
        let time_limit = database.get_problem_time_limit(problem).await;

        let (result, time) = execute_test(&input, &expected_output, &executable, time_limit).await;

        database
            .set_test_result(submission_id, test_id, result)
            .await;

        database.set_test_time(submission_id, test_id, time).await;

        queue_size.fetch_sub(1, Ordering::SeqCst);
        database
            .increment_submission_tests_done(submission_id)
            .await;
        let tests_done = database.get_submission_tests_done(submission_id).await;
        let total_tests = database.get_tests_for_submission(submission_id).await.len() as i32;
        if tests_done == total_tests {
            database.update_submission_result(submission_id).await;
        }
    }
}

async fn spawn_worker(
    database: &Database,
) -> (Sender<(SubmissionId, TestId, Arc<PathBuf>)>, Arc<AtomicI32>) {
    let (sender, receiver) = mpsc::channel(BUFFER_SIZE);
    let queue_size = Arc::new(AtomicI32::new(0));
    let queue_size_clone = queue_size.clone();

    let database = database.clone();
    tokio::spawn(async move {
        worker(receiver, queue_size_clone, database).await;
    });

    (sender, queue_size)
}

async fn compile_code(code: &str) -> PathBuf {
    // compile the code with gcc and return the executable
    // g++ -o /dev/stdout -O2 -std=c++17 -x c++ -

    let mut output_name = "temp/compiled_".to_owned();
    // output should consist of 10 random characters
    for _ in 0..10 {
        let c = (b'a' + rand::random::<u8>() % 26) as char;
        output_name.push(c);
    }

    // create temp directory if it doesn't exist
    tokio::fs::create_dir_all("temp").await.unwrap();

    let mut proc = Command::new("g++")
        .arg("-o")
        .arg(output_name.clone())
        .arg("-O2")
        .arg("-std=c++17")
        .arg("-x")
        .arg("c++")
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    let mut stdin = proc.stdin.take().unwrap();
    stdin.write_all(code.as_bytes()).await.unwrap();
    drop(stdin);

    proc.wait_with_output().await.unwrap();

    Path::new(&output_name).to_owned()
}

#[derive(Clone)]
pub struct WorkerManager {
    workers: Arc<Vec<(Sender<(SubmissionId, TestId, Arc<PathBuf>)>, Arc<AtomicI32>)>>,
}

impl WorkerManager {
    pub async fn new(worker_count: usize, database: &Database) -> WorkerManager {
        let mut workers = Vec::new();
        for _ in 0..worker_count {
            workers.push(spawn_worker(database).await);
        }
        WorkerManager {
            workers: Arc::new(workers),
        }
    }

    async fn execute_test(
        &self,
        submission_id: SubmissionId,
        test_id: TestId,
        executable: Arc<PathBuf>,
    ) {
        let mut min_queue_size = i32::MAX;
        let mut min_queue_index = 0;
        for (_sender, queue_size) in self.workers.iter() {
            let queue_size = queue_size.load(Ordering::SeqCst);
            if queue_size < min_queue_size {
                min_queue_size = queue_size;
                min_queue_index += 1;
            }
        }

        let (sender, queue_size) = &self.workers[min_queue_index];
        queue_size.fetch_add(1, Ordering::SeqCst);
        sender
            .send((submission_id, test_id, executable))
            .await
            .unwrap();
    }

    pub async fn test_submission(&self, submission_id: SubmissionId, database: &Database) {
        database
            .set_submission_result(submission_id, TestingResult::Compiling)
            .await;

        let code = database.get_submission_code(submission_id).await;
        let exe = Arc::new(compile_code(&code).await);

        database
            .set_submission_result(submission_id, TestingResult::Testing)
            .await;
        for subtask in database.get_subtasks_for_submission(submission_id).await {
            database
                .set_submission_result(submission_id, TestingResult::Testing)
                .await;
        }

        let tests = database.get_tests_for_submission(submission_id).await;
        for test in tests {
            self.execute_test(submission_id, test, exe.clone()).await;
        }
    }
}

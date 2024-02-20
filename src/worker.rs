use crate::database::Database;
use crate::submission::{SubmissionId, TestingResult};
use crate::test::TestId;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::{mpsc, Mutex};

const BUFFER_SIZE: usize = 255;

async fn worker(
    mut receiver: Receiver<(SubmissionId, TestId, Arc<PathBuf>)>,
    queue_size: Arc<AtomicI32>,
    database: Database,
) {
    loop {
        let (submission_id, test_id, executable) = receiver.recv().await.unwrap();
        // execute the test

        let mut child = Command::new(executable.as_os_str())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        let (input, expected_output) = database.get_test_data(test_id).await;

        let mut stdin = child.stdin.as_mut().unwrap();
        stdin.write_all(input.as_bytes()).await.unwrap();

        let output = child.wait_with_output().await.unwrap();
        let output = String::from_utf8(output.stdout).unwrap();

        if output == expected_output {
            database
                .set_test_result(submission_id, test_id, TestingResult::Accepted)
                .await;
        } else {
            database
                .set_test_result(submission_id, test_id, TestingResult::WrongAnswer)
                .await;
        };

        queue_size.fetch_sub(1, Ordering::SeqCst);
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
    lock: Arc<Mutex<()>>,
}

impl WorkerManager {
    pub async fn new(worker_count: usize, database: &Database) -> WorkerManager {
        let mut workers = Vec::new();
        for _ in 0..worker_count {
            workers.push(spawn_worker(database).await);
        }
        WorkerManager {
            workers: Arc::new(workers),
            lock: Arc::new(Mutex::new(())),
        }
    }

    async fn execute_test(
        &self,
        submission_id: SubmissionId,
        test_id: TestId,
        executable: Arc<PathBuf>,
    ) {
        // holds a lock to the workers, so that we can find the worker with the smallest queue size
        let _ = self.lock.lock().await;

        let mut min_queue_size = i32::MAX;
        for (_sender, queue_size) in self.workers.iter() {
            let queue_size = queue_size.load(Ordering::SeqCst);
            if queue_size < min_queue_size {
                min_queue_size = queue_size;
            }
        }

        for (sender, queue_size) in self.workers.iter() {
            if queue_size.load(Ordering::SeqCst) == min_queue_size {
                sender
                    .send((submission_id, test_id, executable))
                    .await
                    .unwrap();
                queue_size.fetch_add(1, Ordering::SeqCst);
                break;
            }
        }
    }

    pub async fn test_submission(&self, submission_id: SubmissionId, database: &Database) {
        let code = database.get_submission_code(submission_id).await;
        let exe = Arc::new(compile_code(&code).await);

        let tests = database.get_tests_for_submission(submission_id).await;
        for test in tests {
            self.execute_test(submission_id, test, exe.clone()).await;
        }
    }
}

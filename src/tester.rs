use crate::database::submission::TestingResult;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

pub async fn execute_test(official_input: &str, official_output: &str, executable_path: &Path, time_limit: i32) -> (TestingResult, i32) {
    let start_time = tokio::time::Instant::now();
    let mut child = Command::new(executable_path.as_os_str()).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();

    let stdin = child.stdin.as_mut().unwrap();
    stdin.write_all(official_input.as_bytes()).await.unwrap();

    let process = tokio::spawn(tokio::time::timeout(Duration::from_millis(time_limit as u64), async move {
        let status = child.wait_with_output().await.unwrap();
        status
    }));

    let process = process.await;

    if let Ok(Ok(output)) = process {
        let output = String::from_utf8(output.stdout).unwrap();

        let output = output.split_ascii_whitespace();
        let official_output = official_output.split_ascii_whitespace();

        let result = if output.eq(official_output) { TestingResult::Accepted } else { TestingResult::WrongAnswer };

        (result, start_time.elapsed().as_millis() as i32)
    } else {
        (TestingResult::TimeLimitExceeded, time_limit)
    }
}

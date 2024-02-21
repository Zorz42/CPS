use crate::database::submission::TestingResult;
use std::path::Path;
use std::process::Stdio;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

pub async fn execute_test(
    official_input: &str,
    official_output: &str,
    executable_path: &Path,
) -> TestingResult {
    let mut child = Command::new(executable_path.as_os_str())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    let stdin = child.stdin.as_mut().unwrap();
    stdin.write_all(official_input.as_bytes()).await.unwrap();

    let output = child.wait_with_output().await.unwrap();
    let output = String::from_utf8(output.stdout).unwrap();

    let output = output.split_ascii_whitespace();
    let official_output = official_output.split_ascii_whitespace();

    if output.eq(official_output) {
        TestingResult::Accepted
    } else {
        TestingResult::WrongAnswer
    }
}

use crate::database::submission::TestingResult;
use anyhow::Result;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

pub async fn execute_test(official_input: &str, official_output: &str, executable_path: &Path, time_limit: i32) -> Result<(TestingResult, i32)> {
    return execute_test_unsafely(official_input, official_output, executable_path, time_limit).await;
}

pub async fn execute_test_unsafely(official_input: &str, official_output: &str, executable_path: &Path, time_limit: i32) -> Result<(TestingResult, i32)> {
    let start_time = tokio::time::Instant::now();
    let mut child = Command::new(executable_path.as_os_str()).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn()?;

    let stdin = child.stdin.as_mut().ok_or_else(|| anyhow::anyhow!("Failed to open stdin"))?;
    stdin.write_all(official_input.as_bytes()).await?;

    let process = tokio::spawn(tokio::time::timeout(Duration::from_millis(time_limit as u64), async move {
        let status = child.wait_with_output().await?;
        anyhow::Ok(status)
    }));

    let process = process.await?;

    if let Ok(output) = process {
        let output = String::from_utf8(output?.stdout)?;

        let output = output.split_ascii_whitespace();
        let official_output = official_output.split_ascii_whitespace();

        let result = if output.eq(official_output) { TestingResult::Accepted } else { TestingResult::WrongAnswer };

        Ok((result, start_time.elapsed().as_millis() as i32))
    } else {
        Ok((TestingResult::TimeLimitExceeded, time_limit))
    }
}

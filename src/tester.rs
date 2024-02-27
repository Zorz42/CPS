use crate::database::submission::TestingResult;
use anyhow::Result;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;

pub async fn is_isolate_installed() -> bool {
    let child = Command::new("isolate").arg("--version").stdout(Stdio::null()).stderr(Stdio::null()).spawn();
    if let Ok(mut child) = child {
        let status = child.wait().await;
        if let Ok(status) = status {
            return status.success();
        }
    }
    false
}

pub async fn execute_test(official_input: &str, official_output: &str, executable_path: &Path, time_limit: i32, worker_id: i32) -> Result<(TestingResult, i32)> {
    // check if isolate is installed
    if is_isolate_installed().await {
        execute_test_isolated(official_input, official_output, executable_path, time_limit, worker_id).await
    } else {
        execute_test_unsafely(official_input, official_output, executable_path, time_limit).await
    }
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

async fn cleanup_box(box_id: i32) -> Result<()> {
    let mut child = Command::new("isolate").arg("--cleanup").arg(format!("--box-id={box_id}")).spawn()?;
    child.wait().await?;
    Ok(())
}

pub async fn execute_test_isolated(official_input: &str, official_output: &str, executable_path: &Path, time_limit: i32, box_id: i32) -> Result<(TestingResult, i32)> {
    cleanup_box(box_id).await?;

    // first initialize the box
    let child = Command::new("isolate")
        .arg("--init")
        .arg(format!("--box-id={box_id}"))
        .arg("--fsize=1024")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;
    let status = child.wait_with_output().await?;
    if !status.status.success() {
        return Ok((TestingResult::InternalError, 0));
    }

    let mut box_path = String::from_utf8(status.stdout)?;
    if box_path.ends_with('\n') {
        box_path.pop();
    }

    let meta_file = format!("temp/meta{box_id}.txt");

    let executable_name = executable_path
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("Failed to get file name"))?
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("Failed to convert to string"))?;

    // copy the executable to the box
    std::fs::copy(executable_path, format!("{box_path}/box/{executable_name}"))?;

    // run the executable
    let mut child = Command::new("isolate")
        .arg(format!("--box-id={box_id}"))
        .arg(format!("--meta={meta_file}"))
        .arg("--silent")
        .arg(format!("--time={}", time_limit as f32 / 1000.0))
        .arg("--run")
        .arg("--")
        .arg(executable_name)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;

    let stdin = child.stdin.as_mut().ok_or_else(|| anyhow::anyhow!("Failed to open stdin"))?;
    stdin.write_all(official_input.as_bytes()).await?;

    let output = child.wait_with_output().await?;
    let output = String::from_utf8(output.stdout)?;

    cleanup_box(box_id).await?;

    let meta = {
        let mut meta_file_obj = tokio::fs::File::open(&meta_file).await?;
        let mut meta = String::new();
        meta_file_obj.read_to_string(&mut meta).await?;
        meta
    };

    tokio::fs::remove_file(&meta_file).await?;

    let mut exitcode = 0;
    let mut exitsignal = 0;
    let mut status = String::new();
    let mut killed = 0;
    let mut time = 0.0;

    for line in meta.lines() {
        let mut parts = line.split(':').collect::<Vec<_>>();
        if parts.len() != 2 {
            continue;
        }
        let key = parts.remove(0);
        let value = parts.remove(0);

        match key {
            "exitcode" => exitcode = value.parse()?,
            "status" => status = value.to_owned(),
            "killed" => killed = value.parse()?,
            "time" => time = value.parse()?,
            "exitsig" => exitsignal = value.parse()?,
            _ => {}
        }
    }

    if exitsignal != 0 || exitcode != 0 {
        return Ok((TestingResult::RuntimeError, (time * 1000.0) as i32));
    }

    if killed != 0 {
        return match status.as_str() {
            "TO" => Ok((TestingResult::TimeLimitExceeded, (time * 1000.0) as i32)),
            "SG" | "RE" => Ok((TestingResult::RuntimeError, (time * 1000.0) as i32)),
            _ => Ok((TestingResult::InternalError, (time * 1000.0) as i32)),
        };
    }

    let output = output.split_ascii_whitespace();
    let official_output = official_output.split_ascii_whitespace();

    let result = if output.eq(official_output) { TestingResult::Accepted } else { TestingResult::WrongAnswer };

    Ok((result, (time * 1000.0) as i32))
}

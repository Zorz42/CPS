use crate::database::submission::{testing_result_to_short_string, testing_result_to_string, TestingResult};
use crate::database::user::UserId;
use crate::database::Database;
use crate::request_handler::{create_html_response, RedirectSite};
use crate::sidebar::{create_sidebar_context, SidebarContext};
use crate::worker::WorkerManager;
use anyhow::{anyhow, Result};
use askama::Template;
use http_body_util::BodyExt;
use http_body_util::Full;
use hyper::body::{Bytes, Incoming};
use hyper::{Request, Response};

#[derive(Template)]
#[template(path = "submission.html")]
#[allow(clippy::type_complexity)]
pub struct SubmissionSite {
    code: String,
    subtasks: Vec<(i32, i32, bool, String, Vec<(String, String, i32)>)>,
    points: i32,
    max_points: i32,
    result: String,
    sidebar_context: SidebarContext,
}

fn split_bytes_by_bytes(data: Vec<u8>, splitter: Vec<u8>) -> Vec<Vec<u8>> {
    let mut res = Vec::new();
    let mut current = Vec::new();
    let mut i = 0;

    while i < data.len() {
        if data[i..].starts_with(&splitter) {
            res.push(current);
            current = Vec::new();
            i += splitter.len();
        } else {
            current.push(data[i]);
            i += 1;
        }
    }

    res.push(current);

    res
}

fn join_bytes(data: Vec<Vec<u8>>, splitter: Vec<u8>) -> Vec<u8> {
    let mut res = Vec::new();

    for (i, part) in data.iter().enumerate() {
        res.extend_from_slice(part);
        if i != data.len() - 1 {
            res.extend_from_slice(&splitter);
        }
    }

    res
}

pub async fn extract_file_from_request(request: Request<Incoming>) -> Result<Vec<u8>> {
    let boundary = request
        .headers()
        .get("content-type")
        .and_then(|ct| ct.to_str().ok())
        .and_then(|ct| {
            let parts: Vec<&str> = ct.split(';').collect();
            let mut boundary = None;

            for part in parts {
                let parts: Vec<&str> = part.trim().split('=').collect();
                if parts.len() != 2 {
                    continue;
                }

                if parts.first().unwrap_or(&"") == &"boundary" {
                    boundary = Some((*parts.get(1).unwrap_or(&"")).to_owned());
                }
            }

            boundary
        })
        .unwrap_or_else(|| "no-boundary".to_owned());

    let boundary = format!("--{boundary}").as_bytes().to_vec();

    let body = request.into_body().collect().await?.to_bytes().to_vec();

    let mut parts = split_bytes_by_bytes(body, boundary);
    parts.retain(|x| !x.is_empty());
    parts.pop();
    let part = parts.first().ok_or_else(|| anyhow!("No file in request"))?;
    let mut code_parts = split_bytes_by_bytes(part.to_vec(), "\r\n".as_bytes().to_vec());
    code_parts.remove(0);
    code_parts.remove(0);
    code_parts.remove(0);
    code_parts.remove(0);
    code_parts.pop();

    Ok(join_bytes(code_parts, "\r\n".as_bytes().to_vec()))
}

pub async fn handle_submission_form(
    database: &Database,
    user_id: UserId,
    contest_id: &str,
    problem_id: &str,
    request: Request<Incoming>,
    workers: &WorkerManager,
) -> Result<Option<Response<Full<Bytes>>>> {
    let code = extract_file_from_request(request).await?;
    let code = String::from_utf8_lossy(&code).to_string();

    if !code.is_empty() {
        database.add_submission(user_id, problem_id.parse()?, code, workers).await?;
    }

    Ok(Some(create_html_response(&RedirectSite {
        url: format!("/contest/{contest_id}/problem/{problem_id}"),
    })?))
}

pub async fn create_submission_page(database: &Database, submission_id: &str, user: UserId) -> Result<Option<Response<Full<Bytes>>>> {
    if let Ok(submission_id) = submission_id.parse() {
        let code = database.get_submission_code(submission_id).await?;
        let subtasks = database.get_subtasks_for_submission(submission_id).await?;
        let mut subtask_vec = Vec::new();
        for subtask in subtasks {
            let tests = database.get_tests_for_subtask(subtask).await?;
            let mut test_vec = Vec::new();

            for test in tests {
                let time = database.get_test_time(submission_id, test).await?.unwrap_or(0);
                let test_result = database.get_test_result(submission_id, test).await?;

                let color = match test_result {
                    TestingResult::InQueue | TestingResult::Compiling | TestingResult::Testing => "#909090",
                    TestingResult::Accepted => "#00FF00",
                    TestingResult::WrongAnswer
                    | TestingResult::RuntimeError
                    | TestingResult::TimeLimitExceeded
                    | TestingResult::MemoryLimitExceeded
                    | TestingResult::CompilationError
                    | TestingResult::InternalError => "#FF0000",
                }
                .to_owned();

                test_vec.push((testing_result_to_string(test_result), color, time));
            }

            let points = database.get_subtask_points_result(submission_id, subtask).await?.unwrap_or(0);
            let max_points = database.get_subtask_total_points(subtask).await?;

            let result = database.get_subtask_result(submission_id, subtask).await?;
            let hide_score = result == TestingResult::InQueue || result == TestingResult::Testing || result == TestingResult::CompilationError || result == TestingResult::Compiling;

            let message = testing_result_to_short_string(result);

            subtask_vec.push((points, max_points, hide_score, message, test_vec));
        }

        let result = database.get_submission_result(submission_id).await?;
        let points = database.get_submission_points(submission_id).await?.unwrap_or(0);
        let problem = database.get_submission_problem(submission_id).await?;
        let max_points = database.get_problem_total_points(problem).await?.max(1);

        return Ok(Some(create_html_response(&SubmissionSite {
            code,
            subtasks: subtask_vec,
            points,
            max_points,
            result: testing_result_to_string(result),
            sidebar_context: create_sidebar_context(database, Some(user)).await?,
        })?));
    }

    Ok(None)
}

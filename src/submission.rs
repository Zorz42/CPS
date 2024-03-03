use crate::database::submission::testing_result_to_string;
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
    subtasks: Vec<(String, String, Vec<(String, String)>)>,
    result: String,
    score: String,
    sidebar_context: SidebarContext,
}

async fn extract_file_from_request(request: Request<Incoming>) -> Result<String> {
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

    let boundary = format!("--{boundary}");

    let body = request.into_body().collect().await?.to_bytes();
    let body = String::from_utf8_lossy(&body).to_string();

    let mut parts = body.split(&boundary).collect::<Vec<&str>>();
    parts.retain(|x| !x.is_empty());
    parts.pop();
    let part = parts.first().ok_or_else(|| anyhow!("No file in request"))?;
    let mut code_parts = part.split("\r\n").collect::<Vec<&str>>();
    code_parts.remove(0);
    code_parts.remove(0);
    code_parts.remove(0);
    code_parts.remove(0);
    code_parts.pop();

    Ok(code_parts.join("\r\n\r\n"))
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
                let time = database.get_test_time(submission_id, test).await?;

                let time_str = time.map_or_else(String::new, |time| format!("{time}ms"));

                test_vec.push((testing_result_to_string(database.get_test_result(submission_id, test).await?), time_str));
            }

            let points = database.get_subtask_points_result(submission_id, subtask).await?;
            let score_string = if let Some(points) = points {
                format!("{}/{}", points, database.get_subtask_total_points(subtask).await?)
            } else {
                String::new()
            };

            subtask_vec.push((score_string, testing_result_to_string(database.get_subtask_result(submission_id, subtask).await?), test_vec));
        }

        let result = testing_result_to_string(database.get_submission_result(submission_id).await?);
        let points = database.get_submission_points(submission_id).await?;
        let problem = database.get_submission_problem(submission_id).await?;
        let score = if let Some(points) = points {
            format!("{}/{}", points, database.get_problem_total_points(problem).await?)
        } else {
            String::new()
        };

        return Ok(Some(create_html_response(&SubmissionSite {
            code,
            subtasks: subtask_vec,
            result,
            score,
            sidebar_context: create_sidebar_context(database, Some(user)).await?,
        })?));
    }

    Ok(None)
}

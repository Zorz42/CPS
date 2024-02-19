use crate::database::Database;
use crate::problem::ProblemId;
use crate::user::UserId;
use crate::{create_html_response, RedirectSite};
use anyhow::Result;
use http_body_util::BodyExt;
use http_body_util::Full;
use hyper::body::{Bytes, Incoming};
use hyper::{Request, Response};

pub type SubmissionId = i32;

impl Database {
    pub async fn init_submissions(&self) {
        self.get_postgres_client()
            .execute(
                "CREATE TABLE IF NOT EXISTS submissions (
                    submission_id SERIAL PRIMARY KEY,
                    user_id INT REFERENCES users(user_id),
                    problem_id INT REFERENCES problems(problem_id),
                    code TEXT NOT NULL
                );",
                &[],
            )
            .await
            .unwrap();
    }

    pub async fn add_submission(
        &self,
        user_id: UserId,
        problem_id: ProblemId,
        code: String,
    ) -> SubmissionId {
        self.get_postgres_client()
            .query(
                "INSERT INTO submissions (user_id, problem_id, code) VALUES ($1, $2, $3) RETURNING submission_id",
                &[&user_id, &problem_id, &code],
            ).await
            .unwrap()
            .get(0).unwrap()
            .get(0)
    }

    pub async fn get_submissions_by_user_for_problem(
        &self,
        user_id: UserId,
        problem_id: ProblemId,
    ) -> Vec<SubmissionId> {
        self.get_postgres_client()
            .query(
                "SELECT submission_id FROM submissions WHERE user_id = $1 AND problem_id = $2",
                &[&user_id, &problem_id],
            )
            .await
            .unwrap()
            .iter()
            .map(|row| row.get(0))
            .collect()
    }
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

                if parts[0] == "boundary" {
                    boundary = Some(parts[1].to_string());
                }
            }

            boundary
        })
        .unwrap_or("no-boundary".to_string());

    let boundary = format!("--{}", boundary);

    let body = request.into_body().collect().await?.to_bytes();
    let body = String::from_utf8_lossy(&body).to_string();

    let mut parts = body.split(&boundary).collect::<Vec<&str>>();
    parts.retain(|x| !x.is_empty());
    parts.pop();
    let part = parts[0];
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
    user_id: Option<UserId>,
    contest_id: &str,
    problem_id: &str,
    request: Request<Incoming>,
) -> Result<Option<Response<Full<Bytes>>>> {
    let code = extract_file_from_request(request).await?;

    database
        .add_submission(user_id.unwrap(), problem_id.parse().unwrap(), code)
        .await;

    Ok(Some(create_html_response(RedirectSite {
        url: format!("/contest/{}/problem/{}", contest_id, problem_id),
    })?))
}

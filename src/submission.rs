use crate::database::Database;
use crate::problem::ProblemId;
use crate::request_handler::{create_html_response, RedirectSite};
use crate::user::UserId;
use anyhow::Result;
use askama::Template;
use http_body_util::BodyExt;
use http_body_util::Full;
use hyper::body::{Bytes, Incoming};
use hyper::{Request, Response};

pub type SubmissionId = i32;

pub enum TestingResult {
    InQueue,
    Compiling,
    Testing,
    Accepted,
    WrongAnswer,
    RuntimeError,
    TimeLimitExceeded,
    MemoryLimitExceeded,
    InternalError,
}

// make sure that testing results are stored in the database as integers
pub fn testing_result_to_i32(result: TestingResult) -> i32 {
    match result {
        TestingResult::InQueue => 0,
        TestingResult::Compiling => 1,
        TestingResult::Testing => 2,
        TestingResult::Accepted => 3,
        TestingResult::WrongAnswer => 4,
        TestingResult::RuntimeError => 5,
        TestingResult::TimeLimitExceeded => 6,
        TestingResult::MemoryLimitExceeded => 7,
        TestingResult::InternalError => 8,
    }
}

pub fn i32_to_testing_result(result: i32) -> TestingResult {
    match result {
        0 => TestingResult::InQueue,
        1 => TestingResult::Compiling,
        2 => TestingResult::Testing,
        3 => TestingResult::Accepted,
        4 => TestingResult::WrongAnswer,
        5 => TestingResult::RuntimeError,
        6 => TestingResult::TimeLimitExceeded,
        7 => TestingResult::MemoryLimitExceeded,
        8 => TestingResult::InternalError,
        _ => panic!("Invalid testing result"),
    }
}

// make sure to display testing results as strings in the HTML
pub fn testing_result_to_string(result: TestingResult) -> String {
    match result {
        TestingResult::InQueue => "In Queue".to_string(),
        TestingResult::Compiling => "Compiling".to_string(),
        TestingResult::Testing => "Testing".to_string(),
        TestingResult::Accepted => "Accepted".to_string(),
        TestingResult::WrongAnswer => "Wrong Answer".to_string(),
        TestingResult::RuntimeError => "Runtime Error".to_string(),
        TestingResult::TimeLimitExceeded => "Time Limit Exceeded".to_string(),
        TestingResult::MemoryLimitExceeded => "Memory Limit Exceeded".to_string(),
        TestingResult::InternalError => "Internal Error".to_string(),
    }
}

#[derive(Template)]
#[template(path = "submission.html")]
pub struct SubmissionSite {
    code: String,
    subtasks: Vec<(i32, String, Vec<String>)>,
}

impl Database {
    pub async fn init_submissions(&self) {
        self.get_postgres_client()
            .execute(
                "CREATE TABLE IF NOT EXISTS submissions (
                    submission_id SERIAL PRIMARY KEY,
                    user_id INT REFERENCES users(user_id),
                    problem_id INT REFERENCES problems(problem_id),
                    code TEXT NOT NULL,
                    result INT NOT NULL
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
        let submission_id = self.get_postgres_client()
            .query(
                "INSERT INTO submissions (user_id, problem_id, code, result) VALUES ($1, $2, $3, $4) RETURNING submission_id",
                &[&user_id, &problem_id, &code, &testing_result_to_i32(TestingResult::InQueue)],
            ).await
            .unwrap()
            .get(0).unwrap()
            .get(0);

        // add all subtasks for the problem
        let subtasks = self.get_subtasks_for_problem(problem_id).await;
        for subtask in subtasks {
            self.get_postgres_client()
                .execute(
                    "INSERT INTO subtask_results (submission_id, subtask_id, result) VALUES ($1, $2, $3)",
                    &[&submission_id, &subtask, &testing_result_to_i32(TestingResult::InQueue)],
                ).await.unwrap();
        }

        // add all tests for the problem
        let tests = self.get_all_tests_for_problem(problem_id).await;
        for test in tests {
            self.get_postgres_client()
                .execute(
                    "INSERT INTO test_results (submission_id, test_id, result) VALUES ($1, $2, $3)",
                    &[
                        &submission_id,
                        &test,
                        &testing_result_to_i32(TestingResult::InQueue),
                    ],
                )
                .await
                .unwrap();
        }

        submission_id
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

    pub async fn get_all_submissions_for_user(&self, user_id: UserId) -> Vec<SubmissionId> {
        self.get_postgres_client()
            .query(
                "SELECT submission_id FROM submissions WHERE user_id = $1",
                &[&user_id],
            )
            .await
            .unwrap()
            .iter()
            .map(|row| row.get(0))
            .collect()
    }

    pub async fn delete_all_submissions_for_user(&self, user_id: UserId) {
        for submission in self.get_all_submissions_for_user(user_id).await {
            self.delete_all_results_for_submission(submission).await;
        }

        self.get_postgres_client()
            .execute("DELETE FROM submissions WHERE user_id = $1", &[&user_id])
            .await
            .unwrap();
    }

    pub async fn get_submission_code(&self, submission_id: SubmissionId) -> String {
        self.get_postgres_client()
            .query(
                "SELECT code FROM submissions WHERE submission_id = $1",
                &[&submission_id],
            )
            .await
            .unwrap()
            .get(0)
            .unwrap()
            .get(0)
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

pub async fn create_submission_page(
    database: &Database,
    submission_id: &str,
) -> Result<Option<Response<Full<Bytes>>>> {
    if let Ok(submission_id) = submission_id.parse() {
        let code = database.get_submission_code(submission_id).await;
        let subtasks = database.get_subtasks_for_submission(submission_id).await;
        let mut subtask_vec = Vec::new();
        for subtask in subtasks {
            let tests = database
                .get_tests_for_subtask_in_submission(submission_id, subtask)
                .await;
            let mut test_vec = Vec::new();
            for test in tests {
                test_vec.push(testing_result_to_string(
                    database.get_test_result(submission_id, test).await,
                ));
            }
            subtask_vec.push((
                subtask,
                testing_result_to_string(database.get_subtask_result(submission_id, subtask).await),
                test_vec,
            ));
        }

        return Ok(Some(create_html_response(SubmissionSite {
            code,
            subtasks: subtask_vec,
        })?));
    }

    Ok(None)
}

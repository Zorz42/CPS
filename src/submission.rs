use crate::id::GenericId;
use crate::problem::{ProblemDatabase, ProblemId};
use crate::test::EvaluationSubtask;
use crate::user::UserId;
use crate::{create_html_response, GlobalState, RedirectSite};
use anyhow::Result;
use http_body_util::BodyExt;
use http_body_util::Full;
use hyper::body::{Bytes, Incoming};
use hyper::{Request, Response};
use std::collections::HashMap;

pub type SubmissionId = GenericId;

pub enum EvaluationStatus {
    Pending,
    Compiling,
    Testing,
    Accepted,
    WrongAnswer,
    CompilationError,
    RuntimeError,
    TimeLimitExceeded,
    MemoryLimitExceeded,
    InternalError,
    Unknown,
}

pub struct Submission {
    pub user_id: UserId,
    pub problem_id: ProblemId,
    pub code: String,
    pub status: EvaluationStatus,
    pub subtasks: Vec<EvaluationSubtask>,
}

pub struct SubmissionDatabase {
    submissions: HashMap<SubmissionId, Submission>,
}

impl SubmissionDatabase {
    pub fn new() -> SubmissionDatabase {
        SubmissionDatabase {
            submissions: HashMap::new(),
        }
    }

    pub fn add_submission(
        &mut self,
        user_id: UserId,
        problem_id: ProblemId,
        code: String,
        problems: &mut ProblemDatabase,
    ) -> SubmissionId {
        let id = SubmissionId::new();
        self.submissions.insert(
            id,
            Submission {
                user_id,
                problem_id,
                code,
                status: EvaluationStatus::Pending,
                subtasks: Vec::new(),
            },
        );

        problems.add_submission(problem_id, id, user_id);

        id
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
    state: &GlobalState,
    user_id: Option<UserId>,
    contest_id: &str,
    problem_id: &str,
    request: Request<Incoming>,
) -> Result<Option<Response<Full<Bytes>>>> {
    let code = extract_file_from_request(request).await?;

    let mut submissions = state.submissions();
    let mut problems = state.problems();

    submissions.add_submission(
        user_id.unwrap(),
        ProblemId::from_int(problem_id.parse().unwrap()),
        code,
        &mut problems,
    );

    Ok(Some(create_html_response(RedirectSite {
        url: format!("/contest/{}/problem/{}", contest_id, problem_id),
    })?))
}

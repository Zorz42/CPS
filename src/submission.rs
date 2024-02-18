use crate::id::GenericId;
use crate::test::EvaluationSubtask;
use crate::user::UserId;
use crate::GlobalState;
use anyhow::Result;
use http_body_util::BodyExt;
use http_body_util::Full;
use hyper::body::{Bytes, Incoming};
use hyper::{Request, Response};

pub type SubmissionId = GenericId;

pub enum EvaluationStatus {
    Pending,
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
    pub user_id: u128,
    pub problem_id: u32,
    pub code: String,
    pub status: EvaluationStatus,
    pub subtasks: Vec<EvaluationSubtask>,
}

pub struct SubmissionDatabase {
    submissions: Vec<Submission>,
}

impl SubmissionDatabase {
    pub fn new() -> SubmissionDatabase {
        SubmissionDatabase {
            submissions: Vec::new(),
        }
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
    problem_id: &str,
    request: Request<Incoming>,
) -> Result<Option<Response<Full<Bytes>>>> {
    let code = extract_file_from_request(request).await?;

    println!("Received code: {:?}", vec![code]);

    Ok(None)
}

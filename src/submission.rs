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

pub async fn handle_submission_form(
    state: &GlobalState,
    user_id: Option<UserId>,
    problem_id: &str,
    request: Request<Incoming>,
) -> Result<Option<Response<Full<Bytes>>>> {
    let body = request.into_body().collect().await?.to_bytes();
    let body = String::from_utf8_lossy(&body).to_string();

    // erase first 3 lines and last line
    let mut lines = body.lines();
    lines.next();
    lines.next();
    lines.next();
    let mut lines = lines.collect::<Vec<&str>>();
    lines.pop();
    let code = lines.join("\n");

    println!("Received code: {}", code);

    Ok(None)
}

use crate::id::GenericId;
use crate::test::EvaluationSubtask;

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

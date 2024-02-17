use crate::test::EvaluationSubtask;

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
    user_id: u128,
    problem_id: u32,
    code: String,
    status: EvaluationStatus,
    subtasks: Vec<EvaluationSubtask>,
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

use crate::submission::EvaluationStatus;
use std::path::PathBuf;

pub struct EvaluationSubtask {
    pub subtask_number: u32,
    pub status: EvaluationStatus,
    pub tests: Vec<EvaluationTest>,
}

pub struct EvaluationTest {
    pub test_number: u32,
    pub status: EvaluationStatus,
}

pub struct ProblemSubtask {
    pub subtask_number: u32,
    pub tests: Vec<ProblemTest>,
}

pub struct ProblemTest {
    pub problem_id: u128,
    pub test_number: u32,
    pub input_file: PathBuf,
    pub output_file: PathBuf,
}

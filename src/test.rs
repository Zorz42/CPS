use crate::id::GenericId;
use crate::submission::EvaluationStatus;
use std::path::PathBuf;

type TestId = GenericId;
type SubtaskId = GenericId;

pub struct EvaluationSubtask {
    pub status: EvaluationStatus,
    pub tests: Vec<EvaluationTest>,
}

pub struct EvaluationTest {
    pub status: EvaluationStatus,
}

pub struct ProblemSubtask {
    pub tests: Vec<ProblemTest>,
    pub subtask_id: SubtaskId,
}

pub struct ProblemTest {
    pub problem_id: u128,
    pub test_id: TestId,
    pub input_file: PathBuf,
    pub output_file: PathBuf,
}

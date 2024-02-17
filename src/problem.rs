use crate::id::GenericId;
use crate::submission::{Submission, SubmissionId};
use crate::user::UserId;
use std::collections::HashMap;

pub type ProblemId = GenericId;

pub struct Problem {
    name: String,
    description: String,
    submissions: Vec<Submission>,
    submissions_per_user: HashMap<UserId, Vec<SubmissionId>>,
}

pub struct ProblemDatabase {
    problems: Vec<Problem>,
}

impl ProblemDatabase {
    pub fn new() -> ProblemDatabase {
        ProblemDatabase {
            problems: Vec::new(),
        }
    }
}

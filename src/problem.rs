use crate::submission::Submission;
use crate::user::UserId;
use std::collections::HashMap;

pub struct Problem {
    name: String,
    description: String,
    submissions: Vec<Submission>,
    submissions_per_user: HashMap<UserId, Vec<u32>>,
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

use crate::id::GenericId;
use crate::submission::SubmissionId;
use crate::user::UserId;
use std::collections::HashMap;

pub type ProblemId = GenericId;

#[derive(Clone)]
pub struct Problem {
    pub name: String,
    pub description: String,
    pub submissions: Vec<SubmissionId>,
    pub submissions_per_user: HashMap<UserId, Vec<SubmissionId>>,
}

pub struct ProblemDatabase {
    pub problems: HashMap<ProblemId, Problem>,
}

impl ProblemDatabase {
    pub fn new() -> ProblemDatabase {
        ProblemDatabase {
            problems: HashMap::new(),
        }
    }

    pub fn get_problem(&self, id: ProblemId) -> Option<&Problem> {
        self.problems.get(&id)
    }

    pub fn add_problem(&mut self, name: &str, description: &str) -> ProblemId {
        let id = ProblemId::new();
        self.problems.insert(
            id,
            Problem {
                name: name.to_owned(),
                description: description.to_owned(),
                submissions: Vec::new(),
                submissions_per_user: HashMap::new(),
            },
        );
        id
    }
}

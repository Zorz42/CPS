use crate::id::GenericId;
use crate::problem::ProblemId;
use crate::user::UserId;
use std::collections::HashMap;

pub type ContestId = GenericId;

#[derive(Clone)]
pub struct Contest {
    pub name: String,
    pub problems: Vec<ProblemId>,
}

impl Contest {
    pub fn new(name: &str) -> Contest {
        Contest {
            name: name.to_owned(),
            problems: Vec::new(),
        }
    }
}

pub struct ContestDatabase {
    contests: HashMap<ContestId, Contest>,
    available_contests: HashMap<UserId, Vec<ContestId>>,
}

impl ContestDatabase {
    pub fn new() -> ContestDatabase {
        ContestDatabase {
            contests: HashMap::new(),
            available_contests: HashMap::new(),
        }
    }

    pub fn add_contest(&mut self, name: &str) -> ContestId {
        let id = ContestId::new();
        self.contests.insert(id, Contest::new(name));
        id
    }

    pub fn add_problem_to_contest(&mut self, contest_id: ContestId, problem_id: ProblemId) {
        self.contests
            .get_mut(&contest_id)
            .unwrap()
            .problems
            .push(problem_id);
    }

    pub fn make_contest_available(&mut self, user_id: UserId, contest_id: ContestId) {
        self.available_contests
            .entry(user_id)
            .or_insert_with(Vec::new)
            .push(contest_id);
    }

    pub fn get_available_contests(&self, user_id: UserId) -> Vec<ContestId> {
        self.available_contests
            .get(&user_id)
            .cloned()
            .unwrap_or(Vec::new())
    }

    pub fn get_contest(&self, id: ContestId) -> Option<&Contest> {
        self.contests.get(&id)
    }
}

use crate::id::GenericId;
use crate::problem::ProblemId;
use crate::user::UserId;
use std::collections::HashMap;

type ContestId = GenericId;

pub struct Contest {
    name: String,
    problems: Vec<ProblemId>,
}

impl Contest {
    pub fn new(name: &str, problems: Vec<ProblemId>) -> Contest {
        Contest {
            name: name.to_owned(),
            problems,
        }
    }
}

pub struct ContestDatabase {
    contests: HashMap<ContestId, Contest>,
    available_contests: HashMap<UserId, Vec<ContestId>>, // available contests for each user
}

impl ContestDatabase {
    pub fn new() -> ContestDatabase {
        ContestDatabase {
            contests: HashMap::new(),
            available_contests: HashMap::new(),
        }
    }

    pub fn add_contest(&mut self, name: &str, problems: Vec<ProblemId>) -> ContestId {
        let id = ContestId::new();
        self.contests.insert(id, Contest::new(name, problems));
        id
    }
}

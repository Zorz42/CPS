use std::collections::HashMap;

pub struct Contest {
    name: String,
    problems: Vec<u32>,
}

pub struct ContestDatabase {
    contests: HashMap<u128, Contest>,
    available_contests: HashMap<u128, Vec<u32>>, // available contests for each user
}

impl ContestDatabase {
    pub fn new() -> ContestDatabase {
        ContestDatabase {
            contests: HashMap::new(),
            available_contests: HashMap::new(),
        }
    }
}

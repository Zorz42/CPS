use crate::id::GenericId;
use crate::problem::ProblemId;
use crate::user::UserId;
use crate::{create_html_response, GlobalState};
use anyhow::Result;
use askama::Template;
use http_body_util::Full;
use hyper::body::Bytes;
use hyper::Response;
use std::collections::HashMap;

pub type ContestId = GenericId;

#[derive(Template)]
#[template(path = "contest.html")]
pub struct ContestSite {
    contest_id: u128,
    problems: Vec<(u128, String)>,
}

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

pub fn create_contest_page(
    global: &GlobalState,
    contest_id: &str,
) -> Result<Option<Response<Full<Bytes>>>> {
    if let Some(contest_id) = contest_id.parse::<u128>().ok() {
        let contest_id = ContestId::from_int(contest_id);
        let contest = global.contests().get_contest(contest_id).cloned();
        if let Some(contest) = contest {
            let mut problems = Vec::new();
            for problem_id in contest.problems {
                let problem = global.problems().get_problem(problem_id).unwrap().clone();
                problems.push((problem_id.to_int(), problem.name.clone()));
            }

            return Ok(Some(create_html_response(ContestSite {
                contest_id: contest_id.to_int(),
                problems,
            })?));
        }
    }
    Ok(None)
}

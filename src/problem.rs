use crate::contest::ContestId;
use crate::id::GenericId;
use crate::submission::SubmissionId;
use crate::user::UserId;
use crate::{create_html_response, GlobalState};
use anyhow::Result;
use askama::Template;
use http_body_util::Full;
use hyper::body::Bytes;
use hyper::Response;
use std::collections::HashMap;

pub type ProblemId = GenericId;

#[derive(Template)]
#[template(path = "problem.html")]
pub struct ProblemSite {
    contest_id: u128,
    problem_id: u128,
    problem_name: String,
}

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

pub fn create_problem_page(
    global: &GlobalState,
    contest_id: &str,
    problem_id: &str,
) -> Result<Option<Response<Full<Bytes>>>> {
    if let (Some(contest_id), Some(problem_id)) = (
        contest_id.parse::<u128>().ok(),
        problem_id.parse::<u128>().ok(),
    ) {
        let contest_id = ContestId::from_int(contest_id);
        let problem_id = ProblemId::from_int(problem_id);
        let contest = global.contests().get_contest(contest_id).cloned();
        let problem = global.problems().get_problem(problem_id).cloned();
        if let (Some(_contest), Some(problem)) = (contest, problem) {
            return Ok(Some(create_html_response(ProblemSite {
                contest_id: contest_id.to_int(),
                problem_id: problem_id.to_int(),
                problem_name: problem.name.clone(),
            })?));
        }
    }

    Ok(None)
}

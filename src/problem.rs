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
    submissions: Vec<u128>,
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

    pub fn add_submission(
        &mut self,
        problem_id: ProblemId,
        submission_id: SubmissionId,
        user_id: UserId,
    ) {
        self.problems
            .get_mut(&problem_id)
            .unwrap()
            .submissions
            .push(submission_id);
        self.problems
            .get_mut(&problem_id)
            .unwrap()
            .submissions_per_user
            .entry(user_id)
            .or_insert_with(Vec::new)
            .push(submission_id);
    }
}

pub fn create_problem_page(
    global: &GlobalState,
    contest_id: &str,
    problem_id: &str,
    user_id: Option<UserId>,
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
            let submissions = if let Some(user_id) = user_id {
                problem
                    .submissions_per_user
                    .get(&user_id)
                    .cloned()
                    .unwrap_or(Vec::new())
            } else {
                Vec::new()
            };

            let submissions = submissions
                .iter()
                .map(|id| id.to_int())
                .collect::<Vec<u128>>();

            return Ok(Some(create_html_response(ProblemSite {
                contest_id: contest_id.to_int(),
                problem_id: problem_id.to_int(),
                problem_name: problem.name.clone(),
                submissions,
            })?));
        }
    }

    Ok(None)
}

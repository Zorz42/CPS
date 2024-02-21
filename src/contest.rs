use crate::database::contest::ContestId;
use crate::database::problem::ProblemId;
use crate::database::Database;
use crate::request_handler::create_html_response;
use anyhow::Result;
use askama::Template;
use http_body_util::Full;
use hyper::body::Bytes;
use hyper::Response;

#[derive(Template)]
#[template(path = "contest.html")]
pub struct ContestSite {
    contest_id: ContestId,
    problems: Vec<(ProblemId, String)>,
}

pub async fn create_contest_page(
    database: &Database,
    contest_id: &str,
) -> Result<Option<Response<Full<Bytes>>>> {
    if let Some(contest_id) = contest_id.parse::<i32>().ok() {
        if database.is_contest_id_valid(contest_id).await {
            let mut problems = Vec::new();
            for problem_id in database.get_problems_for_contest(contest_id).await {
                problems.push((
                    problem_id,
                    database.get_problem_name(problem_id).await.clone(),
                ));
            }

            return Ok(Some(create_html_response(ContestSite {
                contest_id,
                problems,
            })?));
        }
    }
    Ok(None)
}

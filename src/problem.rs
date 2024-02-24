use crate::database::contest::ContestId;
use crate::database::problem::ProblemId;
use crate::database::submission::SubmissionId;
use crate::database::user::UserId;
use crate::database::Database;
use crate::request_handler::create_html_response;
use crate::sidebar::{create_sidebar_context, SidebarContext};
use anyhow::Result;
use askama::Template;
use http_body_util::Full;
use hyper::body::Bytes;
use hyper::Response;

#[derive(Template)]
#[template(path = "problem.html")]
pub struct ProblemSite {
    contest_id: ContestId,
    problem_id: ProblemId,
    problem_name: String,
    problem_description: String,
    submissions: Vec<(SubmissionId, String)>,
    sidebar_context: SidebarContext,
}

pub async fn create_problem_page(database: &Database, contest_id: &str, problem_id: &str, user_id: Option<UserId>) -> Result<Option<Response<Full<Bytes>>>> {
    if let (Some(contest_id), Some(problem_id)) = (contest_id.parse::<ContestId>().ok(), problem_id.parse::<ProblemId>().ok()) {
        if !database.is_contest_id_valid(contest_id).await {
            return Ok(None);
        }

        if !database.is_problem_id_valid(problem_id).await {
            return Ok(None);
        }

        let submissions = if let Some(user_id) = user_id {
            let mut res = Vec::new();
            for id in database.get_submissions_by_user_for_problem(user_id, problem_id).await? {
                let total_points = database.get_problem_total_points(problem_id).await?;
                let score = database
                    .get_submission_points(id)
                    .await?
                    .map_or_else(|| "Testing...".to_owned(), |points| format!("{points}/{total_points}"));
                res.push((id, score));
            }
            res
        } else {
            Vec::new()
        };

        let problem_description = database.get_problem_description(problem_id).await?;

        return Ok(Some(create_html_response(&ProblemSite {
            contest_id,
            problem_id,
            problem_description,
            problem_name: database.get_problem_name(problem_id).await?,
            submissions,
            sidebar_context: create_sidebar_context(database, user_id).await?,
        })?));
    }

    Ok(None)
}

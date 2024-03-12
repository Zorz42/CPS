use crate::database::contest::ContestId;
use crate::database::problem::ProblemId;
use crate::database::user::UserId;
use crate::database::Database;
use crate::request_handler::{create_html_response, RedirectSite};
use crate::sidebar::{create_sidebar_context, SidebarContext};
use crate::user::parse_body;
use anyhow::Result;
use askama::Template;
use http_body_util::BodyExt;
use http_body_util::Full;
use hyper::body::{Bytes, Incoming};
use hyper::{Request, Response};

#[derive(Template)]
#[template(path = "contest.html")]
pub struct ContestSite {
    contest_name: String,
    is_admin: bool,
    users: Vec<(String, UserId, bool)>,
    contest_id: ContestId,
    sidebar_context: SidebarContext,
    problems: Vec<(String, ProblemId)>,
}

pub async fn create_contest_page(database: &Database, contest_id: &str, user: UserId) -> Result<Option<Response<Full<Bytes>>>> {
    if let Ok(contest_id) = contest_id.parse::<i32>() {
        if database.is_contest_id_valid(contest_id).await {
            let contest_name = database.get_contest_name(contest_id).await?;
            let is_admin = database.is_user_admin(user).await?;

            let mut users = Vec::new();
            for user_id in database.get_contests_for_user(user).await? {
                if let Some(username) = database.get_username(user_id).await? {
                    let is_in_contest = database.is_user_in_contest(user_id, contest_id).await?;
                    users.push((username, user_id, is_in_contest));
                }
            }

            let mut problems = Vec::new();
            for problem_id in database.get_problems_for_contest(contest_id).await? {
                let problem_name = database.get_problem_name(problem_id).await?;
                problems.push((problem_name, problem_id));
            }

            return Ok(Some(create_html_response(&ContestSite {
                contest_name,
                is_admin,
                users,
                contest_id,
                sidebar_context: create_sidebar_context(database, Some(user)).await?,
                problems,
            })?));
        }
    }
    Ok(None)
}

pub async fn handle_participant_modification(database: &Database, contest_id: &str, user: UserId, request: Request<Incoming>) -> Result<Option<Response<Full<Bytes>>>> {
    if let Ok(contest_id) = contest_id.parse::<i32>() {
        if database.is_contest_id_valid(contest_id).await && database.is_user_admin(user).await? {
            let body = request.into_body().collect().await?.to_bytes();
            let body = String::from_utf8_lossy(&body).to_string();
            let parsed_body = parse_body(&body);

            let all_users = database.get_all_users().await?;
            for user_id in all_users {
                if let Some(action) = parsed_body.get(&format!("user_{user_id}")) {
                    if action == "on" {
                        database.add_user_to_contest(user_id, contest_id).await?;
                    } else {
                        database.remove_user_from_contest(user_id, contest_id).await?;
                    }
                }
            }
            return Ok(Some(create_html_response(&RedirectSite {
                url: format!("/contest/{contest_id}"),
            })?));
        }
    }
    Ok(None)
}

pub async fn handle_problem_deletion_from_contest(database: &Database, contest_id: &str, problem_id: &str) -> Result<Response<Full<Bytes>>> {
    let contest_id = contest_id.parse::<i32>()?;
    let problem_id = problem_id.parse::<i32>()?;

    database.remove_problem_from_contest(contest_id, problem_id).await?;

    Ok(create_html_response(&RedirectSite {
        url: format!("/contest/{contest_id}"),
    })?)
}

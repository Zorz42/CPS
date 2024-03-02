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
#[template(path = "contest.html")]
pub struct ContestSite {
    contest_name: String,
    sidebar_context: SidebarContext,
}

pub async fn create_contest_page(database: &Database, contest_id: &str, user: UserId) -> Result<Option<Response<Full<Bytes>>>> {
    if let Ok(contest_id) = contest_id.parse::<i32>() {
        if database.is_contest_id_valid(contest_id).await {
            let contest_name = database.get_contest_name(contest_id).await?;

            return Ok(Some(create_html_response(&ContestSite {
                contest_name,
                sidebar_context: create_sidebar_context(database, Some(user)).await?,
            })?));
        }
    }
    Ok(None)
}

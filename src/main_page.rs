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
#[template(path = "main.html")]
pub struct MainSite {
    sidebar_context: SidebarContext,
    users: Vec<(String, UserId)>,
    is_admin: bool,
}

pub async fn create_main_page(database: &Database, user: Option<UserId>) -> Result<Response<Full<Bytes>>> {
    let is_admin = if let Some(user) = user { database.is_user_admin(user).await? } else { false };
    let user_ids = if is_admin { database.get_all_users().await? } else { vec![] };
    let mut users = vec![];
    for user_id in user_ids {
        if let Some(username) = database.get_username(user_id).await? {
            users.push((username, user_id));
        }
    }

    create_html_response(&MainSite {
        sidebar_context: create_sidebar_context(database, user).await?,
        users,
        is_admin,
    })
}

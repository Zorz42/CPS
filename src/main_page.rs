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
}

pub async fn create_main_page(database: &Database, user: Option<UserId>) -> Result<Response<Full<Bytes>>> {
    create_html_response(&MainSite {
        sidebar_context: create_sidebar_context(database, user).await?,
    })
}

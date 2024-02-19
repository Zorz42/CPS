use crate::database::Database;
use crate::request_handler::create_html_response;
use crate::user::UserId;
use askama::Template;
use http_body_util::Full;
use hyper::body::Bytes;
use hyper::Response;

#[derive(Template)]
#[template(path = "main.html")]
pub struct MainSite {
    logged_in: bool,
    username: String,
    contests: Vec<(i32, String)>,
}

pub async fn create_main_page(
    database: &Database,
    user: Option<UserId>,
) -> anyhow::Result<Response<Full<Bytes>>> {
    let mut contests = Vec::new();
    if let Some(user) = user {
        for id in database.get_contests_for_user(user).await {
            contests.push((id, database.get_contest_name(id).await));
        }
    }

    let username = if let Some(user) = user {
        database.get_username(user).await?.unwrap_or_default()
    } else {
        "".to_owned()
    };

    Ok(create_html_response(MainSite {
        logged_in: user.is_some(),
        username,
        contests,
    })?)
}

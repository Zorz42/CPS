use crate::database::user::{parse_login_string, UserToken};
use crate::database::Database;
use crate::request_handler::{create_html_response, RedirectSite};
use anyhow::{anyhow, Result};
use askama::Template;
use http_body_util::BodyExt;
use http_body_util::Full;
use hyper::body::{Bytes, Incoming};
use hyper::header::SET_COOKIE;
use hyper::{Request, Response};

#[derive(Template)]
#[template(path = "login.html")]
pub struct LoginSite {
    error_message: String,
}

pub fn get_login_token(request: &Request<Incoming>) -> Result<Option<UserToken>> {
    /*request.headers().get("cookie").and_then(|cookie| cookie.to_str().ok()).and_then(|cookie| {
        for part in cookie.split(';') {
            let parts: Vec<&str> = part.trim().split('=').collect();
            if parts.len() != 2 {
                continue;
            }

            if parts[0] == "login_token" {
                return Some(parts[1].parse().unwrap_or("Invalid Token").to_owned()
            }
        }

        Ok(None)
    })*/

    let cookie = request.headers().get("cookie");
    if let Some(cookie) = cookie {
        let cookie = cookie.to_str()?;
        for part in cookie.split(';') {
            let parts: Vec<&str> = part.trim().split('=').collect();
            if parts.len() != 2 {
                continue;
            }

            if parts.first().ok_or_else(|| anyhow!("Impossible error"))? == &"login_token" {
                return Ok(Some(parts.get(1).ok_or_else(|| anyhow!("Impossible error"))?.parse().unwrap_or_else(|_| "Invalid Token".to_owned())));
            }
        }
    }
    Ok(None)
}

pub async fn handle_login_form(database: &Database, request: Request<Incoming>) -> Result<Response<Full<Bytes>>> {
    let body = request.into_body().collect().await?.to_bytes();
    let body = String::from_utf8_lossy(&body).to_string();
    let (username, password) = parse_login_string(&body);

    return if let Some(id) = database.try_login(&username, &password).await? {
        let token = database.add_token(id).await?;

        let mut response = create_html_response(&RedirectSite { url: "/".to_owned() })?;

        response.headers_mut().append(SET_COOKIE, format!("login_token={token}").parse()?);

        Ok(response)
    } else {
        let error_message = {
            if database.get_user_from_username(&username).await?.is_none() {
                "User does not exist".to_owned()
            } else {
                "Invalid password".to_owned()
            }
        };

        let response = create_html_response(&LoginSite { error_message })?;

        Ok(response)
    };
}

pub async fn handle_logout_form(database: &Database, token: Option<UserToken>) -> Result<Response<Full<Bytes>>> {
    let response = create_html_response(&RedirectSite { url: "/".to_owned() })?;

    if let Some(token) = token {
        database.remove_token(token).await?;
    }

    Ok(response)
}

pub fn create_login_page() -> Result<Response<Full<Bytes>>> {
    create_html_response(&LoginSite { error_message: String::new() })
}

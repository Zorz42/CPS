use crate::database::user::UserToken;
use crate::database::Database;
use crate::request_handler::{create_html_response, RedirectSite};
use anyhow::{anyhow, Result};
use askama::Template;
use http_body_util::BodyExt;
use http_body_util::Full;
use hyper::body::{Bytes, Incoming};
use hyper::header::SET_COOKIE;
use hyper::{Request, Response};
use std::collections::HashMap;

#[derive(Template)]
#[template(path = "login.html")]
pub struct LoginSite {
    pub error_message: String,
}

pub fn parse_body(body: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();

    for part in body.split('&') {
        let parts: Vec<&str> = part.split('=').collect();
        if parts.len() != 2 {
            continue;
        }

        map.insert(parts.first().unwrap_or(&"").to_owned().to_owned(), parts.get(1).unwrap_or(&"").to_owned().to_owned());
    }

    map
}

pub fn get_login_token(request: &Request<Incoming>) -> Result<Option<UserToken>> {
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
    let parsed_body = parse_body(&body);
    let username = parsed_body.get("username").ok_or_else(|| anyhow!("Username not found"))?;
    let password = parsed_body.get("password").ok_or_else(|| anyhow!("Password not found"))?;

    return if let Some(id) = database.try_login(username, password).await? {
        let token = database.add_token(id).await?;

        let mut response = create_html_response(&RedirectSite { url: "/".to_owned() })?;

        response.headers_mut().append(SET_COOKIE, format!("login_token={token}").parse()?);

        println!("Logged in with username: \"{username}\"");

        Ok(response)
    } else {
        let error_message = {
            if database.get_user_from_username(username).await?.is_none() {
                "User does not exist".to_owned()
            } else {
                "Invalid password".to_owned()
            }
        };

        println!("Failed to log in: {error_message}, with username: \"{username}\" and password: \"{password}\"");

        let response = create_html_response(&LoginSite { error_message })?;

        Ok(response)
    };
}

pub async fn handle_logout_form(database: &Database, token: Option<UserToken>) -> Result<Response<Full<Bytes>>> {
    println!("Logged out");

    let response = create_html_response(&RedirectSite { url: "/".to_owned() })?;

    if let Some(token) = token {
        database.remove_token(token).await?;
    }

    Ok(response)
}

pub fn create_login_page() -> Result<Response<Full<Bytes>>> {
    create_html_response(&LoginSite { error_message: String::new() })
}

pub async fn handle_user_creation(database: &Database, request: Request<Incoming>, is_admin: bool) -> Result<Response<Full<Bytes>>> {
    if !is_admin {
        return create_html_response(&LoginSite {
            error_message: "You must be an admin to perform this action".to_owned(),
        });
    }

    let response = create_html_response(&RedirectSite { url: "/".to_owned() })?;

    let body = request.into_body().collect().await?.to_bytes();
    let body = String::from_utf8_lossy(&body).to_string();
    let parsed_body = parse_body(&body);

    let username = parsed_body.get("username").ok_or_else(|| anyhow!("Username not found"))?;
    let password = parsed_body.get("password").ok_or_else(|| anyhow!("Password not found"))?;
    let is_admin = parsed_body.get("is_admin").is_some_and(|x| x == "on");

    database.add_user(username, password, is_admin).await?;

    Ok(response)
}

pub async fn delete_user(database: &Database, user_id: &str) -> Result<Response<Full<Bytes>>> {
    let user_id = user_id.parse()?;

    database.delete_user(user_id).await?;
    create_html_response(&RedirectSite { url: "/".to_owned() })
}

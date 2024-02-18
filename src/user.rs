use crate::id::GenericId;
use crate::{create_html_response, GlobalState, RedirectSite};
use anyhow::Result;
use askama::Template;
use bcrypt::{hash, verify, DEFAULT_COST};
use http_body_util::BodyExt;
use http_body_util::Full;
use hyper::body::{Bytes, Incoming};
use hyper::header::SET_COOKIE;
use hyper::{Request, Response};
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::Instant;

const TOKEN_EXPIRY: Duration = Duration::from_secs(60 * 60);

pub type UserId = GenericId;
pub type UserToken = GenericId;

#[derive(Template)]
#[template(path = "login.html")]
pub struct LoginSite {
    error_message: String,
}

pub struct User {
    pub username: String,
    pub password: String,
    pub is_admin: bool,
}

impl User {
    fn new(username: &str, password: &str, is_admin: bool) -> User {
        User {
            username: username.to_owned(),
            password: hash(password, DEFAULT_COST).unwrap(),
            is_admin,
        }
    }
}

pub struct UserDatabase {
    users: HashMap<UserId, User>,
    usernames: HashMap<String, UserId>,
    tokens: HashMap<UserToken, UserId>,
    token_expiry: HashMap<UserToken, Instant>,
}

impl UserDatabase {
    pub fn new() -> UserDatabase {
        UserDatabase {
            users: HashMap::new(),
            usernames: HashMap::new(),
            tokens: HashMap::new(),
            token_expiry: HashMap::new(),
        }
    }

    pub fn add_user(&mut self, username: &str, password: &str, is_admin: bool) -> UserId {
        let id = UserId::new();
        self.users
            .insert(id, User::new(username, password, is_admin));
        self.usernames.insert(username.to_string(), id);
        id
    }

    pub fn get_user(&self, id: UserId) -> Option<&User> {
        self.users.get(&id)
    }

    pub fn remove_user(&mut self, id: UserId) -> Option<User> {
        self.usernames.remove(&self.users[&id].username);
        self.users.remove(&id)
    }

    pub fn add_token(&mut self, user_id: UserId) -> UserToken {
        let token = UserToken::new();
        self.tokens.insert(token, user_id);
        self.token_expiry
            .insert(token, Instant::now() + TOKEN_EXPIRY);
        token
    }

    pub fn get_user_id_by_token(&mut self, token: UserToken) -> Option<UserId> {
        let expired = if let Some(expiry) = self.token_expiry.get(&token) {
            Instant::now() > *expiry
        } else {
            false
        };

        if expired {
            self.tokens.remove(&token);
            self.token_expiry.remove(&token);
            return None;
        }

        self.tokens.get(&token).copied()
    }

    pub fn remove_token(&mut self, token: UserToken) {
        self.tokens.remove(&token);
    }

    pub fn get_user_id_by_username(&self, username: &str) -> Option<UserId> {
        self.usernames.get(username).copied()
    }

    pub fn try_login(&self, username: &str, password: &str) -> Option<UserId> {
        if let Some(id) = self.get_user_id_by_username(username) {
            if let Some(user) = self.get_user(id) {
                if verify(password, &user.password).unwrap_or(false) {
                    return Some(id);
                }
            }
        }
        None
    }

    pub fn is_admin(&self, id: UserId) -> bool {
        self.get_user(id).map(|user| user.is_admin).unwrap_or(false)
    }
}

pub fn parse_login_string(body: &str) -> (String, String) {
    let mut username = String::new();
    let mut password = String::new();

    for part in body.split('&') {
        let parts: Vec<&str> = part.split('=').collect();
        if parts.len() != 2 {
            continue;
        }

        match parts[0] {
            "username" => username = parts[1].to_string(),
            "password" => password = parts[1].to_string(),
            _ => {}
        }
    }

    (username, password)
}

pub fn get_login_token(request: &Request<hyper::body::Incoming>) -> Option<UserToken> {
    request
        .headers()
        .get("cookie")
        .and_then(|cookie| cookie.to_str().ok())
        .and_then(|cookie| {
            for part in cookie.split(';') {
                let parts: Vec<&str> = part.trim().split('=').collect();
                if parts.len() != 2 {
                    continue;
                }

                if parts[0] == "login_token" {
                    return Some(UserToken::from_int(parts[1].parse().unwrap_or(0)));
                }
            }

            None
        })
}

pub async fn handle_login_form(
    global: &GlobalState,
    request: Request<Incoming>,
) -> Result<Response<Full<Bytes>>> {
    let body = request.into_body().collect().await?.to_bytes();
    let body = String::from_utf8_lossy(&body).to_string();
    let (username, password) = parse_login_string(&body);

    let mut users = global.users();
    return if let Some(id) = users.try_login(&username, &password) {
        let token = users.add_token(id);

        let mut response = create_html_response(RedirectSite {
            url: "/".to_owned(),
        })?;

        response.headers_mut().append(
            SET_COOKIE,
            format!("login_token={}", token.to_int()).parse()?,
        );

        Ok(response)
    } else {
        let error_message = {
            if users.get_user_id_by_username(&username).is_none() {
                "User does not exist".to_owned()
            } else {
                "Invalid password".to_owned()
            }
        };

        let response = create_html_response(LoginSite { error_message })?;

        Ok(response)
    };
}

pub async fn handle_logout_form(
    global: &GlobalState,
    token: Option<UserToken>,
) -> Result<Response<Full<Bytes>>> {
    let response = create_html_response(RedirectSite {
        url: "/".to_owned(),
    })?;

    if let Some(token) = token {
        global.users().remove_token(token);
    }

    return Ok(response);
}

pub fn create_login_page() -> Result<Response<Full<Bytes>>> {
    Ok(create_html_response(LoginSite {
        error_message: "".to_owned(),
    })?)
}

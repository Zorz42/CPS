use crate::database::Database;
use crate::request_handler::{create_html_response, RedirectSite};
use anyhow::{bail, Result};
use askama::Template;
use bcrypt::{hash, verify, DEFAULT_COST};
use http_body_util::BodyExt;
use http_body_util::Full;
use hyper::body::{Bytes, Incoming};
use hyper::header::SET_COOKIE;
use hyper::{Request, Response};
use rand::distributions::Alphanumeric;
use rand::Rng;
use std::time::Duration;

const TOKEN_EXPIRY: Duration = Duration::from_secs(60 * 60 * 24);
const TOKEN_LENGTH: usize = 255;

pub type UserId = i32;
pub type UserToken = String;

#[derive(Template)]
#[template(path = "login.html")]
pub struct LoginSite {
    error_message: String,
}

impl Database {
    pub async fn init_users(&self) {
        // create the users table
        self.get_postgres_client()
            .execute(
                "CREATE TABLE IF NOT EXISTS users (
                    user_id SERIAL PRIMARY KEY,
                    username VARCHAR(50) UNIQUE NOT NULL,
                    password VARCHAR(100) NOT NULL,
                    is_admin BOOLEAN NOT NULL
                );",
                &[],
            )
            .await
            .unwrap();

        // create the tokens table
        self.get_postgres_client()
            .execute(
                &format!(
                    "CREATE TABLE IF NOT EXISTS tokens (
                    token VARCHAR({TOKEN_LENGTH}) NOT NULL,
                    expiration_date TIMESTAMPTZ NOT NULL,
                    user_id INT REFERENCES users(user_id)
                );"
                ),
                &[],
            )
            .await
            .unwrap();
    }

    pub async fn get_user_from_username(&self, username: &str) -> Result<Option<UserId>> {
        let rows = self
            .get_postgres_client()
            .query(
                "SELECT user_id FROM users WHERE username = $1",
                &[&username],
            )
            .await?;
        if rows.is_empty() {
            return Ok(None);
        }
        Ok(rows[0].get(0))
    }

    pub async fn add_user(&self, username: &str, password: &str, is_admin: bool) -> Result<UserId> {
        let hashed_password = hash(password, DEFAULT_COST)?;

        // create the user and return the user_id
        let rows = self.get_postgres_client()
            .query(
                "INSERT INTO users (username, password, is_admin) VALUES ($1, $2, $3) RETURNING user_id",
                &[&username, &hashed_password, &is_admin],
            ).await?;

        Ok(rows[0].get(0))
    }

    pub async fn delete_user(&self, user_id: UserId) {
        self.delete_all_tokens_for_user(user_id).await;
        self.remove_user_from_all_contests(user_id).await;
        self.delete_all_submissions_for_user(user_id).await;
        self.get_postgres_client()
            .execute("DELETE FROM users WHERE user_id = $1", &[&user_id])
            .await
            .unwrap();
    }

    pub async fn delete_all_tokens_for_user(&self, user_id: UserId) {
        self.get_postgres_client()
            .execute("DELETE FROM tokens WHERE user_id = $1", &[&user_id])
            .await
            .unwrap();
    }

    pub async fn add_user_override(
        &self,
        username: &str,
        password: &str,
        is_admin: bool,
    ) -> Result<UserId> {
        if let Some(user_id) = self.get_user_from_username(username).await? {
            self.delete_user(user_id).await;
        }

        self.add_user(username, password, is_admin).await
    }

    pub async fn try_login(&self, username: &str, password: &str) -> Result<Option<UserId>> {
        let user_id = self.get_user_from_username(username).await?;
        let user_id = match user_id {
            Some(user_id) => user_id,
            None => return Ok(None),
        };

        let hashed_password = self
            .get_postgres_client()
            .query("SELECT password FROM users WHERE user_id = $1", &[&user_id])
            .await?;

        if hashed_password.is_empty() {
            bail!("User does not have a password");
        }

        let hashed_password = hashed_password[0].get(0);

        Ok(if verify(password, hashed_password)? {
            Some(user_id)
        } else {
            None
        })
    }

    pub async fn get_username(&self, user_id: UserId) -> Result<Option<String>> {
        let rows = self
            .get_postgres_client()
            .query("SELECT username FROM users WHERE user_id = $1", &[&user_id])
            .await?;
        if rows.is_empty() {
            return Ok(None);
        }
        Ok(Some(rows[0].get(0)))
    }

    pub async fn add_token(&self, user_id: UserId) -> UserToken {
        let token: UserToken = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(255)
            .map(char::from)
            .collect();
        let expiration_date = chrono::Utc::now() + TOKEN_EXPIRY;
        self.get_postgres_client()
            .execute(
                "INSERT INTO tokens (token, expiration_date, user_id) VALUES ($1, $2, $3)",
                &[&token, &expiration_date, &user_id],
            )
            .await
            .unwrap();
        token
    }

    pub async fn remove_token(&self, token: UserToken) {
        self.get_postgres_client()
            .execute("DELETE FROM tokens WHERE token = $1", &[&token])
            .await
            .unwrap();
    }

    pub async fn get_user_from_token(&self, token: UserToken) -> Result<Option<UserId>> {
        let rows = self
            .get_postgres_client()
            .query("SELECT user_id FROM tokens WHERE token = $1", &[&token])
            .await?;
        if rows.is_empty() {
            return Ok(None);
        }
        Ok(Some(rows[0].get(0)))
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

pub fn get_login_token(request: &Request<Incoming>) -> Option<UserToken> {
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
                    return Some(parts[1].parse().unwrap_or("Invalid Token".to_owned()));
                }
            }

            None
        })
}

pub async fn handle_login_form(
    database: &Database,
    request: Request<Incoming>,
) -> Result<Response<Full<Bytes>>> {
    let body = request.into_body().collect().await?.to_bytes();
    let body = String::from_utf8_lossy(&body).to_string();
    let (username, password) = parse_login_string(&body);

    return if let Some(id) = database.try_login(&username, &password).await? {
        let token = database.add_token(id).await;

        let mut response = create_html_response(RedirectSite {
            url: "/".to_owned(),
        })?;

        response
            .headers_mut()
            .append(SET_COOKIE, format!("login_token={}", token).parse()?);

        Ok(response)
    } else {
        let error_message = {
            if database.get_user_from_username(&username).await?.is_none() {
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
    database: &Database,
    token: Option<UserToken>,
) -> Result<Response<Full<Bytes>>> {
    let response = create_html_response(RedirectSite {
        url: "/".to_owned(),
    })?;

    if let Some(token) = token {
        database.remove_token(token).await;
    }

    return Ok(response);
}

pub fn create_login_page() -> Result<Response<Full<Bytes>>> {
    Ok(create_html_response(LoginSite {
        error_message: "".to_owned(),
    })?)
}

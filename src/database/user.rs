use crate::database::Database;
use anyhow::{bail, Result};
use bcrypt::{hash, verify, DEFAULT_COST};
use rand::distributions::Alphanumeric;
use rand::Rng;
use std::time::Duration;

const TOKEN_EXPIRY: Duration = Duration::from_secs(60 * 60 * 24);
const TOKEN_LENGTH: usize = 255;

pub type UserId = i32;
pub type UserToken = String;

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

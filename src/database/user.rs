use crate::database::{Database, DatabaseQuery};
use anyhow::anyhow;
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
    pub async fn init_users(&self) -> Result<()> {
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
            .await?;

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
            .await?;

        Ok(())
    }

    pub async fn get_user_from_username(&self, username: &str) -> Result<Option<UserId>> {
        static QUERY: DatabaseQuery = DatabaseQuery::new("SELECT user_id FROM users WHERE username = $1");

        let rows = QUERY.execute(self, &[&username]).await?;
        if rows.is_empty() {
            return Ok(None);
        }
        Ok(rows.first().ok_or_else(|| anyhow!("Error getting first row"))?.get(0))
    }

    pub async fn add_user(&self, username: &str, password: &str, is_admin: bool) -> Result<UserId> {
        static QUERY: DatabaseQuery = DatabaseQuery::new("INSERT INTO users (username, password, is_admin) VALUES ($1, $2, $3) RETURNING user_id");

        let hashed_password = hash(password, DEFAULT_COST)?;

        // create the user and return the user_id
        let rows = QUERY.execute(self, &[&username, &hashed_password, &is_admin]).await?;

        Ok(rows.first().ok_or_else(|| anyhow!("Could not retrieve the first row"))?.get(0))
    }

    pub async fn delete_user(&self, user_id: UserId) -> Result<()> {
        static QUERY: DatabaseQuery = DatabaseQuery::new("DELETE FROM users WHERE user_id = $1");

        self.delete_all_tokens_for_user(user_id).await?;
        self.remove_user_from_all_contests(user_id).await?;
        self.delete_all_submissions_for_user(user_id).await?;
        QUERY.execute(self, &[&user_id]).await?;

        Ok(())
    }

    pub async fn delete_all_tokens_for_user(&self, user_id: UserId) -> Result<()> {
        static QUERY: DatabaseQuery = DatabaseQuery::new("DELETE FROM tokens WHERE user_id = $1");
        QUERY.execute(self, &[&user_id]).await?;
        Ok(())
    }

    pub async fn add_user_override(&self, username: &str, password: &str, is_admin: bool) -> Result<UserId> {
        if let Some(user_id) = self.get_user_from_username(username).await? {
            self.delete_user(user_id).await?;
        }

        self.add_user(username, password, is_admin).await
    }

    pub async fn try_login(&self, username: &str, password: &str) -> Result<Option<UserId>> {
        static QUERY: DatabaseQuery = DatabaseQuery::new("SELECT password FROM users WHERE user_id = $1");

        let user_id = self.get_user_from_username(username).await?;
        let Some(user_id) = user_id else {
            return Ok(None);
        };

        let hashed_password = QUERY.execute(self, &[&user_id]).await?;

        if hashed_password.is_empty() {
            bail!("User does not have a password");
        }

        let hashed_password = hashed_password.first().ok_or_else(|| anyhow!("Error getting the hashed password"))?.get(0);

        Ok(verify(password, hashed_password)?.then_some(user_id))
    }

    pub async fn get_username(&self, user_id: UserId) -> Result<Option<String>> {
        static QUERY: DatabaseQuery = DatabaseQuery::new("SELECT username FROM users WHERE user_id = $1");

        let rows = QUERY.execute(self, &[&user_id]).await?;
        if rows.is_empty() {
            return Ok(None);
        }
        Ok(Some(rows.first().ok_or_else(|| anyhow!("Error getting the first column"))?.get(0)))
    }

    pub async fn add_token(&self, user_id: UserId) -> Result<UserToken> {
        static QUERY: DatabaseQuery = DatabaseQuery::new("INSERT INTO tokens (token, expiration_date, user_id) VALUES ($1, $2, $3)");

        let token: UserToken = rand::thread_rng().sample_iter(&Alphanumeric).take(255).map(char::from).collect();
        let expiration_date = chrono::Utc::now() + TOKEN_EXPIRY;
        QUERY.execute(self, &[&token, &expiration_date, &user_id]).await?;
        Ok(token)
    }

    pub async fn remove_token(&self, token: UserToken) -> Result<()> {
        static QUERY: DatabaseQuery = DatabaseQuery::new("DELETE FROM tokens WHERE token = $1");

        QUERY.execute(self, &[&token]).await?;
        Ok(())
    }

    pub async fn get_user_from_token(&self, token: UserToken) -> Result<Option<UserId>> {
        static QUERY: DatabaseQuery = DatabaseQuery::new("SELECT user_id FROM tokens WHERE token = $1");

        let rows = QUERY.execute(self, &[&token]).await?;
        if rows.is_empty() {
            return Ok(None);
        }
        Ok(Some(rows.first().ok_or_else(|| anyhow!("Error getting the first column"))?.get(0)))
    }

    pub async fn is_user_admin(&self, user_id: UserId) -> Result<bool> {
        static QUERY: DatabaseQuery = DatabaseQuery::new("SELECT is_admin FROM users WHERE user_id = $1");

        let rows = QUERY.execute(self, &[&user_id]).await?;
        if rows.is_empty() {
            bail!("User does not exist");
        }
        Ok(rows.first().ok_or_else(|| anyhow!("Error getting the first column"))?.get(0))
    }
}

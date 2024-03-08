use crate::database::user::UserId;
use crate::database::{Database, DatabaseQuery};
use anyhow::{anyhow, Result};

pub type ContestId = i32;

impl Database {
    pub async fn init_contests(&self) -> Result<()> {
        // add the contests table
        self.get_postgres_client()
            .execute(
                "CREATE TABLE IF NOT EXISTS contests (
                    contest_id SERIAL PRIMARY KEY,
                    contest_name VARCHAR(100) UNIQUE NOT NULL
                );",
                &[],
            )
            .await?;

        // add the table of contest participations
        self.get_postgres_client()
            .execute(
                "CREATE TABLE IF NOT EXISTS contest_participations (
                    contest_id INT REFERENCES contests(contest_id),
                    user_id INT REFERENCES users(user_id),
                    PRIMARY KEY (contest_id, user_id)
                );",
                &[],
            )
            .await?;

        Ok(())
    }

    pub async fn is_contest_id_valid(&self, contest_id: ContestId) -> bool {
        static QUERY: DatabaseQuery = DatabaseQuery::new("SELECT contest_id FROM contests WHERE contest_id = $1");

        QUERY.execute(self, &[&contest_id]).await.ok().is_some_and(|rows| !rows.is_empty())
    }

    pub async fn get_contest_name(&self, contest_id: ContestId) -> Result<String> {
        static QUERY: DatabaseQuery = DatabaseQuery::new("SELECT contest_name FROM contests WHERE contest_id = $1");

        Ok(QUERY
            .execute(self, &[&contest_id])
            .await?
            .first()
            .ok_or_else(|| anyhow::anyhow!("No contest with id {}", contest_id))?
            .get(0))
    }

    pub async fn get_all_contests(&self) -> Result<Vec<ContestId>> {
        static QUERY: DatabaseQuery = DatabaseQuery::new("SELECT contest_id FROM contests");

        Ok(QUERY.execute(self, &[]).await?.iter().map(|row| row.get(0)).collect())
    }

    pub async fn get_contests_for_user(&self, user_id: UserId) -> Result<Vec<ContestId>> {
        static QUERY: DatabaseQuery = DatabaseQuery::new("SELECT contest_id FROM contest_participations WHERE user_id = $1");

        if self.is_user_admin(user_id).await? {
            return self.get_all_contests().await;
        }

        Ok(QUERY.execute(self, &[&user_id]).await?.iter().map(|row| row.get(0)).collect())
    }

    pub async fn add_contest(&self, contest_name: &str) -> Result<ContestId> {
        static QUERY: DatabaseQuery = DatabaseQuery::new("INSERT INTO contests (contest_name) VALUES ($1) RETURNING contest_id");

        Ok(QUERY.execute(self, &[&contest_name]).await?.first().ok_or_else(|| anyhow!("Could not retrieve the first row"))?.get(0))
    }

    pub async fn remove_contest(&self, contest_id: ContestId) -> Result<()> {
        static QUERY: DatabaseQuery = DatabaseQuery::new("DELETE FROM contests WHERE contest_id = $1");

        self.remove_all_participations_for_contest(contest_id).await?;
        self.remove_all_problems_from_contest(contest_id).await?;

        QUERY.execute(self, &[&contest_id]).await?;

        Ok(())
    }

    pub async fn remove_all_participations_for_contest(&self, contest_id: ContestId) -> Result<()> {
        static QUERY: DatabaseQuery = DatabaseQuery::new("DELETE FROM contest_participations WHERE contest_id = $1");

        QUERY.execute(self, &[&contest_id]).await?;

        Ok(())
    }

    pub async fn get_contest_from_name(&self, contest_name: &str) -> Result<Option<ContestId>> {
        static QUERY: DatabaseQuery = DatabaseQuery::new("SELECT contest_id FROM contests WHERE contest_name = $1");

        let rows = QUERY.execute(self, &[&contest_name]).await?;
        if rows.is_empty() {
            return Ok(None);
        }
        Ok(Some(rows.first().ok_or_else(|| anyhow!("Error getting the first row"))?.get(0)))
    }

    pub async fn add_contest_override(&self, contest_name: &str) -> Result<ContestId> {
        if let Some(contest_id) = self.get_contest_from_name(contest_name).await? {
            self.remove_contest(contest_id).await?;
        }
        self.add_contest(contest_name).await
    }

    pub async fn add_user_to_contest(&self, user_id: UserId, contest_id: ContestId) -> Result<()> {
        static QUERY: DatabaseQuery = DatabaseQuery::new("INSERT INTO contest_participations (contest_id, user_id) VALUES ($1, $2)");

        if self.is_user_in_contest(user_id, contest_id).await? {
            return Ok(());
        }

        QUERY.execute(self, &[&contest_id, &user_id]).await?;
        Ok(())
    }

    pub async fn remove_user_from_contest(&self, user_id: UserId, contest_id: ContestId) -> Result<()> {
        static QUERY: DatabaseQuery = DatabaseQuery::new("DELETE FROM contest_participations WHERE user_id = $1 AND contest_id = $2");

        if !self.is_user_in_contest(user_id, contest_id).await? {
            return Ok(());
        }

        QUERY.execute(self, &[&user_id, &contest_id]).await?;
        Ok(())
    }

    pub async fn remove_user_from_all_contests(&self, user_id: UserId) -> Result<()> {
        static QUERY: DatabaseQuery = DatabaseQuery::new("DELETE FROM contest_participations WHERE user_id = $1");

        QUERY.execute(self, &[&user_id]).await?;
        Ok(())
    }

    pub async fn is_user_in_contest(&self, user_id: UserId, contest_id: ContestId) -> Result<bool> {
        static QUERY: DatabaseQuery = DatabaseQuery::new("SELECT user_id FROM contest_participations WHERE user_id = $1 AND contest_id = $2");

        Ok(!QUERY.execute(self, &[&user_id, &contest_id]).await?.is_empty())
    }
}

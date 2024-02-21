use crate::database::user::UserId;
use crate::database::Database;
use anyhow::Result;

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
        self.get_postgres_client()
            .query("SELECT contest_id FROM contests WHERE contest_id = $1", &[&contest_id])
            .await
            .ok()
            .map(|rows| !rows.is_empty())
            .unwrap_or(false)
    }

    pub async fn get_contest_name(&self, contest_id: ContestId) -> Result<String> {
        Ok(self
            .get_postgres_client()
            .query("SELECT contest_name FROM contests WHERE contest_id = $1", &[&contest_id])
            .await?
            .get(0)
            .ok_or_else(|| anyhow::anyhow!("No contest with id {}", contest_id))?
            .get(0))
    }

    pub async fn get_contests_for_user(&self, user_id: UserId) -> Result<Vec<ContestId>> {
        Ok(self
            .get_postgres_client()
            .query("SELECT contest_id FROM contest_participations WHERE user_id = $1", &[&user_id])
            .await?
            .iter()
            .map(|row| row.get(0))
            .collect())
    }

    pub async fn add_contest(&self, contest_name: &str) -> Result<ContestId> {
        Ok(self
            .get_postgres_client()
            .query_one("INSERT INTO contests (contest_name) VALUES ($1) RETURNING contest_id", &[&contest_name])
            .await?
            .get(0))
    }

    pub async fn remove_contest(&self, contest_id: ContestId) -> Result<()> {
        self.remove_all_participations_for_contest(contest_id).await;
        self.remove_all_problems_from_contest(contest_id).await;

        self.get_postgres_client().execute("DELETE FROM contests WHERE contest_id = $1", &[&contest_id]).await?;

        Ok(())
    }

    pub async fn remove_all_participations_for_contest(&self, contest_id: ContestId) -> Result<()> {
        self.get_postgres_client().execute("DELETE FROM contest_participations WHERE contest_id = $1", &[&contest_id]).await?;

        Ok(())
    }

    pub async fn get_contest_from_name(&self, contest_name: &str) -> Result<Option<ContestId>> {
        let rows = self.get_postgres_client().query("SELECT contest_id FROM contests WHERE contest_name = $1", &[&contest_name]).await?;
        if rows.is_empty() {
            return Ok(None);
        }
        Ok(Some(rows[0].get(0)))
    }

    pub async fn add_contest_override(&self, contest_name: &str) -> Result<ContestId> {
        if let Some(contest_id) = self.get_contest_from_name(contest_name).await? {
            self.remove_contest(contest_id).await?;
        }
        self.add_contest(contest_name).await
    }

    pub async fn add_user_to_contest(&self, user_id: UserId, contest_id: ContestId) -> Result<()> {
        self.get_postgres_client()
            .execute("INSERT INTO contest_participations (contest_id, user_id) VALUES ($1, $2)", &[&contest_id, &user_id])
            .await?;
        Ok(())
    }

    pub async fn remove_user_from_all_contests(&self, user_id: UserId) -> Result<()> {
        self.get_postgres_client().execute("DELETE FROM contest_participations WHERE user_id = $1", &[&user_id]).await?;
        Ok(())
    }
}

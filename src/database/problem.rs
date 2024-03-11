use crate::database::contest::ContestId;
use crate::database::user::UserId;
use crate::database::{Database, DatabaseQuery};
use anyhow::{anyhow, Result};

pub type ProblemId = i32;

impl Database {
    pub async fn init_problems(&self) -> Result<()> {
        self.get_postgres_client()
            .execute(
                "CREATE TABLE IF NOT EXISTS problems (
                    problem_id SERIAL PRIMARY KEY,
                    problem_name VARCHAR(100) UNIQUE NOT NULL,
                    problem_description TEXT NOT NULL,
                    points INT NOT NULL,
                    time_limit INT NOT NULL
                );",
                &[],
            )
            .await?;

        // add table of contest problems
        self.get_postgres_client()
            .execute(
                "CREATE TABLE IF NOT EXISTS contest_problems (
                    contest_id INT REFERENCES contests(contest_id),
                    problem_id INT REFERENCES problems(problem_id),
                    PRIMARY KEY (contest_id, problem_id)
                );",
                &[],
            )
            .await?;

        // add table of user scores for problems
        self.get_postgres_client()
            .execute(
                "CREATE TABLE IF NOT EXISTS user_problem_scores (
                    user_id INT REFERENCES users(user_id),
                    problem_id INT REFERENCES problems(problem_id),
                    score INT NOT NULL,
                    PRIMARY KEY (user_id, problem_id)
                );",
                &[],
            )
            .await?;

        Ok(())
    }

    pub async fn is_problem_id_valid(&self, problem_id: ProblemId) -> bool {
        static QUERY: DatabaseQuery = DatabaseQuery::new("SELECT problem_id FROM problems WHERE problem_id = $1");

        QUERY.execute(self, &[&problem_id]).await.ok().is_some_and(|rows| !rows.is_empty())
    }

    pub async fn get_problem_name(&self, problem_id: ProblemId) -> Result<String> {
        static QUERY: DatabaseQuery = DatabaseQuery::new("SELECT problem_name FROM problems WHERE problem_id = $1");

        Ok(QUERY.execute(self, &[&problem_id]).await?.first().ok_or_else(|| anyhow!("No problem with id {}", problem_id))?.get(0))
    }

    pub async fn get_problems_for_contest(&self, contest_id: ContestId) -> Result<Vec<ProblemId>> {
        static QUERY: DatabaseQuery = DatabaseQuery::new("SELECT problem_id FROM contest_problems WHERE contest_id = $1");

        let rows = QUERY.execute(self, &[&contest_id]).await?;

        let mut result = Vec::new();
        for row in rows {
            result.push(row.get(0));
        }

        Ok(result)
    }

    pub async fn add_problem(&self, problem_name: &str, problem_description: &str, time_limit: i32) -> Result<ProblemId> {
        static QUERY: DatabaseQuery = DatabaseQuery::new("INSERT INTO problems (problem_name, problem_description, points, time_limit) VALUES ($1, $2, $3, $4) RETURNING problem_id");

        Ok(QUERY
            .execute(self, &[&problem_name, &problem_description, &0, &time_limit])
            .await?
            .first()
            .ok_or_else(|| anyhow!("Could not retrieve the first row"))?
            .get(0))
    }

    pub async fn remove_problem(&self, problem_id: ProblemId) -> Result<()> {
        static QUERY: DatabaseQuery = DatabaseQuery::new("DELETE FROM problems WHERE problem_id = $1");

        self.delete_all_subtasks_and_tests_for_problem(problem_id).await?;

        QUERY.execute(self, &[&problem_id]).await?;

        Ok(())
    }

    pub async fn get_problem_id_from_name(&self, problem_name: &str) -> Result<ProblemId> {
        static QUERY: DatabaseQuery = DatabaseQuery::new("SELECT problem_id FROM problems WHERE problem_name = $1");

        Ok(QUERY
            .execute(self, &[&problem_name])
            .await?
            .first()
            .ok_or_else(|| anyhow!("No problem with name {}", problem_name))?
            .get(0))
    }

    pub async fn add_problem_override(&self, problem_name: &str, problem_description: &str, time_limit: i32) -> Result<ProblemId> {
        if let Ok(problem_id) = self.get_problem_id_from_name(problem_name).await {
            self.remove_problem(problem_id).await?;
        }
        self.add_problem(problem_name, problem_description, time_limit).await
    }

    pub async fn add_problem_to_contest(&self, contest_id: ContestId, problem_id: ProblemId) -> Result<()> {
        static QUERY: DatabaseQuery = DatabaseQuery::new("INSERT INTO contest_problems (contest_id, problem_id) VALUES ($1, $2)");

        QUERY.execute(self, &[&contest_id, &problem_id]).await?;
        Ok(())
    }

    pub async fn remove_all_problems_from_contest(&self, contest_id: ContestId) -> Result<()> {
        static QUERY: DatabaseQuery = DatabaseQuery::new("DELETE FROM contest_problems WHERE contest_id = $1");

        QUERY.execute(self, &[&contest_id]).await?;
        Ok(())
    }

    pub async fn get_problem_description(&self, problem_id: ProblemId) -> Result<String> {
        static QUERY: DatabaseQuery = DatabaseQuery::new("SELECT problem_description FROM problems WHERE problem_id = $1");

        Ok(QUERY.execute(self, &[&problem_id]).await?.first().ok_or_else(|| anyhow!("No problem with id {}", problem_id))?.get(0))
    }

    pub async fn get_problem_total_points(&self, problem_id: ProblemId) -> Result<i32> {
        static QUERY: DatabaseQuery = DatabaseQuery::new("SELECT points FROM problems WHERE problem_id = $1");

        Ok(QUERY.execute(self, &[&problem_id]).await?.first().ok_or_else(|| anyhow!("No problem with id {}", problem_id))?.get(0))
    }

    pub async fn get_problem_time_limit(&self, problem_id: ProblemId) -> Result<i32> {
        static QUERY: DatabaseQuery = DatabaseQuery::new("SELECT time_limit FROM problems WHERE problem_id = $1");

        Ok(QUERY.execute(self, &[&problem_id]).await?.first().ok_or_else(|| anyhow!("No problem with id {}", problem_id))?.get(0))
    }

    pub async fn get_user_score_for_problem(&self, user_id: i32, problem_id: ProblemId) -> Result<i32> {
        static QUERY: DatabaseQuery = DatabaseQuery::new("SELECT score FROM user_problem_scores WHERE user_id = $1 AND problem_id = $2");

        Ok(QUERY.execute(self, &[&user_id, &problem_id]).await?.first().map_or(0, |row| row.get(0)))
    }

    pub async fn update_user_score_for_problem(&self, user_id: i32, problem_id: ProblemId) -> Result<()> {
        static QUERY: DatabaseQuery = DatabaseQuery::new("INSERT INTO user_problem_scores (user_id, problem_id, score) VALUES ($1, $2, $3) ON CONFLICT (user_id, problem_id) DO UPDATE SET score = $3");

        let submissions = self.get_submissions_by_user_for_problem(user_id, problem_id).await?;
        let subtasks = self.get_subtasks_for_problem(problem_id).await?;

        let mut score = 0;

        for subtask in subtasks {
            let mut subtask_score = 0;

            for submission in &submissions {
                subtask_score = subtask_score.max(self.get_subtask_points_result(*submission, subtask).await?.unwrap_or(0));
            }

            score += subtask_score;
        }

        QUERY.execute(self, &[&user_id, &problem_id, &score]).await?;

        Ok(())
    }

    pub async fn set_problem_description(&self, problem_id: ProblemId, problem_description: &str) -> Result<()> {
        static QUERY: DatabaseQuery = DatabaseQuery::new("UPDATE problems SET problem_description = $2 WHERE problem_id = $1");

        QUERY.execute(self, &[&problem_id, &problem_description]).await?;
        Ok(())
    }

    pub async fn set_problem_name(&self, problem_id: ProblemId, problem_name: &str) -> Result<()> {
        static QUERY: DatabaseQuery = DatabaseQuery::new("UPDATE problems SET problem_name = $2 WHERE problem_id = $1");

        QUERY.execute(self, &[&problem_id, &problem_name]).await?;
        Ok(())
    }

    pub async fn problem_with_name_exists(&self, problem_name: &str) -> bool {
        static QUERY: DatabaseQuery = DatabaseQuery::new("SELECT problem_id FROM problems WHERE problem_name = $1");

        QUERY.execute(self, &[&problem_name]).await.ok().is_some_and(|rows| !rows.is_empty())
    }

    pub async fn remove_user_scores(&self, user_id: UserId) -> Result<()> {
        static QUERY: DatabaseQuery = DatabaseQuery::new("DELETE FROM user_problem_scores WHERE user_id = $1");

        QUERY.execute(self, &[&user_id]).await?;
        Ok(())
    }
}

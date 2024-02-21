use crate::database::contest::ContestId;
use crate::database::Database;
use anyhow::Result;

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

        Ok(())
    }

    pub async fn is_problem_id_valid(&self, problem_id: ProblemId) -> bool {
        self.get_postgres_client()
            .query("SELECT problem_id FROM problems WHERE problem_id = $1", &[&problem_id])
            .await
            .ok()
            .map(|rows| !rows.is_empty())
            .unwrap_or(false)
    }

    pub async fn get_problem_name(&self, problem_id: ProblemId) -> String {
        self.get_postgres_client()
            .query("SELECT problem_name FROM problems WHERE problem_id = $1", &[&problem_id])
            .await
            .ok()
            .map(|rows| rows[0].get(0))
            .unwrap_or("".to_string())
    }

    pub async fn get_problems_for_contest(&self, contest_id: ContestId) -> Vec<ProblemId> {
        self.get_postgres_client()
            .query("SELECT problem_id FROM contest_problems WHERE contest_id = $1", &[&contest_id])
            .await
            .ok()
            .map(|rows| rows.iter().map(|row| row.get(0)).collect())
            .unwrap_or(Vec::new())
    }

    pub async fn add_problem(&self, problem_name: &str, problem_description: &str, time_limit: i32) -> ProblemId {
        self.get_postgres_client()
            .query(
                "INSERT INTO problems (problem_name, problem_description, points, time_limit) VALUES ($1, $2, $3, $4) RETURNING problem_id",
                &[&problem_name, &problem_description, &0, &time_limit],
            )
            .await
            .unwrap()
            .get(0)
            .unwrap()
            .get(0)
    }

    pub async fn remove_problem(&self, problem_id: ProblemId) {
        self.delete_all_subtasks_and_tests_for_problem(problem_id).await;

        self.get_postgres_client().execute("DELETE FROM problems WHERE problem_id = $1", &[&problem_id]).await.unwrap();
    }

    pub async fn get_problem_id_from_name(&self, problem_name: &str) -> Option<ProblemId> {
        self.get_postgres_client()
            .query("SELECT problem_id FROM problems WHERE problem_name = $1", &[&problem_name])
            .await
            .ok()
            .map(|rows| rows.get(0).map(|row| row.get(0)))
            .flatten()
    }

    pub async fn add_problem_override(&self, problem_name: &str, problem_description: &str, time_limit: i32) -> ProblemId {
        if let Some(problem_id) = self.get_problem_id_from_name(problem_name).await {
            self.remove_problem(problem_id).await;
        }
        self.add_problem(problem_name, problem_description, time_limit).await
    }

    pub async fn add_problem_to_contest(&self, contest_id: ContestId, problem_id: ProblemId) {
        self.get_postgres_client()
            .execute("INSERT INTO contest_problems (contest_id, problem_id) VALUES ($1, $2)", &[&contest_id, &problem_id])
            .await
            .unwrap();
    }

    pub async fn remove_all_problems_from_contest(&self, contest_id: ContestId) {
        self.get_postgres_client().execute("DELETE FROM contest_problems WHERE contest_id = $1", &[&contest_id]).await.unwrap();
    }

    pub async fn get_problem_description(&self, problem_id: ProblemId) -> String {
        self.get_postgres_client()
            .query("SELECT problem_description FROM problems WHERE problem_id = $1", &[&problem_id])
            .await
            .ok()
            .map(|rows| rows[0].get(0))
            .unwrap_or("".to_string())
    }

    pub async fn get_problem_total_points(&self, problem_id: ProblemId) -> i32 {
        self.get_postgres_client()
            .query("SELECT points FROM problems WHERE problem_id = $1", &[&problem_id])
            .await
            .ok()
            .map(|rows| rows[0].get(0))
            .unwrap_or(0)
    }

    pub async fn get_problem_time_limit(&self, problem_id: ProblemId) -> i32 {
        self.get_postgres_client()
            .query("SELECT time_limit FROM problems WHERE problem_id = $1", &[&problem_id])
            .await
            .ok()
            .map(|rows| rows[0].get(0))
            .unwrap_or(0)
    }
}

use crate::create_html_response;
use crate::database::Database;
use crate::problem::ProblemId;
use crate::user::UserId;
use anyhow::Result;
use askama::Template;
use http_body_util::Full;
use hyper::body::Bytes;
use hyper::Response;

pub type ContestId = i32;

#[derive(Template)]
#[template(path = "contest.html")]
pub struct ContestSite {
    contest_id: ContestId,
    problems: Vec<(ProblemId, String)>,
}

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
            .query(
                "SELECT contest_id FROM contests WHERE contest_id = $1",
                &[&contest_id],
            )
            .await
            .ok()
            .map(|rows| !rows.is_empty())
            .unwrap_or(false)
    }

    pub async fn get_contest_name(&self, contest_id: ContestId) -> String {
        self.get_postgres_client()
            .query(
                "SELECT contest_name FROM contests WHERE contest_id = $1",
                &[&contest_id],
            )
            .await
            .ok()
            .map(|rows| rows[0].get(0))
            .unwrap_or("".to_string())
    }

    pub async fn get_contests_for_user(&self, user_id: UserId) -> Vec<ContestId> {
        self.get_postgres_client()
            .query(
                "SELECT contest_id FROM contest_participations WHERE user_id = $1",
                &[&user_id],
            )
            .await
            .ok()
            .map(|rows| rows.iter().map(|row| row.get(0)).collect())
            .unwrap_or(Vec::new())
    }

    pub async fn add_contest(&self, contest_name: &str) -> ContestId {
        self.get_postgres_client()
            .query(
                "INSERT INTO contests (contest_name) VALUES ($1) RETURNING contest_id",
                &[&contest_name],
            )
            .await
            .unwrap()
            .get(0)
            .unwrap()
            .get(0)
    }

    pub async fn add_user_to_contest(&self, user_id: UserId, contest_id: ContestId) {
        self.get_postgres_client()
            .execute(
                "INSERT INTO contest_participations (contest_id, user_id) VALUES ($1, $2)",
                &[&contest_id, &user_id],
            )
            .await
            .unwrap();
    }
}

pub async fn create_contest_page(
    database: &Database,
    contest_id: &str,
) -> Result<Option<Response<Full<Bytes>>>> {
    if let Some(contest_id) = contest_id.parse::<i32>().ok() {
        if database.is_contest_id_valid(contest_id).await {
            let mut problems = Vec::new();
            for problem_id in database.get_problems_for_contest(contest_id).await {
                problems.push((
                    problem_id,
                    database.get_problem_name(problem_id).await.clone(),
                ));
            }

            return Ok(Some(create_html_response(ContestSite {
                contest_id,
                problems,
            })?));
        }
    }
    Ok(None)
}

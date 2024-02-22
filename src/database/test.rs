use crate::database::problem::ProblemId;
use crate::database::submission::{i32_to_testing_result, testing_result_to_i32, SubmissionId, TestingResult};
use crate::database::Database;
use anyhow::Result;

pub type TestId = i32;
pub type SubtaskId = i32;

impl Database {
    pub async fn init_tests(&self) -> Result<()> {
        // create the subtasks table
        self.get_postgres_client()
            .execute(
                "CREATE TABLE IF NOT EXISTS subtasks (
                        subtask_id SERIAL PRIMARY KEY,
                        problem_id INT REFERENCES problems(problem_id),
                        subtask_score INT NOT NULL
                    );",
                &[],
            )
            .await?;

        // create the tests table
        self.get_postgres_client()
            .execute(
                "CREATE TABLE IF NOT EXISTS tests (
                        test_id SERIAL PRIMARY KEY,
                        problem_id INT REFERENCES problems(problem_id),
                        input_data TEXT NOT NULL,
                        output_data TEXT NOT NULL
                    );",
                &[],
            )
            .await?;

        // create a relation that connects tests to subtasks
        self.get_postgres_client()
            .execute(
                "CREATE TABLE IF NOT EXISTS subtask_tests (
                        subtask_id INT REFERENCES subtasks(subtask_id),
                        test_id INT REFERENCES tests(test_id),
                        PRIMARY KEY (subtask_id, test_id)
                    );",
                &[],
            )
            .await?;

        // create a table of subtask results
        self.get_postgres_client()
            .execute(
                "CREATE TABLE IF NOT EXISTS subtask_results (
                        submission_id INT REFERENCES submissions(submission_id),
                        subtask_id INT REFERENCES subtasks(subtask_id),
                        points INT,
                        result INT NOT NULL
                    );",
                &[],
            )
            .await?;

        // create a table of test results
        self.get_postgres_client()
            .execute(
                "CREATE TABLE IF NOT EXISTS test_results (
                        submission_id INT REFERENCES submissions(submission_id),
                        test_id INT REFERENCES tests(test_id),
                        result INT NOT NULL,
                        time INT
                    );",
                &[],
            )
            .await?;

        Ok(())
    }

    pub async fn add_test(&self, input_data: &str, output_data: &str, problem_id: ProblemId) -> Result<TestId> {
        Ok(self
            .get_postgres_client()
            .query(
                "INSERT INTO tests (input_data, output_data, problem_id) VALUES ($1, $2, $3) RETURNING test_id",
                &[&input_data, &output_data, &problem_id],
            )
            .await?
            .first()
            .ok_or_else(|| anyhow::anyhow!("Could not retrieve the first row"))?
            .get(0))
    }

    pub async fn get_tests_for_subtask(&self, subtask_id: SubtaskId) -> Result<Vec<TestId>> {
        Ok(self
            .get_postgres_client()
            .query("SELECT test_id FROM subtask_tests WHERE subtask_id = $1", &[&subtask_id])
            .await?
            .iter()
            .map(|row| row.get(0))
            .collect())
    }

    pub async fn get_test_data(&self, test_id: TestId) -> Result<(String, String)> {
        let column = self.get_postgres_client().query("SELECT input_data, output_data FROM tests WHERE test_id = $1", &[&test_id]).await?;

        let row = column.first().ok_or_else(|| anyhow::anyhow!("No test with id {}", test_id))?;

        Ok((row.get(0), row.get(1)))
    }

    pub async fn add_subtask(&self, problem_id: ProblemId, subtask_score: i32) -> Result<SubtaskId> {
        // increment points of the problem
        self.get_postgres_client()
            .execute("UPDATE problems SET points = points + $2 WHERE problem_id = $1", &[&problem_id, &subtask_score])
            .await?;

        Ok(self
            .get_postgres_client()
            .query("INSERT INTO subtasks (problem_id, subtask_score) VALUES ($1, $2) RETURNING subtask_id", &[&problem_id, &subtask_score])
            .await?
            .first()
            .ok_or_else(|| anyhow::anyhow!("Could not retrieve the first row"))?
            .get(0))
    }

    pub async fn get_subtasks_for_problem(&self, problem_id: ProblemId) -> Result<Vec<SubtaskId>> {
        Ok(self
            .get_postgres_client()
            .query("SELECT subtask_id FROM subtasks WHERE problem_id = $1", &[&problem_id])
            .await?
            .iter()
            .map(|row| row.get(0))
            .collect())
    }

    pub async fn add_test_to_subtask(&self, subtask_id: SubtaskId, test_id: TestId) -> Result<()> {
        self.get_postgres_client()
            .execute("INSERT INTO subtask_tests (subtask_id, test_id) VALUES ($1, $2)", &[&subtask_id, &test_id])
            .await?;
        Ok(())
    }

    pub async fn delete_all_subtasks_and_tests_for_problem(&self, problem_id: ProblemId) -> Result<()> {
        self.get_postgres_client()
            .execute("DELETE FROM subtask_tests WHERE subtask_id IN (SELECT subtask_id FROM subtasks WHERE problem_id = $1)", &[&problem_id])
            .await?;

        self.get_postgres_client().execute("DELETE FROM subtasks WHERE problem_id = $1", &[&problem_id]).await?;

        self.get_postgres_client().execute("DELETE FROM tests WHERE problem_id = $1", &[&problem_id]).await?;

        Ok(())
    }

    pub async fn get_all_tests_for_problem(&self, problem_id: ProblemId) -> Result<Vec<TestId>> {
        Ok(self
            .get_postgres_client()
            .query("SELECT test_id FROM tests WHERE problem_id = $1", &[&problem_id])
            .await?
            .iter()
            .map(|row| row.get(0))
            .collect())
    }

    pub async fn get_subtasks_for_submission(&self, submission_id: SubmissionId) -> Result<Vec<SubtaskId>> {
        Ok(self
            .get_postgres_client()
            .query("SELECT subtask_id FROM subtask_results WHERE submission_id = $1", &[&submission_id])
            .await?
            .iter()
            .map(|row| row.get(0))
            .collect())
    }

    pub async fn get_test_result(&self, submission_id: SubmissionId, test_id: TestId) -> Result<TestingResult> {
        let result = self
            .get_postgres_client()
            .query("SELECT result FROM test_results WHERE submission_id = $1 AND test_id = $2", &[&submission_id, &test_id])
            .await?
            .first()
            .ok_or_else(|| anyhow::anyhow!("No test result for submission {} and test {}", submission_id, test_id))?
            .get(0);

        Ok(i32_to_testing_result(result))
    }

    pub async fn get_subtask_result(&self, submission_id: SubmissionId, subtask_id: SubtaskId) -> Result<TestingResult> {
        let result = self
            .get_postgres_client()
            .query("SELECT result FROM subtask_results WHERE submission_id = $1 AND subtask_id = $2", &[&submission_id, &subtask_id])
            .await?
            .first()
            .ok_or_else(|| anyhow::anyhow!("No subtask result for submission {} and subtask {}", submission_id, subtask_id))?
            .get(0);

        Ok(i32_to_testing_result(result))
    }

    pub async fn delete_all_results_for_submission(&self, submission_id: SubmissionId) -> Result<()> {
        self.get_postgres_client().execute("DELETE FROM test_results WHERE submission_id = $1", &[&submission_id]).await?;

        self.get_postgres_client().execute("DELETE FROM subtask_results WHERE submission_id = $1", &[&submission_id]).await?;

        Ok(())
    }

    pub async fn get_tests_for_submission(&self, submission_id: SubmissionId) -> Result<Vec<TestId>> {
        Ok(self
            .get_postgres_client()
            .query("SELECT test_id FROM test_results WHERE submission_id = $1", &[&submission_id])
            .await?
            .iter()
            .map(|row| row.get(0))
            .collect())
    }

    pub async fn set_test_result(&self, submission_id: SubmissionId, test_id: TestId, result: TestingResult) -> Result<()> {
        self.get_postgres_client()
            .execute(
                "UPDATE test_results SET result = $3 WHERE submission_id = $1 AND test_id = $2",
                &[&submission_id, &test_id, &testing_result_to_i32(result)],
            )
            .await?;

        Ok(())
    }

    pub async fn get_subtask_points_result(&self, submission_id: SubmissionId, subtask_id: SubtaskId) -> Result<Option<i32>> {
        let column = self
            .get_postgres_client()
            .query("SELECT points FROM subtask_results WHERE submission_id = $1 AND subtask_id = $2", &[&submission_id, &subtask_id])
            .await?;
        let row = column
            .first()
            .ok_or_else(|| anyhow::anyhow!("No subtask result for submission {} and subtask {}", submission_id, subtask_id))?;

        Ok(row.try_get(0).ok())
    }

    pub async fn get_subtask_total_points(&self, subtask_id: SubtaskId) -> Result<i32> {
        Ok(self
            .get_postgres_client()
            .query("SELECT subtask_score FROM subtasks WHERE subtask_id = $1", &[&subtask_id])
            .await?
            .first()
            .ok_or_else(|| anyhow::anyhow!("No subtask with id {}", subtask_id))?
            .get(0))
    }

    pub async fn get_test_time(&self, submission_id: SubmissionId, test_id: TestId) -> Result<Option<i32>> {
        let column = self
            .get_postgres_client()
            .query("SELECT time FROM test_results WHERE submission_id = $1 AND test_id = $2", &[&submission_id, &test_id])
            .await?;
        let row = column.first().ok_or_else(|| anyhow::anyhow!("No test result for submission {} and test {}", submission_id, test_id))?;

        Ok(row.try_get(0).ok())
    }

    pub async fn set_test_time(&self, submission_id: SubmissionId, test_id: TestId, time: i32) -> Result<()> {
        self.get_postgres_client()
            .execute("UPDATE test_results SET time = $3 WHERE submission_id = $1 AND test_id = $2", &[&submission_id, &test_id, &time])
            .await?;
        Ok(())
    }

    pub async fn set_subtask_result(&self, submission_id: SubmissionId, subtask_id: SubtaskId, result: TestingResult) -> Result<()> {
        self.get_postgres_client()
            .execute(
                "UPDATE subtask_results SET result = $3 WHERE submission_id = $1 AND subtask_id = $2",
                &[&submission_id, &subtask_id, &testing_result_to_i32(result)],
            )
            .await?;
        Ok(())
    }
}

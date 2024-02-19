use crate::database::Database;

pub type TestId = i32;
pub type SubtaskId = i32;

impl Database {
    pub async fn init_tests(&self) {
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
            .await
            .unwrap();

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
            .await
            .unwrap();

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
            .await
            .unwrap();

        // create a table of subtask results
        self.get_postgres_client()
            .execute(
                "CREATE TABLE IF NOT EXISTS subtask_results (
                        submission_id INT REFERENCES submissions(submission_id),
                        subtask_id INT REFERENCES subtasks(subtask_id),
                        points INT NOT NULL,
                        result TESTING_RESULT
                    );",
                &[],
            )
            .await
            .unwrap();

        // create a table of test results
        self.get_postgres_client()
            .execute(
                "CREATE TABLE IF NOT EXISTS test_results (
                        submission_id INT REFERENCES submissions(submission_id),
                        test_id INT REFERENCES tests(test_id),
                        result TESTING_RESULT
                    );",
                &[],
            )
            .await
            .unwrap();
    }

    pub async fn add_test(&self, input_data: &str, output_data: &str) -> TestId {
        self.get_postgres_client()
            .query(
                "INSERT INTO tests (input_data, output_data) VALUES ($1, $2) RETURNING test_id",
                &[&input_data, &output_data],
            )
            .await
            .unwrap()
            .get(0)
            .unwrap()
            .get(0)
    }

    pub async fn get_tests_for_subtask(&self, subtask_id: SubtaskId) -> Vec<TestId> {
        self.get_postgres_client()
            .query(
                "SELECT test_id FROM subtask_tests WHERE subtask_id = $1",
                &[&subtask_id],
            )
            .await
            .unwrap()
            .iter()
            .map(|row| row.get(0))
            .collect()
    }

    pub async fn get_test_data(&self, test_id: TestId) -> (String, String) {
        self.get_postgres_client()
            .query(
                "SELECT input_data, output_data FROM tests WHERE test_id = $1",
                &[&test_id],
            )
            .await
            .unwrap()
            .iter()
            .map(|row| (row.get(0), row.get(1)))
            .next()
            .unwrap()
    }

    pub async fn get_subtask_score(&self, subtask_id: SubtaskId) -> i32 {
        self.get_postgres_client()
            .query(
                "SELECT subtask_score FROM subtasks WHERE subtask_id = $1",
                &[&subtask_id],
            )
            .await
            .unwrap()
            .iter()
            .map(|row| row.get(0))
            .next()
            .unwrap()
    }

    pub async fn add_subtask(&self, problem_id: i32, subtask_score: i32) -> SubtaskId {
        self.get_postgres_client()
            .query(
                "INSERT INTO subtasks (problem_id, subtask_score) VALUES ($1, $2) RETURNING subtask_id",
                &[&problem_id, &subtask_score],
            ).await
            .unwrap()
            .get(0).unwrap()
            .get(0)
    }

    pub async fn get_subtasks_for_problem(&self, problem_id: i32) -> Vec<SubtaskId> {
        self.get_postgres_client()
            .query(
                "SELECT subtask_id FROM subtasks WHERE problem_id = $1",
                &[&problem_id],
            )
            .await
            .unwrap()
            .iter()
            .map(|row| row.get(0))
            .collect()
    }

    pub async fn add_test_to_subtask(&self, subtask_id: SubtaskId, test_id: TestId) {
        self.get_postgres_client()
            .execute(
                "INSERT INTO subtask_tests (subtask_id, test_id) VALUES ($1, $2)",
                &[&subtask_id, &test_id],
            )
            .await
            .unwrap();
    }

    pub async fn delete_all_subtasks_and_tests_for_problem(&self, problem_id: i32) {
        self.get_postgres_client()
            .execute("DELETE FROM subtask_tests WHERE subtask_id IN (SELECT subtask_id FROM subtasks WHERE problem_id = $1)", &[&problem_id])
            .await
            .unwrap();

        self.get_postgres_client()
            .execute("DELETE FROM subtasks WHERE problem_id = $1", &[&problem_id])
            .await
            .unwrap();

        self.get_postgres_client()
            .execute("DELETE FROM tests WHERE problem_id = $1", &[&problem_id])
            .await
            .unwrap();
    }
}

use crate::database::problem::ProblemId;
use crate::database::test::SubtaskId;
use crate::database::user::UserId;
use crate::database::Database;
use crate::worker::WorkerManager;

pub type SubmissionId = i32;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum TestingResult {
    InQueue,
    Compiling,
    Testing,
    Accepted,
    WrongAnswer,
    RuntimeError,
    TimeLimitExceeded,
    MemoryLimitExceeded,
    InternalError,
}

// make sure that testing results are stored in the database as integers
pub const fn testing_result_to_i32(result: TestingResult) -> i32 {
    match result {
        TestingResult::InQueue => 1,
        TestingResult::Compiling => 2,
        TestingResult::Testing => 3,
        TestingResult::Accepted => 4,
        TestingResult::WrongAnswer => 5,
        TestingResult::RuntimeError => 6,
        TestingResult::TimeLimitExceeded => 7,
        TestingResult::MemoryLimitExceeded => 8,
        TestingResult::InternalError => 9,
    }
}

pub fn i32_to_testing_result(result: i32) -> TestingResult {
    match result {
        1 => TestingResult::InQueue,
        2 => TestingResult::Compiling,
        3 => TestingResult::Testing,
        4 => TestingResult::Accepted,
        5 => TestingResult::WrongAnswer,
        6 => TestingResult::RuntimeError,
        7 => TestingResult::TimeLimitExceeded,
        8 => TestingResult::MemoryLimitExceeded,
        9 => TestingResult::InternalError,
        _ => panic!("Invalid testing result"),
    }
}

// make sure to display testing results as strings in the HTML
pub fn testing_result_to_string(result: TestingResult) -> String {
    match result {
        TestingResult::InQueue => "In Queue".to_string(),
        TestingResult::Compiling => "Compiling".to_string(),
        TestingResult::Testing => "Testing".to_string(),
        TestingResult::Accepted => "Accepted".to_string(),
        TestingResult::WrongAnswer => "Wrong Answer".to_string(),
        TestingResult::RuntimeError => "Runtime Error".to_string(),
        TestingResult::TimeLimitExceeded => "Time Limit Exceeded".to_string(),
        TestingResult::MemoryLimitExceeded => "Memory Limit Exceeded".to_string(),
        TestingResult::InternalError => "Internal Error".to_string(),
    }
}

impl Database {
    pub async fn init_submissions(&self) {
        self.get_postgres_client()
            .execute(
                "CREATE TABLE IF NOT EXISTS submissions (
                    submission_id SERIAL PRIMARY KEY,
                    user_id INT REFERENCES users(user_id),
                    problem_id INT REFERENCES problems(problem_id),
                    code TEXT NOT NULL,
                    result INT NOT NULL,
                    points INT,
                    tests_done INT NOT NULL
                );",
                &[],
            )
            .await
            .unwrap();
    }

    pub async fn add_submission(&self, user_id: UserId, problem_id: ProblemId, code: String, workers: &WorkerManager) -> SubmissionId {
        let submission_id = self
            .get_postgres_client()
            .query(
                "INSERT INTO submissions (user_id, problem_id, code, result, tests_done) VALUES ($1, $2, $3, $4, $5) RETURNING submission_id",
                &[&user_id, &problem_id, &code, &testing_result_to_i32(TestingResult::InQueue), &0],
            )
            .await
            .unwrap()
            .get(0)
            .unwrap()
            .get(0);

        // add all subtasks for the problem
        let subtasks = self.get_subtasks_for_problem(problem_id).await;
        for subtask in subtasks {
            self.get_postgres_client()
                .execute(
                    "INSERT INTO subtask_results (submission_id, subtask_id, result) VALUES ($1, $2, $3)",
                    &[&submission_id, &subtask, &testing_result_to_i32(TestingResult::InQueue)],
                )
                .await
                .unwrap();
        }

        // add all tests for the problem
        let tests = self.get_all_tests_for_problem(problem_id).await;
        for test in tests {
            self.get_postgres_client()
                .execute(
                    "INSERT INTO test_results (submission_id, test_id, result) VALUES ($1, $2, $3)",
                    &[&submission_id, &test, &testing_result_to_i32(TestingResult::InQueue)],
                )
                .await
                .unwrap();
        }

        let database = self.clone();
        let workers = workers.clone();
        tokio::spawn(async move {
            workers.test_submission(submission_id, &database).await;
        });

        submission_id
    }

    async fn update_subtask_result(&self, submission_id: SubmissionId, subtask_id: SubtaskId) {
        let tests = self.get_tests_for_subtask_in_submission(submission_id, subtask_id).await;
        let mut result = TestingResult::Accepted;
        for test in tests {
            let test_result = self.get_test_result(submission_id, test).await;
            if test_result != TestingResult::Accepted {
                result = test_result;
            }
        }

        self.get_postgres_client()
            .execute(
                "UPDATE subtask_results SET result = $1 WHERE submission_id = $2 AND subtask_id = $3",
                &[&testing_result_to_i32(result), &submission_id, &subtask_id],
            )
            .await
            .unwrap();

        let points = if result == TestingResult::Accepted { self.get_subtask_total_points(subtask_id).await } else { 0 };

        self.get_postgres_client()
            .execute(
                "UPDATE subtask_results SET points = $1 WHERE submission_id = $2 AND subtask_id = $3",
                &[&points, &submission_id, &subtask_id],
            )
            .await
            .unwrap();
    }

    pub async fn update_submission_result(&self, submission_id: SubmissionId) {
        let subtasks = self.get_subtasks_for_submission(submission_id).await;
        let mut result = TestingResult::Accepted;
        let mut points = 0;
        for subtask in subtasks {
            self.update_subtask_result(submission_id, subtask).await;
            let subtask_result = self.get_subtask_result(submission_id, subtask).await;
            if subtask_result != TestingResult::Accepted {
                result = subtask_result;
            }
            points += self.get_subtask_points_result(submission_id, subtask).await.unwrap_or(0);
        }

        self.get_postgres_client()
            .execute("UPDATE submissions SET result = $1 WHERE submission_id = $2", &[&testing_result_to_i32(result), &submission_id])
            .await
            .unwrap();

        self.get_postgres_client()
            .execute("UPDATE submissions SET points = $1 WHERE submission_id = $2", &[&points, &submission_id])
            .await
            .unwrap();
    }

    pub async fn get_submissions_by_user_for_problem(&self, user_id: UserId, problem_id: ProblemId) -> Vec<SubmissionId> {
        self.get_postgres_client()
            .query("SELECT submission_id FROM submissions WHERE user_id = $1 AND problem_id = $2", &[&user_id, &problem_id])
            .await
            .unwrap()
            .iter()
            .map(|row| row.get(0))
            .collect()
    }

    pub async fn get_all_submissions_for_user(&self, user_id: UserId) -> Vec<SubmissionId> {
        self.get_postgres_client()
            .query("SELECT submission_id FROM submissions WHERE user_id = $1", &[&user_id])
            .await
            .unwrap()
            .iter()
            .map(|row| row.get(0))
            .collect()
    }

    pub async fn delete_all_submissions_for_user(&self, user_id: UserId) {
        for submission in self.get_all_submissions_for_user(user_id).await {
            self.delete_all_results_for_submission(submission).await;
        }

        self.get_postgres_client().execute("DELETE FROM submissions WHERE user_id = $1", &[&user_id]).await.unwrap();
    }

    pub async fn get_submission_code(&self, submission_id: SubmissionId) -> String {
        self.get_postgres_client()
            .query("SELECT code FROM submissions WHERE submission_id = $1", &[&submission_id])
            .await
            .unwrap()
            .get(0)
            .unwrap()
            .get(0)
    }

    pub async fn get_submission_result(&self, submission_id: SubmissionId) -> TestingResult {
        i32_to_testing_result(
            self.get_postgres_client()
                .query("SELECT result FROM submissions WHERE submission_id = $1", &[&submission_id])
                .await
                .unwrap()
                .get(0)
                .unwrap()
                .get(0),
        )
    }

    pub async fn get_submission_tests_done(&self, submission_id: SubmissionId) -> i32 {
        self.get_postgres_client()
            .query("SELECT tests_done FROM submissions WHERE submission_id = $1", &[&submission_id])
            .await
            .unwrap()
            .get(0)
            .unwrap()
            .get(0)
    }

    pub async fn increment_submission_tests_done(&self, submission_id: SubmissionId) {
        self.get_postgres_client()
            .execute("UPDATE submissions SET tests_done = tests_done + 1 WHERE submission_id = $1", &[&submission_id])
            .await
            .unwrap();
    }

    pub async fn get_submission_points(&self, submission_id: SubmissionId) -> Option<i32> {
        let column = self
            .get_postgres_client()
            .query("SELECT points FROM submissions WHERE submission_id = $1", &[&submission_id])
            .await
            .unwrap();

        let row = column.get(0).unwrap();

        row.try_get(0).ok()
    }

    pub async fn get_submission_problem(&self, submission_id: SubmissionId) -> ProblemId {
        self.get_postgres_client()
            .query("SELECT problem_id FROM submissions WHERE submission_id = $1", &[&submission_id])
            .await
            .unwrap()
            .get(0)
            .unwrap()
            .get(0)
    }

    pub async fn set_submission_result(&self, submission_id: SubmissionId, result: TestingResult) {
        self.get_postgres_client()
            .execute("UPDATE submissions SET result = $1 WHERE submission_id = $2", &[&testing_result_to_i32(result), &submission_id])
            .await
            .unwrap();
    }
}

use crate::database::contest::ContestId;
use crate::database::problem::ProblemId;
use crate::database::submission::{testing_result_to_short_string, SubmissionId, TestingResult};
use crate::database::test::{SubtaskId, TestId};
use crate::database::user::UserId;
use crate::database::Database;
use crate::request_handler::{create_html_response, RedirectSite};
use crate::sidebar::{create_sidebar_context, SidebarContext};
use crate::submission::extract_file_from_request;
use anyhow::{anyhow, bail, Result};
use askama::Template;
use http_body_util::BodyExt;
use http_body_util::Full;
use hyper::body::{Bytes, Incoming};
use hyper::{Request, Response};
use std::collections::HashMap;

#[derive(Template)]
#[template(path = "problem.html")]
pub struct ProblemSite {
    contest_id: ContestId,
    problem_id: ProblemId,
    problem_name: String,
    problem_description: String,
    submissions: Vec<(SubmissionId, i32, i32, bool, String)>,
    sidebar_context: SidebarContext,
    points: i32,
    max_points: i32,
    is_admin: bool,
}

#[derive(Template)]
#[template(path = "edit_problem.html")]
pub struct EditProblemSite {
    contest_id: ContestId,
    problem_id: ProblemId,
    problem_name: String,
    problem_description: String,
    sidebar_context: SidebarContext,
    subtasks: Vec<(SubtaskId, Vec<TestId>)>,
}

pub async fn create_problem_page(database: &Database, contest_id: &str, problem_id: &str, user_id: UserId) -> Result<Option<Response<Full<Bytes>>>> {
    if let (Some(contest_id), Some(problem_id)) = (contest_id.parse::<ContestId>().ok(), problem_id.parse::<ProblemId>().ok()) {
        if !database.is_contest_id_valid(contest_id).await {
            return Ok(None);
        }

        if !database.is_problem_id_valid(problem_id).await {
            return Ok(None);
        }

        let mut submissions = {
            let mut res = Vec::new();
            for id in database.get_submissions_by_user_for_problem(user_id, problem_id).await? {
                let total_points = database.get_problem_total_points(problem_id).await?;
                let points = database.get_submission_points(id).await?.unwrap_or(0);
                let result = database.get_submission_result(id).await?;
                let mut message = testing_result_to_short_string(result);

                if result == TestingResult::Testing {
                    let percent = (database.get_submission_tests_done(id).await? as f64 / database.get_tests_for_submission(id).await?.len() as f64 * 100.0).round();
                    message = format!("{message} ({percent}%)");
                }

                let hide_score = result == TestingResult::InQueue || result == TestingResult::Testing || result == TestingResult::CompilationError || result == TestingResult::Compiling;

                res.push((id, points, total_points.max(1), hide_score, message));
            }
            res
        };

        // Sort by submission id in descending order
        submissions.sort_by(|a, b| b.0.cmp(&a.0));

        let problem_description = database.get_problem_description(problem_id).await?;

        let points = database.get_user_score_for_problem(user_id, problem_id).await?;
        let max_points = database.get_problem_total_points(problem_id).await?.max(1);
        let is_admin = database.is_user_admin(user_id).await?;

        return Ok(Some(create_html_response(&ProblemSite {
            contest_id,
            problem_id,
            problem_description,
            problem_name: database.get_problem_name(problem_id).await?,
            submissions,
            sidebar_context: create_sidebar_context(database, Some(user_id)).await?,
            points,
            max_points,
            is_admin,
        })?));
    }

    Ok(None)
}

pub async fn create_edit_problem_page(database: &Database, contest_id: &str, problem_id: &str, user_id: UserId) -> Result<Option<Response<Full<Bytes>>>> {
    if let (Some(contest_id), Some(problem_id)) = (contest_id.parse::<ContestId>().ok(), problem_id.parse::<ProblemId>().ok()) {
        if !database.is_contest_id_valid(contest_id).await {
            return Ok(None);
        }

        if !database.is_problem_id_valid(problem_id).await {
            return Ok(None);
        }

        let problem_description = database.get_problem_description(problem_id).await?;

        let mut subtasks = Vec::new();
        for subtask_id in database.get_subtasks_for_problem(problem_id).await? {
            let mut tests = Vec::new();
            for test_id in database.get_tests_for_subtask(subtask_id).await? {
                tests.push(test_id);
            }
            subtasks.push((subtask_id, tests));
        }

        return Ok(Some(create_html_response(&EditProblemSite {
            contest_id,
            problem_id,
            problem_description,
            problem_name: database.get_problem_name(problem_id).await?,
            sidebar_context: create_sidebar_context(database, Some(user_id)).await?,
            subtasks,
        })?));
    }

    Ok(None)
}

pub fn parse_body_with_boundaries(body: &str) -> Result<HashMap<String, String>> {
    // boundary is the first line
    let boundary = body.lines().next().ok_or_else(|| anyhow!("No boundary found"))?;
    let elements: Vec<&str> = body.split(boundary).collect();

    let mut result = HashMap::new();

    for element in elements {
        let mut lines = element.lines().collect::<Vec<&str>>();
        while lines.first().map_or(false, |x| x.is_empty()) {
            lines.remove(0);
        }
        if let Some(header) = lines.first().copied() {
            lines.remove(0);
            while lines.last().map_or(false, |x| x.is_empty()) {
                lines.pop();
            }
            while lines.first().map_or(false, |x| x.is_empty()) {
                lines.remove(0);
            }

            let content = "\n".to_owned() + &lines.join("\n");

            let mut name = String::new();
            if let Some(mut idx) = header.find("name=\"") {
                idx += 6;
                while header.chars().nth(idx).ok_or_else(|| anyhow!("No closing quote found"))? != '"' {
                    name.push(header.chars().nth(idx).ok_or_else(|| anyhow!("No character found"))?);
                    idx += 1;
                }

                result.insert(name, content);
            }
        }
    }

    Ok(result)
}

pub async fn handle_problem_editing(database: &Database, contest_id: &str, problem_id: &str, user_id: UserId, request: Request<Incoming>) -> Result<Option<Response<Full<Bytes>>>> {
    if let (Some(contest_id), Some(problem_id)) = (contest_id.parse::<ContestId>().ok(), problem_id.parse::<ProblemId>().ok()) {
        if !database.is_contest_id_valid(contest_id).await {
            return Ok(None);
        }

        if !database.is_problem_id_valid(problem_id).await {
            return Ok(None);
        }

        if !database.is_user_admin(user_id).await? {
            return Ok(None);
        }

        let body = request.into_body().collect().await?.to_bytes();
        let body = String::from_utf8_lossy(&body).to_string();
        let parsed_body = parse_body_with_boundaries(&body)?;

        if let Some(description) = parsed_body.get("description") {
            database.set_problem_description(problem_id, description).await?;
        }

        if let Some(name) = parsed_body.get("name") {
            database.set_problem_name(problem_id, name).await?;
        }

        return Ok(Some(create_html_response(&RedirectSite {
            url: format!("/contest/{contest_id}/edit_problem/{problem_id}"),
        })?));
    }

    Ok(None)
}

pub async fn create_new_problem(database: &Database, contest_id: &str) -> Result<Response<Full<Bytes>>> {
    if let Ok(contest_id) = contest_id.parse::<ContestId>() {
        if !database.is_contest_id_valid(contest_id).await {
            bail!("Invalid contest id");
        }

        let mut problem_number = 1;
        while database.problem_with_name_exists(&format!("Problem {problem_number}")).await {
            problem_number += 1;
            if problem_number > 1000 {
                bail!("Too many problems: an error occured");
            }
        }

        let problem_id = database.add_problem_override(&format!("Problem {problem_number}"), "Insert description here...", 1000).await?;

        database.add_problem_to_contest(contest_id, problem_id).await?;

        return create_html_response(&RedirectSite {
            url: format!("/contest/{contest_id}/edit_problem/{problem_id}"),
        });
    }

    bail!("Invalid contest id");
}

#[derive(serde::Deserialize)]
pub struct CPSTests {
    pub tests: Vec<(String, String)>,
    pub subtask_tests: Vec<Vec<usize>>,
    pub subtask_points: Vec<i32>,
}

pub async fn handle_tests_uploading(database: &Database, contest_id: &str, problem_id: &str, request: Request<Incoming>) -> Result<Response<Full<Bytes>>> {
    let contest_id = contest_id.parse::<ContestId>().map_err(|_e| anyhow!("Invalid contest id"))?;
    let problem_id = problem_id.parse::<ProblemId>().map_err(|_e| anyhow!("Invalid problem id"))?;

    if !database.is_contest_id_valid(contest_id).await {
        bail!("Invalid contest id");
    }

    if !database.is_problem_id_valid(problem_id).await {
        bail!("Invalid problem id");
    }

    let file_contents = extract_file_from_request(request).await?;
    let decompressed = snap::raw::Decoder::new().decompress_vec(file_contents.as_slice())?;

    let tests: CPSTests = bincode::deserialize(decompressed.as_slice())?;

    database.remove_all_submissions_testing_data_for_problem(problem_id).await?;
    database.remove_all_test_data_for_problem(problem_id).await?;

    let mut db_tests = Vec::new();

    for (input, output) in tests.tests {
        let test_id = database.add_test(&input, &output, problem_id).await?;
        db_tests.push(test_id);
    }

    for (tests, points) in tests.subtask_tests.into_iter().zip(tests.subtask_points.into_iter()) {
        let subtask_id = database.add_subtask(problem_id, points).await?;
        for test in tests {
            database.add_test_to_subtask(subtask_id, *db_tests.get(test).ok_or_else(|| anyhow!("Invalid test index"))?).await?;
        }
    }

    create_html_response(&RedirectSite {
        url: format!("/contest/{contest_id}/edit_problem/{problem_id}"),
    })
}

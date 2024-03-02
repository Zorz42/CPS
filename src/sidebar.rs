use crate::database::contest::ContestId;
use crate::database::problem::ProblemId;
use crate::database::user::UserId;
use crate::database::Database;
use anyhow::Result;

#[allow(clippy::type_complexity)]
pub struct SidebarContext {
    pub logged_in: bool,
    pub username: String,
    pub contests: Vec<(ContestId, String, i32, i32, Vec<(ProblemId, String, i32, i32)>)>,
}

pub async fn create_sidebar_context(database: &Database, user: Option<UserId>) -> Result<SidebarContext> {
    let mut contests = Vec::new();
    if let Some(user) = user {
        for id in database.get_contests_for_user(user).await? {
            let mut contest_points = 0;
            let mut contest_max_points = 0;

            let mut problems = Vec::new();
            for problem in database.get_problems_for_contest(id).await? {
                let max_points = database.get_problem_total_points(problem).await?.max(1);
                let points = database.get_user_score_for_problem(user, problem).await?;

                contest_points += points;
                contest_max_points += max_points;

                problems.push((problem, database.get_problem_name(problem).await?, points, max_points));
            }

            contests.push((id, database.get_contest_name(id).await?, contest_points, contest_max_points.max(1), problems));
        }
    }

    let username = if let Some(user) = user {
        database.get_username(user).await?.unwrap_or_default()
    } else {
        String::new()
    };

    Ok(SidebarContext {
        logged_in: user.is_some(),
        username,
        contests,
    })
}

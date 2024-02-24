use crate::database::contest::ContestId;
use crate::database::problem::ProblemId;
use crate::database::user::UserId;
use crate::database::Database;
use anyhow::Result;

#[allow(clippy::type_complexity)]
pub struct SidebarContext {
    pub logged_in: bool,
    pub username: String,
    pub contests: Vec<(ContestId, String, Vec<(ProblemId, String)>)>,
}

pub async fn create_sidebar_context(database: &Database, user: Option<UserId>) -> Result<SidebarContext> {
    let mut contests = Vec::new();
    if let Some(user) = user {
        for id in database.get_contests_for_user(user).await? {
            let mut problems = Vec::new();
            for problem in database.get_problems_for_contest(id).await? {
                problems.push((problem, database.get_problem_name(problem).await?));
            }

            contests.push((id, database.get_contest_name(id).await?, problems));
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

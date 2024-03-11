use crate::contest::{create_contest_page, handle_participant_modification};
use crate::database::Database;
use crate::main_page::create_main_page;
use crate::problem::{create_edit_problem_page, create_problem_page, handle_problem_editing};
use crate::submission::{create_submission_page, handle_submission_form};
use crate::user::{create_login_page, get_login_token, handle_login_form, handle_logout_form, handle_user_creation, LoginSite};
use crate::worker::WorkerManager;
use anyhow::Result;
use askama::Template;
use http_body_util::Full;
use hyper::body::{Bytes, Incoming};
use hyper::{Request, Response};

pub fn create_raw_response(data: Bytes) -> Response<Full<Bytes>> {
    Response::new(Full::new(data))
}

pub fn create_html_response<T: Template>(site_object: &T) -> Result<Response<Full<Bytes>>> {
    Ok(create_raw_response(Bytes::from(site_object.render()?)))
}

#[derive(Template)]
#[template(path = "redirect.html")]
pub struct RedirectSite {
    pub(crate) url: String,
}

#[derive(Template)]
#[template(path = "not_found.html")]
pub struct NotFoundSite;

#[allow(clippy::too_many_lines)]
pub async fn handle_request(request: Request<Incoming>, database: Database, workers: WorkerManager) -> Result<Response<Full<Bytes>>> {
    let token = get_login_token(&request)?;
    let user = if let Some(token) = &token { database.get_user_from_token(token.clone()).await? } else { None };
    let is_admin = if let Some(user) = user { database.is_user_admin(user).await? } else { false };

    let mut parts_owned = request.uri().path().split('/').map(ToOwned::to_owned).collect::<Vec<String>>();
    let mut parts = parts_owned.iter_mut().map(|x| &**x).collect::<Vec<&str>>();
    parts.retain(|x| !x.is_empty());

    if request.method() == hyper::Method::POST {
        if parts == ["login"] {
            return handle_login_form(&database, request).await;
        }

        if let Some(user) = user {
            if parts == ["logout"] {
                return handle_logout_form(&database, token).await;
            }

            if parts == ["create_user"] {
                return handle_user_creation(&database, request, is_admin).await;
            }

            if parts.len() == 5 && parts.first().unwrap_or(&"") == &"contest" && parts.get(2).unwrap_or(&"") == &"problem" && parts.get(4).unwrap_or(&"") == &"submit_file" {
                return handle_submission_form(&database, user, parts.get(1).unwrap_or(&""), parts.get(3).unwrap_or(&""), request, &workers)
                    .await?
                    .map_or_else(|| create_html_response(&NotFoundSite), Ok);
            }

            if parts.len() == 2 && parts.first().unwrap_or(&"") == &"modify_participants" {
                return handle_participant_modification(&database, parts.get(1).unwrap_or(&""), user, request)
                    .await?
                    .map_or_else(|| create_html_response(&NotFoundSite), Ok);
            }

            if parts.len() == 4 && parts.first().unwrap_or(&"") == &"contest" && parts.get(2).unwrap_or(&"") == &"edit_problem" {
                return handle_problem_editing(&database, parts.get(1).unwrap_or(&""), parts.get(3).unwrap_or(&""), user, request)
                    .await?
                    .map_or_else(|| create_html_response(&NotFoundSite), Ok);
            }
        } else {
            return create_html_response(&LoginSite {
                error_message: "You must be logged in to perform this action".to_owned(),
            });
        }
    } else if request.method() == hyper::Method::GET {
        // if the path is empty, we are at the root of the website
        if parts.is_empty() {
            return create_main_page(&database, user).await;
        }

        if parts.len() == 2 && parts.first().unwrap_or(&"") == &"css" {
            let res = match *parts.get(1).unwrap_or(&"") {
                "main.css" => Some(include_bytes!("../templates/css/main.css").to_vec()),
                "sidebar.css" => Some(include_bytes!("../templates/css/sidebar.css").to_vec()),
                "score.css" => Some(include_bytes!("../templates/css/score.css").to_vec()),
                "problem.css" => Some(include_bytes!("../templates/css/problem.css").to_vec()),
                "contest.css" => Some(include_bytes!("../templates/css/contest.css").to_vec()),
                "submission.css" => Some(include_bytes!("../templates/css/submission.css").to_vec()),
                "login.css" => Some(include_bytes!("../templates/css/login.css").to_vec()),
                "edit_problem.css" => Some(include_bytes!("../templates/css/edit_problem.css").to_vec()),
                _ => None,
            };

            if let Some(res) = res {
                return Ok(Response::new(Full::new(res.into())));
            }
        }

        if parts == ["img", "logo.png"] {
            return Ok(Response::new(Full::new(include_bytes!("../templates/img/logo.png").to_vec().into())));
        }

        // if the path is ["login"], we are at the login page
        if parts == ["login"] {
            return create_login_page();
        }

        if let Some(user) = user {
            if parts.len() == 2 && parts.first().unwrap_or(&"") == &"contest" {
                if let Some(result) = create_contest_page(&database, parts.get(1).unwrap_or(&""), user).await? {
                    return Ok(result);
                }
            }

            if parts.len() == 4 && parts.first().unwrap_or(&"") == &"contest" && parts.get(2).unwrap_or(&"") == &"problem" {
                if let Some(result) = create_problem_page(&database, parts.get(1).unwrap_or(&""), parts.get(3).unwrap_or(&""), user).await? {
                    return Ok(result);
                }
            }

            if parts.len() == 4 && parts.first().unwrap_or(&"") == &"contest" && parts.get(2).unwrap_or(&"") == &"edit_problem" && is_admin {
                if let Some(result) = create_edit_problem_page(&database, parts.get(1).unwrap_or(&""), parts.get(3).unwrap_or(&""), user).await? {
                    return Ok(result);
                }
            }

            if parts.len() == 6 && parts.first().unwrap_or(&"") == &"contest" && parts.get(2).unwrap_or(&"") == &"problem" && parts.get(4).unwrap_or(&"") == &"submission" {
                if let Some(result) = create_submission_page(&database, parts.get(5).unwrap_or(&""), user).await? {
                    return Ok(result);
                }
            }

            if is_admin && parts.len() == 2 && (parts.first().unwrap_or(&"") == &"test_input" || parts.first().unwrap_or(&"") == &"test_output") {
                if let Ok(test_id) = parts.get(1).unwrap_or(&"").parse::<i32>() {
                    let test_input = if parts.first().unwrap_or(&"") == &"test_input" {
                        database.get_test_data(test_id).await?.0
                    } else {
                        database.get_test_data(test_id).await?.1
                    };
                    return Ok(create_raw_response(Bytes::from(test_input)));
                }
            }
        } else {
            return create_html_response(&LoginSite {
                error_message: "You must be logged in to perform this action".to_owned(),
            });
        }
    }

    create_html_response(&NotFoundSite)
}

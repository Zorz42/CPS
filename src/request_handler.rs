use crate::contest::create_contest_page;
use crate::database::Database;
use crate::main_page::create_main_page;
use crate::problem::create_problem_page;
use crate::submission::{create_submission_page, handle_submission_form};
use crate::user::{create_login_page, get_login_token, handle_login_form, handle_logout_form, LoginSite};
use crate::worker::WorkerManager;
use anyhow::Result;
use askama::Template;
use http_body_util::Full;
use hyper::body::{Bytes, Incoming};
use hyper::{Request, Response};

pub fn create_html_response<T: Template>(site_object: &T) -> Result<Response<Full<Bytes>>> {
    let response = Response::new(Full::new(Bytes::from(site_object.render()?)));
    Ok(response)
}

#[derive(Template)]
#[template(path = "redirect.html")]
pub struct RedirectSite {
    pub(crate) url: String,
}

#[derive(Template)]
#[template(path = "not_found.html")]
pub struct NotFoundSite;

pub async fn handle_request(request: Request<Incoming>, database: Database, workers: WorkerManager) -> Result<Response<Full<Bytes>>> {
    let token = get_login_token(&request)?;
    let user = if let Some(token) = &token { database.get_user_from_token(token.clone()).await? } else { None };

    let mut parts_owned = request.uri().path().split('/').map(ToOwned::to_owned).collect::<Vec<String>>();
    let mut parts = parts_owned.iter_mut().map(|x| &**x).collect::<Vec<&str>>();
    parts.retain(|x| !x.is_empty());

    if request.method() == hyper::Method::POST {
        if parts == ["login"] {
            return handle_login_form(&database, request).await;
        }

        if user.is_none() {
            return create_html_response(&LoginSite {
                error_message: "You must be logged in to perform this action".to_owned(),
            });
        }

        if parts == ["logout"] {
            return handle_logout_form(&database, token).await;
        }

        if parts.len() == 5 && parts.first().unwrap_or(&"") == &"contest" && parts.get(2).unwrap_or(&"") == &"problem" && parts.get(4).unwrap_or(&"") == &"submit_file" {
            if let Some(result) = handle_submission_form(&database, user, parts.get(1).unwrap_or(&""), parts.get(3).unwrap_or(&""), request, &workers).await? {
                return Ok(result);
            }
        }
    } else if request.method() == hyper::Method::GET {
        // if the path is empty, we are at the root of the website
        if parts.is_empty() {
            return create_main_page(&database, user).await;
        }

        if parts == ["css", "sidebar.css"] {
            return Ok(Response::new(Full::new(include_bytes!("../templates/css/sidebar.css").to_vec().into())));
        }

        if parts == ["css", "big_score.css"] {
            return Ok(Response::new(Full::new(include_bytes!("../templates/css/big_score.css").to_vec().into())));
        }

        if parts == ["css", "problem.css"] {
            return Ok(Response::new(Full::new(include_bytes!("../templates/css/problem.css").to_vec().into())));
        }

        if parts == ["img", "logo.png"] {
            return Ok(Response::new(Full::new(include_bytes!("../templates/img/logo.png").to_vec().into())));
        }

        // if the path is ["login"], we are at the login page
        if parts == ["login"] {
            return create_login_page();
        }

        if user.is_none() {
            return create_html_response(&LoginSite {
                error_message: "You must be logged in to perform this action".to_owned(),
            });
        }

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

        if parts.len() == 6 && parts.first().unwrap_or(&"") == &"contest" && parts.get(2).unwrap_or(&"") == &"problem" && parts.get(4).unwrap_or(&"") == &"submission" {
            if let Some(result) = create_submission_page(&database, parts.get(5).unwrap_or(&""), user).await? {
                return Ok(result);
            }
        }
    }

    create_html_response(&NotFoundSite)
}

use crate::contest::create_contest_page;
use crate::database::Database;
use crate::main_page::create_main_page;
use crate::problem::create_problem_page;
use crate::submission::{create_submission_page, handle_submission_form};
use crate::user::{create_login_page, get_login_token, handle_login_form, handle_logout_form};
use askama::Template;
use http_body_util::Full;
use hyper::body::{Bytes, Incoming};
use hyper::{Request, Response};

pub fn create_html_response<T: Template>(site_object: T) -> anyhow::Result<Response<Full<Bytes>>> {
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

pub async fn handle_request(
    request: Request<Incoming>,
    database: Database,
) -> anyhow::Result<Response<Full<Bytes>>> {
    let token = get_login_token(&request);
    let user = if let Some(token) = &token {
        database.get_user_from_token(token.clone()).await?
    } else {
        None
    };

    let mut parts = request
        .uri()
        .path()
        .split('/')
        .map(|x| x.to_owned())
        .collect::<Vec<String>>();
    parts.retain(|x| !x.is_empty());

    if request.method() == hyper::Method::POST {
        if parts == ["login"] {
            return handle_login_form(&database, request).await;
        }

        if parts == ["logout"] {
            return handle_logout_form(&database, token).await;
        }

        if parts.len() == 5
            && parts[0] == "contest"
            && parts[2] == "problem"
            && parts[4] == "submit_file"
        {
            if let Some(result) =
                handle_submission_form(&database, user, &parts[1], &parts[3], request).await?
            {
                return Ok(result);
            }
        }
    } else if request.method() == hyper::Method::GET {
        // if the path is empty, we are at the root of the website
        if parts.is_empty() {
            return create_main_page(&database, user).await;
        }

        // if the path is ["login"], we are at the login page
        if parts == ["login"] {
            return create_login_page();
        }

        if parts.len() == 2 && parts[0] == "contest" {
            if let Some(result) = create_contest_page(&database, &parts[1]).await? {
                return Ok(result);
            }
        }

        if parts.len() == 4 && parts[0] == "contest" && parts[2] == "problem" {
            if let Some(result) = create_problem_page(&database, &parts[1], &parts[3], user).await?
            {
                return Ok(result);
            }
        }

        if parts.len() == 6
            && parts[0] == "contest"
            && parts[2] == "problem"
            && parts[4] == "submission"
        {
            if let Some(result) = create_submission_page(&database, &parts[5]).await? {
                return Ok(result);
            }
        }
    }

    Ok(create_html_response(NotFoundSite)?)
}

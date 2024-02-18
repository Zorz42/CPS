mod contest;
mod id;
mod problem;
mod submission;
mod test;
mod user;

use std::net::SocketAddr;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use crate::contest::{create_contest_page, ContestDatabase};
use crate::problem::{create_problem_page, ProblemDatabase};
use crate::submission::{handle_submission_form, SubmissionDatabase};
use crate::user::{
    create_login_page, get_login_token, handle_login_form, handle_logout_form, UserDatabase, UserId,
};
use anyhow::Result;
use askama::Template;
use http_body_util::Full;
use hyper::body::{Bytes, Incoming};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;

#[derive(Clone)]
pub struct GlobalState {
    users: Arc<Mutex<UserDatabase>>,
    contests: Arc<Mutex<ContestDatabase>>,
    problems: Arc<Mutex<ProblemDatabase>>,
    submissions: Arc<Mutex<SubmissionDatabase>>,
}

impl GlobalState {
    fn new() -> GlobalState {
        GlobalState {
            users: Arc::new(Mutex::new(UserDatabase::new())),
            contests: Arc::new(Mutex::new(ContestDatabase::new())),
            problems: Arc::new(Mutex::new(ProblemDatabase::new())),
            submissions: Arc::new(Mutex::new(SubmissionDatabase::new())),
        }
    }

    fn users(&self) -> MutexGuard<UserDatabase> {
        self.users.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn contests(&self) -> MutexGuard<ContestDatabase> {
        self.contests.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn problems(&self) -> MutexGuard<ProblemDatabase> {
        self.problems.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn submissions(&self) -> MutexGuard<SubmissionDatabase> {
        self.submissions
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }
}

pub fn create_html_response<T: Template>(site_object: T) -> Result<Response<Full<Bytes>>> {
    let response = Response::new(Full::new(Bytes::from(site_object.render()?)));
    Ok(response)
}

#[derive(Template)]
#[template(path = "redirect.html")]
pub struct RedirectSite {
    url: String,
}

#[derive(Template)]
#[template(path = "not_found.html")]
pub struct NotFoundSite;

#[derive(Template)]
#[template(path = "main.html")]
pub struct MainSite {
    logged_in: bool,
    username: String,
    contests: Vec<(u128, String)>,
}

pub fn create_main_page(
    global: &GlobalState,
    user: Option<UserId>,
) -> Result<Response<Full<Bytes>>> {
    let mut contests = Vec::new();
    if let Some(user) = user {
        let contests_obj = global.contests();
        contests = contests_obj
            .get_available_contests(user)
            .iter()
            .map(|id| {
                (
                    id.to_int(),
                    contests_obj.get_contest(*id).unwrap().name.clone(),
                )
            })
            .collect();
    }

    Ok(create_html_response(MainSite {
        logged_in: user.is_some(),
        username: user
            .map(|id| global.users().get_user(id).unwrap().username.clone())
            .unwrap_or_default(),
        contests,
    })?)
}

async fn handle_request(
    request: Request<Incoming>,
    global: &GlobalState,
) -> Result<Response<Full<Bytes>>> {
    let token = get_login_token(&request);
    let user = token.and_then(|token| global.users().get_user_id_by_token(token));

    let mut parts = request
        .uri()
        .path()
        .split('/')
        .map(|x| x.to_owned())
        .collect::<Vec<String>>();
    parts.retain(|x| !x.is_empty());

    if request.method() == hyper::Method::POST {
        if parts == ["login"] {
            return handle_login_form(global, request).await;
        }

        if parts == ["logout"] {
            return handle_logout_form(global, token).await;
        }

        if parts.len() == 5
            && parts[0] == "contest"
            && parts[2] == "problem"
            && parts[4] == "submit_file"
        {
            if let Some(result) =
                handle_submission_form(global, user, &parts[1], &parts[3], request).await?
            {
                return Ok(result);
            }
        }
    } else if request.method() == hyper::Method::GET {
        // if the path is empty, we are at the root of the website
        if parts.is_empty() {
            return create_main_page(global, user);
        }

        // if the path is ["login"], we are at the login page
        if parts == ["login"] {
            return create_login_page();
        }

        if parts.len() == 2 && parts[0] == "contest" {
            if let Some(result) = create_contest_page(global, &parts[1])? {
                return Ok(result);
            }
        }

        if parts.len() == 4 && parts[0] == "contest" && parts[2] == "problem" {
            if let Some(result) = create_problem_page(global, &parts[1], &parts[3], user)? {
                return Ok(result);
            }
        }
    }

    Ok(create_html_response(NotFoundSite)?)
}

// this function is used to initialize the temporary data
// it will be later replaced by a database
fn init_temporary_data() -> GlobalState {
    let global = GlobalState::new();
    let admin_user = global.users().add_user("admin", "admin", true);
    let contest1 = global.contests().add_contest("Contest 1");
    let _contest2 = global.contests().add_contest("Contest 2");
    let contest10 = global.contests().add_contest("Contest 10");
    global
        .contests()
        .make_contest_available(admin_user, contest1);
    global
        .contests()
        .make_contest_available(admin_user, contest10);

    let problem1 = global.problems().add_problem("Problem 1", "Description 1");
    let problem2 = global.problems().add_problem("Problem 2", "Description 2");
    let problem3 = global
        .problems()
        .add_problem("A Hard Problem", "A Hard Description");

    global.contests().add_problem_to_contest(contest1, problem1);
    global
        .contests()
        .add_problem_to_contest(contest10, problem2);
    global
        .contests()
        .add_problem_to_contest(contest10, problem3);

    global
}

#[tokio::main]
async fn main() -> Result<()> {
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    let listener = TcpListener::bind(addr).await?;
    let global = init_temporary_data();

    loop {
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);

        let global = global.clone();
        tokio::task::spawn(async move {
            println!("Got connection from: {}", io.inner().peer_addr().unwrap());

            if let Err(err) = http1::Builder::new()
                .serve_connection(io, service_fn(|request| handle_request(request, &global)))
                .await
            {
                println!("Error serving connection: {:?}", err);
            }
        });
    }
}

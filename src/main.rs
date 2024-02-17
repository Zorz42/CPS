mod contest;
mod id;
mod problem;
mod submission;
mod test;
mod user;

use std::net::SocketAddr;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use crate::contest::ContestDatabase;
use crate::problem::ProblemDatabase;
use crate::submission::SubmissionDatabase;
use crate::user::{get_login_token, parse_login_string, UserDatabase};
use anyhow::Result;
use askama::Template;
use http_body_util::{BodyExt, Full};
use hyper::body::Bytes;
use hyper::header::SET_COOKIE;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;

#[derive(Clone)]
struct GlobalState {
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

fn create_html_response<T: Template>(site_object: T) -> Result<Response<Full<Bytes>>> {
    let response = Response::new(Full::new(Bytes::from(site_object.render()?)));
    Ok(response)
}

#[derive(Template)]
#[template(path = "redirect.html")]
struct RedirectSite {
    url: String,
}

#[derive(Template)]
#[template(path = "not_found.html")]
struct NotFoundSite;

#[derive(Template)]
#[template(path = "main.html")]
struct MainSite {
    logged_in: bool,
    username: String,
    contests: Vec<(u128, String)>,
}

#[derive(Template)]
#[template(path = "login.html")]
struct LoginSite {
    error_message: String,
}

async fn handle_request(
    request: Request<hyper::body::Incoming>,
    global: &GlobalState,
) -> Result<Response<Full<Bytes>>> {
    let token = get_login_token(&request);
    let user = token.and_then(|token| global.users().get_user_id_by_token(token));

    if request.method() == hyper::Method::POST {
        match request.uri().path() {
            "/login" => {
                let body = request.into_body().collect().await?.to_bytes();
                let body = String::from_utf8_lossy(&body).to_string();
                let (username, password) = parse_login_string(&body);

                let mut users = global.users();
                return if let Some(id) = users.try_login(&username, &password) {
                    let token = users.add_token(id);

                    let mut response = create_html_response(RedirectSite {
                        url: "/".to_owned(),
                    })?;

                    response.headers_mut().append(
                        SET_COOKIE,
                        format!("login_token={}", token.to_int()).parse()?,
                    );

                    Ok(response)
                } else {
                    let error_message = {
                        if users.get_user_id_by_username(&username).is_none() {
                            "User does not exist".to_owned()
                        } else {
                            "Invalid password".to_owned()
                        }
                    };

                    let response = create_html_response(LoginSite { error_message })?;

                    Ok(response)
                };
            }
            "/logout" => {
                let response = create_html_response(RedirectSite {
                    url: "/".to_owned(),
                })?;

                if let Some(token) = token {
                    global.users().remove_token(token);
                }

                return Ok(response);
            }
            _ => {}
        }
    }

    let mut parts = request.uri().path().split('/').collect::<Vec<_>>();
    parts.retain(|x| !x.is_empty());

    // if the path is empty, we are at the root of the website
    if parts.is_empty() {
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

        return Ok(create_html_response(MainSite {
            logged_in: user.is_some(),
            username: user
                .map(|id| global.users().get_user(id).unwrap().username.clone())
                .unwrap_or_default(),
            contests,
        })?);
    }

    // if the path is ["login"], we are at the login page
    if parts == ["login"] {
        return Ok(create_html_response(LoginSite {
            error_message: "".to_owned(),
        })?);
    }

    Ok(create_html_response(NotFoundSite)?)
}

// this function is used to initialize the temporary data
// it will be later replaced by a database
fn init_temporary_data() -> GlobalState {
    let global = GlobalState::new();
    let admin_user = global.users().add_user("admin", "admin", true);
    let contest_1 = global.contests().add_contest("Contest 1");
    let _contest_2 = global.contests().add_contest("Contest 2");
    let contest_10 = global.contests().add_contest("Contest 10");
    global
        .contests()
        .make_contest_available(admin_user, contest_1);
    global
        .contests()
        .make_contest_available(admin_user, contest_10);

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

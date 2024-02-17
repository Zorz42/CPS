mod user;

use std::net::SocketAddr;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use crate::user::UserDatabase;
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
}

impl GlobalState {
    fn new() -> GlobalState {
        GlobalState {
            users: Arc::new(Mutex::new(UserDatabase::new())),
        }
    }

    fn users(&self) -> MutexGuard<UserDatabase> {
        self.users.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

fn parse_login_string(body: &str) -> (String, String) {
    let mut username = String::new();
    let mut password = String::new();

    for part in body.split('&') {
        let parts: Vec<&str> = part.split('=').collect();
        if parts.len() != 2 {
            continue;
        }

        match parts[0] {
            "username" => username = parts[1].to_string(),
            "password" => password = parts[1].to_string(),
            _ => {}
        }
    }

    (username, password)
}

fn get_login_token(request: &Request<hyper::body::Incoming>) -> Option<u128> {
    request
        .headers()
        .get("cookie")
        .and_then(|cookie| cookie.to_str().ok())
        .and_then(|cookie| {
            for part in cookie.split(';') {
                let parts: Vec<&str> = part.trim().split('=').collect();
                if parts.len() != 2 {
                    continue;
                }

                if parts[0] == "login_token" {
                    return Some(parts[1].parse().unwrap());
                }
            }

            None
        })
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
#[template(path = "main.html")]
struct MainSite {
    logged_in: bool,
    username: String,
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
        let body = request.into_body().collect().await?.to_bytes();
        let body = String::from_utf8_lossy(&body).to_string();
        let (username, password) = parse_login_string(&body);

        let mut users = global.users();
        return if let Some(id) = users.try_login(&username, &password) {
            let user = users.get_user_id_by_username(&username).unwrap();
            let token = users.add_token(user);

            let mut response = create_html_response(RedirectSite {
                url: "/".to_owned(),
            })?;

            response
                .headers_mut()
                .append(SET_COOKIE, format!("login_token={}", token).parse()?);

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

    return match request.uri().path() {
        "/" => Ok(create_html_response(MainSite {
            logged_in: user.is_some(),
            username: user
                .map(|id| global.users().get_user(id).unwrap().username.clone())
                .unwrap_or_default(),
        })?),
        "/login" => Ok(create_html_response(LoginSite {
            error_message: "".to_owned(),
        })?),
        _ => Ok(Response::new(Full::new(Bytes::from(include_str!(
            "../templates/not_found.html"
        ))))),
    };
}

#[tokio::main]
async fn main() -> Result<()> {
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    let listener = TcpListener::bind(addr).await?;
    let global = GlobalState::new();

    global.users().add_user("admin", "admin");

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

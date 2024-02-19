mod contest;
mod database;
mod problem;
mod submission;
mod test;
mod user;

use std::net::SocketAddr;

use crate::contest::create_contest_page;
use crate::database::Database;
use crate::problem::create_problem_page;
use crate::submission::handle_submission_form;
use crate::user::{
    create_login_page, get_login_token, handle_login_form, handle_logout_form, UserId,
};
use anyhow::Result;
use askama::Template;
use http_body_util::Full;
use hyper::body::{Bytes, Incoming};
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper::client::conn::http2;
use hyper_util::rt::{TokioExecutor, TokioIo};
use rustls::ServerConfig;
use tokio::net::TcpListener;

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
    contests: Vec<(i32, String)>,
}

pub async fn create_main_page(
    database: &Database,
    user: Option<UserId>,
) -> Result<Response<Full<Bytes>>> {
    let mut contests = Vec::new();
    if let Some(user) = user {
        for id in database.get_contests_for_user(user).await {
            contests.push((id, database.get_contest_name(id).await));
        }
    }

    let username = if let Some(user) = user {
        database.get_username(user).await?.unwrap_or_default()
    } else {
        "".to_owned()
    };

    Ok(create_html_response(MainSite {
        logged_in: user.is_some(),
        username,
        contests,
    })?)
}

async fn handle_request(
    request: Request<Incoming>,
    database: &Database,
) -> Result<Response<Full<Bytes>>> {
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
            return handle_login_form(database, request).await;
        }

        if parts == ["logout"] {
            return handle_logout_form(database, token).await;
        }

        if parts.len() == 5
            && parts[0] == "contest"
            && parts[2] == "problem"
            && parts[4] == "submit_file"
        {
            if let Some(result) =
                handle_submission_form(database, user, &parts[1], &parts[3], request).await?
            {
                return Ok(result);
            }
        }
    } else if request.method() == hyper::Method::GET {
        // if the path is empty, we are at the root of the website
        if parts.is_empty() {
            return create_main_page(database, user).await;
        }

        // if the path is ["login"], we are at the login page
        if parts == ["login"] {
            return create_login_page();
        }

        if parts.len() == 2 && parts[0] == "contest" {
            if let Some(result) = create_contest_page(database, &parts[1]).await? {
                return Ok(result);
            }
        }

        if parts.len() == 4 && parts[0] == "contest" && parts[2] == "problem" {
            if let Some(result) = create_problem_page(database, &parts[1], &parts[3], user).await? {
                return Ok(result);
            }
        }
    }

    Ok(create_html_response(NotFoundSite)?)
}

// this function is used to initialize the temporary data
// it will be later replaced by a database
async fn init_temporary_data(database: &Database) {
    let admin_user = database
        .add_user_override("admin", "admin", true)
        .await
        .unwrap();
    let contest1 = database.add_contest_override("Contest 1").await;
    let _contest2 = database.add_contest_override("Contest 2").await;
    let contest10 = database.add_contest_override("Contest 10").await;
    database.add_user_to_contest(admin_user, contest1).await;
    database.add_user_to_contest(admin_user, contest10).await;

    let problem1 = database
        .add_problem_override("Problem 1", "Description 1")
        .await;
    let problem2 = database
        .add_problem_override("Problem 2", "Description 2")
        .await;
    let problem3 = database
        .add_problem_override("A Hard Problem", "A Hard Description")
        .await;

    database.add_problem_to_contest(contest1, problem1).await;
    database.add_problem_to_contest(contest10, problem2).await;
    database.add_problem_to_contest(contest10, problem3).await;
}

pub fn get_server_config() -> Result<ServerConfig> {
    //get key and certificate from files in ./cert/fullchain.pem and ./cert/privkey.pem
    let mut cert_file = std::io::BufReader::new(std::fs::File::open("./cert/fullchain1.pem")?);
    let mut key_file = std::io::BufReader::new(std::fs::File::open("./cert/privkey1.pem")?);

    let certificates = rustls_pemfile::certs(&mut cert_file);
    let certificates = certificates.filter_map(Result::ok).collect();
    let mut keys = rustls_pemfile::pkcs8_private_keys(&mut key_file);
    let key = keys
        .next()
        .ok_or_else(|| anyhow::anyhow!("error getting a key"))??;
    let key = rustls_pki_types::PrivateKeyDer::Pkcs8(key);

    //build server config
    Ok(ServerConfig::builder()
        .with_no_client_auth() //for now, i'll have to check what this is and verify things
        .with_single_cert(certificates, key)?)
}

#[tokio::main]
async fn main() -> Result<()> {
    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    let listener = TcpListener::bind(addr).await?;
    let database = Database::new().await?;
    database.init_users().await;
    database.init_contests().await?;
    database.init_problems().await?;
    database.init_submissions().await;
    init_temporary_data(&database).await;

    let mut server_config = get_server_config()?;
    server_config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec(), b"http/1.0".to_vec(), b"http/1.2".to_vec()];
    let tls_acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(server_config));

    loop {
        let (mut tcp_stream, _) = listener.accept().await?;
        let tls_acceptor = tls_acceptor.clone();
        //let io = TokioIo::new(stream);

        let database = database.clone();
        tokio::task::spawn(async move {
            println!("Got connection from: {}", tcp_stream.peer_addr().unwrap().ip());
            let tls_stream = tls_acceptor.accept(tcp_stream).await.unwrap();

            if let Err(err) = hyper_util::server::conn::auto::Builder::new()
                .serve_connection(io, service_fn(|request| handle_request(request, &database)))
                .await
            {
                println!("Error serving connection: {:?}", err);
            }
        });
    }
}

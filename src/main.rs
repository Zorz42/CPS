mod contest;
mod database;
mod main_page;
mod problem;
mod request_handler;
mod sidebar;
mod submission;
mod tester;
mod user;
mod worker;

use std::net::SocketAddr;
use std::sync::Arc;

use crate::database::Database;
use crate::request_handler::handle_request;
use crate::tester::is_isolate_installed;
use crate::worker::WorkerManager;
use anyhow::{bail, Result};
use hyper::service::service_fn;
use hyper_util::rt::{TokioExecutor, TokioIo};
use tokio::net::TcpListener;
use tokio_rustls::rustls::ServerConfig;

// this function is used to initialize the temporary data
// it will be later replaced by a database
#[allow(dead_code)]
async fn init_temporary_data(database: &Database) -> Result<()> {
    let admin_user = database.add_user_override("admin", "admin", true).await?;
    let contest1 = database.add_contest_override("Contest 1").await?;
    let _contest2 = database.add_contest_override("Contest 2").await;
    let contest10 = database.add_contest_override("Contest 10").await?;
    database.add_user_to_contest(admin_user, contest1).await?;
    database.add_user_to_contest(admin_user, contest10).await?;

    let problem1 = database.add_problem_override("Problem 1", "You get a and b and you have to return a + b.", 1000).await?;
    let problem2 = database.add_problem_override("Problem 2", "Description 2", 1000).await?;
    let problem3 = database.add_problem_override("A Hard Problem", "A Hard Description", 1000).await?;

    database.add_problem_to_contest(contest1, problem1).await?;
    database.add_problem_to_contest(contest10, problem2).await?;
    database.add_problem_to_contest(contest10, problem3).await?;

    // subtask1: small inputs
    let subtask1 = database.add_subtask(problem1, 30).await?;
    // subtask2: large inputs
    let subtask2 = database.add_subtask(problem1, 30).await?;
    // subtask3: negative inputs
    let subtask3 = database.add_subtask(problem1, 40).await?;

    let tests = vec![("1 2", "3"), ("3 4", "7"), ("5 6", "11"), ("7 8", "15")];
    for (input, output) in tests {
        database.add_test_to_subtask(subtask1, database.add_test(input, output, problem1).await?).await?;
        database.add_test_to_subtask(subtask2, database.add_test(input, output, problem1).await?).await?;
        database.add_test_to_subtask(subtask3, database.add_test(input, output, problem1).await?).await?;
    }

    let tests = vec![
        ("1000000000 1000000000", "2000000000"),
        ("1000000000 1000000001", "2000000001"),
        ("1000000000 1000000002", "2000000002"),
        ("1000000000 1000000003", "2000000003"),
        ("1000000000000 1000000000000", "2000000000000"),
    ];

    for (input, output) in tests {
        database.add_test_to_subtask(subtask2, database.add_test(input, output, problem1).await?).await?;
    }

    let tests = vec![("-1 -2", "-3"), ("-3 -4", "-7"), ("-5 -6", "-11"), ("-7 -8", "-15")];
    for (input, output) in tests {
        database.add_test_to_subtask(subtask3, database.add_test(input, output, problem1).await?).await?;
    }

    // note: this task and these tests are obviously a joke for testing purposes
    Ok(())
}

pub fn get_server_config() -> Result<ServerConfig> {
    // get key and certificate from files in ./cert/fullchain.pem and ./cert/privkey.pem
    let mut cert_file = std::io::BufReader::new(std::fs::File::open("./cert/fullchain1.pem")?);
    let mut key_file = std::io::BufReader::new(std::fs::File::open("./cert/privkey1.pem")?);

    let certificates = rustls_pemfile::certs(&mut cert_file);
    let certificates = certificates.filter_map(Result::ok).collect();
    let mut keys = rustls_pemfile::pkcs8_private_keys(&mut key_file);
    let key = keys.next().ok_or_else(|| anyhow::anyhow!("error getting a key"))??;
    let key = rustls_pki_types::PrivateKeyDer::Pkcs8(key);

    // build server config
    Ok(ServerConfig::builder()
        .with_no_client_auth() //for now, i'll have to check what this is and verify things
        .with_single_cert(certificates, key)?)
}

#[tokio::main]
async fn main() -> Result<()> {
    let port = 3000;
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = TcpListener::bind(addr).await?;
    let database = Database::new().await?;
    database.init_users().await?;
    database.init_contests().await?;
    database.init_problems().await?;
    database.init_submissions().await?;
    database.init_tests().await?;

    if !is_isolate_installed().await {
        println!(
            "Warning: isolate is not installed. This means that the testing system will be unsafe. Please install isolate to ensure that arbitrary code sent by users is run in a safe environment."
        );
    }

    // init_temporary_data(&database).await?; // this should be called once and then it stays in the database

    let workers = WorkerManager::new(32, &database);

    let server_config = get_server_config();
    let tls_acceptor = if let Ok(mut server_config) = server_config {
        server_config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec(), b"http/1.0".to_vec(), b"http/1.2".to_vec()];
        Some(tokio_rustls::TlsAcceptor::from(Arc::new(server_config)))
    } else {
        println!("Error getting server config");
        println!("If you wish to use TLS (https), please provide a valid certificate and key in the ./cert/ directory.");
        println!("Reverting to http...");
        None
    };

    println!("Server is now running on port {port}.");

    loop {
        let tcp_stream = listener.accept().await?.0;
        let tls_acceptor = tls_acceptor.clone();

        let database = database.clone();
        let workers = workers.clone();
        tokio::task::spawn(async move {
            println!("Got connection from: {}", tcp_stream.peer_addr()?.ip());

            let tokio_builder = hyper_util::server::conn::auto::Builder::new(TokioExecutor::new());
            let service = service_fn(move |request| handle_request(request, database.clone(), workers.clone()));

            let result = if let Some(tls_acceptor) = tls_acceptor {
                let stream = tls_acceptor.accept(tcp_stream).await;
                match stream {
                    Ok(stream) => tokio_builder.serve_connection(TokioIo::new(stream), service).await,
                    Err(err) => {
                        println!("Error accepting TLS connection: {:?}", err);
                        return;
                    }
                }
            } else {
                tokio_builder.serve_connection(TokioIo::new(tcp_stream), service).await
            };

            if let Err(err) = result {
                println!("Error serving connection: {:?}", err);
            }
        });
    }
}

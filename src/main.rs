use std::convert::Infallible;
use std::net::SocketAddr;

use anyhow::Result;
use http_body_util::{BodyExt, Full};
use hyper::body::Bytes;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;

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

async fn handle_request(
    request: Request<hyper::body::Incoming>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    //println!("Got request {:?}", request);
    if request.method() == hyper::Method::POST {
        let body = request.into_body().collect().await.unwrap().to_bytes();
        let body = String::from_utf8_lossy(&body).to_string();
        let (username, password) = parse_login_string(&body);

        println!("Got login info: {}, {}", username, password);
        return Ok(Response::new(Full::new(Bytes::from("Got POST request"))));
    }

    return match request.uri().path() {
        "/login" => Ok(Response::new(Full::new(Bytes::from(include_str!(
            "../html/login.html"
        ))))),
        _ => Ok(Response::new(Full::new(Bytes::from(include_str!(
            "../html/not_found.html"
        ))))),
    };
}

#[tokio::main]
async fn main() -> Result<()> {
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    let listener = TcpListener::bind(addr).await?;

    loop {
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);

        tokio::task::spawn(async move {
            println!("Got connection from: {}", io.inner().peer_addr().unwrap());

            if let Err(err) = http1::Builder::new()
                .serve_connection(io, service_fn(handle_request))
                .await
            {
                println!("Error serving connection: {:?}", err);
            }
        });
    }
}

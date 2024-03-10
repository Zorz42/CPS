// this is a bombarding test which means it will run the server with a lot of requests and check if it can handle it

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod bombardment_tests {
    use crate::{create_database, get_config};

    async fn bombard_url(url: &str) {
        let config = get_config("testing_config.toml").unwrap();
        let port = config.port;
        let database = create_database(&config).await.unwrap();
        let test_user = database.add_user_override("test", "test", true).await.unwrap();
        let token = database.add_token(test_user).await.unwrap();

        let server = tokio::spawn(async move {
            crate::run_server(&config, &database).await.unwrap();
        });

        let url = format!("http://127.0.0.1:{port}/{url}").parse::<reqwest::Url>().unwrap();

        let cookies = format!("login_token={token}");

        let mut request_headers = reqwest::header::HeaderMap::new();
        request_headers.insert(reqwest::header::COOKIE, reqwest::header::HeaderValue::from_bytes(cookies.as_bytes()).unwrap());

        let client = reqwest::ClientBuilder::new().default_headers(request_headers).cookie_store(true).build().unwrap();

        let mut tasks = Vec::new();
        for i in 0..10000 {
            let client = client.clone();
            let url = url.clone();
            tasks.push(tokio::spawn(async move {
                let response = client.get(url).send().await.unwrap();
                assert!(response.status().is_success());
            }));
            if i > 200 {
                // wait a bit
                tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;
            }
        }

        for task in tasks {
            task.await.unwrap();
        }

        server.abort();
    }

    #[tokio::test]
    #[ignore]
    async fn bombardment_main_page() {
        bombard_url("").await;
    }
}

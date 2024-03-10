// this is a bombarding test which means it will run the server with a lot of requests and check if it can handle it

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod bombardment_tests {
    use crate::database::Database;
    use crate::{create_database, get_config, Config};

    const CONFIG_FILE: &str = "testing_config.toml";

    async fn start_server(config: Config, database: Database) -> (tokio::task::JoinHandle<()>, String) {
        let test_user = database.add_user_override("test", "test", true).await.unwrap();
        let token = database.add_token(test_user).await.unwrap();

        let server = tokio::spawn(async move {
            crate::run_server(&config, &database).await.unwrap();
        });

        let cookies = format!("login_token={token}");

        (server, cookies)
    }

    async fn bombard_url(cookies: String, port: u16, url: &str) {
        tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;
        let url = format!("http://127.0.0.1:{port}/{url}").parse::<reqwest::Url>().unwrap();

        let mut request_headers = reqwest::header::HeaderMap::new();
        request_headers.insert(reqwest::header::COOKIE, reqwest::header::HeaderValue::from_bytes(cookies.as_bytes()).unwrap());

        let client = reqwest::ClientBuilder::new().default_headers(request_headers).cookie_store(true).build().unwrap();

        let mut tasks = Vec::new();
        for i in 0..1000 {
            let client = client.clone();
            let url = url.clone();
            tasks.push(tokio::spawn(async move {
                let response = client.get(url).send().await.unwrap();
                assert!(response.status().is_success());
            }));
            if i > 100 {
                // wait a bit
                tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;
            }
        }

        for task in tasks {
            task.await.unwrap();
        }
    }

    #[tokio::test]
    #[ignore]
    async fn bombardment_main_page() {
        let config = get_config(CONFIG_FILE).unwrap();
        let port = config.port;
        let database = create_database(&config).await.unwrap();

        let (server, cookies) = start_server(config.clone(), database.clone()).await;

        // literally bombard almost every url on the site ;)

        bombard_url(cookies.clone(), port, "").await;
        bombard_url(cookies.clone(), port, "login").await;

        let contests = database.get_all_contests().await.unwrap();

        for contest in contests {
            bombard_url(cookies.clone(), port, &format!("contest/{contest}")).await;

            let problems = database.get_problems_for_contest(contest).await.unwrap();

            for problem in problems {
                bombard_url(cookies.clone(), port, &format!("contest/{contest}/problem/{problem}")).await;
                bombard_url(cookies.clone(), port, &format!("contest/{contest}/edit_problem/{problem}")).await;
            }
        }

        server.abort();
    }
}

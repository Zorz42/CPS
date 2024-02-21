pub mod contest;
pub mod problem;
pub mod submission;
pub mod test;
pub mod user;

use anyhow::Result;
use std::env;
use std::sync::Arc;

const DB_NAME: &str = "cps";

#[derive(Clone)]
pub struct Database {
    postgres_client: Arc<tokio_postgres::Client>,
}

impl Database {
    pub async fn new() -> Result<Database> {
        let args: Vec<String> = env::args().collect();
        let username = args.get(1).ok_or(anyhow::anyhow!("no username argument"))?;
        let password = args.get(2).ok_or(anyhow::anyhow!("no password argument"))?;
        let host = args.get(3).ok_or(anyhow::anyhow!("no host argument"))?;
        let (client, connection) = tokio_postgres::connect(
            &format!("host={host} user={username} password={password} dbname={DB_NAME}"),
            tokio_postgres::NoTls,
        )
            .await?;

        // Spawn a new task to process the connection in the background.
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("connection error: {}", e);
            }
        });

        Ok(Database {
            postgres_client: Arc::new(client),
        })
    }

    pub fn get_postgres_client(&self) -> &tokio_postgres::Client {
        &self.postgres_client
    }
}

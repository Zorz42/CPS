use anyhow::Result;

const DB_NAME: &str = "cps";

pub struct Database {
    postgres_client: tokio_postgres::Client,
}

impl Database {
    pub async fn new(address: &str, user: &str, password: &str) -> Result<Database> {
        let (client, connection) = tokio_postgres::connect(
            &format!("host={address} user={user} password={password} dbname={DB_NAME}"),
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
            postgres_client: client,
        })
    }

    pub fn get_postgres_client(&self) -> &tokio_postgres::Client {
        &self.postgres_client
    }
}

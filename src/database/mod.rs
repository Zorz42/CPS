pub mod contest;
pub mod problem;
pub mod submission;
pub mod test;
pub mod user;

use anyhow::Result;
use std::env;
use std::sync::{Arc, OnceLock};
use tokio_postgres::types::ToSql;
use tokio_postgres::{Row, Statement};

const DB_NAME: &str = "cps";

#[derive(Clone)]
pub struct Database {
    postgres_client: Arc<tokio_postgres::Client>,
}

impl Database {
    pub async fn new() -> Result<Self> {
        let args: Vec<String> = env::args().collect();
        let username = args.get(1).ok_or_else(|| anyhow::anyhow!("no username argument"))?;
        let password = args.get(2).ok_or_else(|| anyhow::anyhow!("no password argument"))?;
        let host = args.get(3).ok_or_else(|| anyhow::anyhow!("no host argument"))?;
        let (client, connection) = tokio_postgres::connect(&format!("host={host} user={username} password={password} dbname={DB_NAME}"), tokio_postgres::NoTls).await?;

        // Spawn a new task to process the connection in the background.
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("connection error: {e}");
            }
        });

        Ok(Self { postgres_client: Arc::new(client) })
    }

    pub fn get_postgres_client(&self) -> &tokio_postgres::Client {
        &self.postgres_client
    }
}

/// this is a struct that is static and holds a prepared query to the database
pub struct DatabaseQuery {
    query: &'static str,
    prepared: OnceLock<Statement>,
}

impl DatabaseQuery {
    pub const fn new(query: &'static str) -> Self {
        Self { query, prepared: OnceLock::new() }
    }

    async fn get_query(&self, database: &Database) -> Result<Statement> {
        if let Some(prepared) = self.prepared.get() {
            return Ok(prepared.clone());
        }

        let prepared = database.get_postgres_client().prepare(self.query).await?;
        Ok(self.prepared.get_or_init(|| prepared.clone()).clone())
    }

    pub async fn execute(&self, database: &Database, params: &[&(dyn ToSql + Sync)]) -> Result<Vec<Row>> {
        let query = self.get_query(database).await?;
        Ok(database.get_postgres_client().query(&query, params).await?)
    }
}

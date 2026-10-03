//! The database as the handlers see it: batches of statements built by [`crate::sql`].

use std::future::Future;
use std::pin::Pin;

use serde::de::DeserializeOwned;

use crate::sql::Stmt;

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// The rows one statement returned, each a JSON object keyed by column name.
#[derive(Debug, Default)]
pub struct Rows(pub Vec<serde_json::Value>);

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    /// A key or check of the schema rejected the batch.
    #[error("a database constraint rejected the write")]
    Constraint,
    #[error("database: {0}")]
    Other(String),
}

pub trait Db: Send + Sync {
    /// Runs the statements as one transaction and returns the rows of each.
    fn batch(&self, statements: Vec<Stmt>) -> BoxFuture<'_, Result<Vec<Rows>, DbError>>;
}

impl Rows {
    pub fn all<T: DeserializeOwned>(self) -> Result<Vec<T>, DbError> {
        self.0.into_iter().map(|row| serde_json::from_value(row).map_err(|e| DbError::Other(e.to_string()))).collect()
    }

    /// The row of a statement that returns at most one.
    pub fn first<T: DeserializeOwned>(self) -> Result<Option<T>, DbError> {
        Ok(self.all()?.into_iter().next())
    }
}

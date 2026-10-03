//! [`Db`] over an in-memory SQLite database with the real schema, for tests on the host.

use std::sync::Mutex;

use rusqlite::types::{Value, ValueRef};
use rusqlite::{Connection, ErrorCode};

use crate::db::{BoxFuture, Db, DbError, Rows};
use crate::sql::{Param, Stmt};

pub struct SqliteDb(Mutex<Connection>);

impl SqliteDb {
    pub fn new() -> Self {
        let connection = Connection::open_in_memory().expect("open an in-memory database");
        // D1 enforces foreign keys; SQLite does only when asked.
        connection.execute_batch("PRAGMA foreign_keys = ON").expect("enable foreign keys");
        connection.execute_batch(include_str!("../migrations/0001_init.sql")).expect("apply the schema");
        Self(Mutex::new(connection))
    }
}

impl Default for SqliteDb {
    fn default() -> Self {
        Self::new()
    }
}

impl Db for SqliteDb {
    fn batch(&self, statements: Vec<Stmt>) -> BoxFuture<'_, Result<Vec<Rows>, DbError>> {
        Box::pin(async move {
            let mut connection = self.0.lock().expect("the database lock is not poisoned");
            let transaction = connection.transaction().map_err(error)?;
            let results = statements.iter().map(|s| run(&transaction, s)).collect::<Result<_, _>>().map_err(error)?;
            transaction.commit().map_err(error)?;
            Ok(results)
        })
    }
}

fn run(connection: &Connection, statement: &Stmt) -> rusqlite::Result<Rows> {
    let mut prepared = connection.prepare(statement.sql())?;
    let names: Vec<String> = prepared.column_names().into_iter().map(str::to_owned).collect();
    let params = statement.params().iter().map(|p| match p {
        Param::Null => Value::Null,
        Param::Int(i) => Value::Integer(*i),
        Param::Text(s) => Value::Text(s.clone()),
    });
    let mut rows = prepared.query(rusqlite::params_from_iter(params))?;
    let mut objects = Vec::new();
    while let Some(row) = rows.next()? {
        let mut object = serde_json::Map::new();
        for (index, name) in names.iter().enumerate() {
            let value = match row.get_ref(index)? {
                ValueRef::Null => serde_json::Value::Null,
                ValueRef::Integer(i) => i.into(),
                ValueRef::Text(text) => String::from_utf8_lossy(text).into_owned().into(),
                other => unreachable!("the schema has only integer and text columns, not {other:?}"),
            };
            object.insert(name.clone(), value);
        }
        objects.push(object.into());
    }
    Ok(Rows(objects))
}

fn error(e: rusqlite::Error) -> DbError {
    match e.sqlite_error_code() {
        Some(ErrorCode::ConstraintViolation) => DbError::Constraint,
        _ => DbError::Other(e.to_string()),
    }
}

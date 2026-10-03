//! Every SQL statement of the server.
//!
//! A [`Stmt`] can only be made here, from a sea-query statement, so no request
//! data ever becomes SQL text: it reaches the database as a bound parameter.

use sea_query::{
    Expr, ExprTrait, Func, Iden, JoinType, OnConflict, Order, Query, QueryStatementWriter, SimpleExpr,
    SqliteQueryBuilder, Value, extension::sqlite::SqliteExpr,
};
use serde::Deserialize;
use serde_json::json;
use topo_core::wire::Role;
use topo_core::{Kind, Node, NodeId, Priority, Status};

pub struct Stmt {
    sql: String,
    params: Vec<Param>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Param {
    Null,
    Int(i64),
    Text(String),
}

impl Stmt {
    pub fn sql(&self) -> &str {
        &self.sql
    }

    pub fn params(&self) -> &[Param] {
        &self.params
    }
}

fn stmt(query: &impl QueryStatementWriter) -> Stmt {
    let (sql, values) = query.build(SqliteQueryBuilder);
    let params = values
        .0
        .into_iter()
        .map(|value| match value {
            Value::Bool(Some(b)) => Param::Int(b.into()),
            Value::BigInt(Some(i)) => Param::Int(i),
            Value::BigUnsigned(Some(u)) => Param::Int(u as i64),
            Value::String(Some(s)) => Param::Text(s),
            Value::BigInt(None) | Value::String(None) => Param::Null,
            other => unreachable!("statements bind only integers and text, not {other:?}"),
        })
        .collect();
    Stmt { sql, params }
}

#[derive(Iden)]
enum Users {
    Table,
    Id,
    Login,
}

#[derive(Iden)]
enum Workspaces {
    Table,
    Id,
    Name,
}

#[derive(Iden)]
enum WorkspaceMembers {
    Table,
    WorkspaceId,
    UserId,
    Role,
}

#[derive(Iden)]
enum Tokens {
    Table,
    Id,
    Hash,
    UserId,
    WorkspaceId,
    Name,
    ExpiresAt,
    CreatedAt,
}

#[derive(Iden)]
enum Nodes {
    Table,
    WorkspaceId,
    Id,
    Kind,
    Title,
    Status,
    Due,
    Tags,
    DependsOn,
    Milestones,
    Body,
    Priority,
    Assignee,
    Prs,
    CreatedAt,
    UpdatedAt,
    CompletedAt,
}

#[derive(Iden)]
enum Changes {
    Table,
    WorkspaceId,
    Version,
    UserId,
    TokenId,
    TokenName,
    Agent,
    IdempotencyKey,
    Ops,
    Created,
    CreatedAt,
}

fn now() -> Expr {
    Func::cust("unixepoch").into()
}

fn role_text(role: Role) -> &'static str {
    match role {
        Role::Owner => "owner",
        Role::Editor => "editor",
        Role::Viewer => "viewer",
    }
}

/// Rechecks the token and current membership inside the transaction that uses it.
pub fn authorized(workspace_id: &str, user_id: u64, token_id: &str, roles: &[Role]) -> SimpleExpr {
    let member = Query::select()
        .expr(Expr::val(1i64))
        .from(WorkspaceMembers::Table)
        .and_where(Expr::col(WorkspaceMembers::WorkspaceId).eq(workspace_id))
        .and_where(Expr::col(WorkspaceMembers::UserId).eq(user_id as i64))
        .and_where(Expr::col(WorkspaceMembers::Role).is_in(roles.iter().copied().map(role_text)))
        .to_owned();
    let mut token = token_query(user_id, token_id);
    token.and_where(Expr::col(Tokens::WorkspaceId).is_null().or(Expr::col(Tokens::WorkspaceId).eq(workspace_id)));
    Expr::exists(member).and(Expr::exists(token))
}

fn token_query(user_id: u64, token_id: &str) -> sea_query::SelectStatement {
    Query::select()
        .expr(Expr::val(1i64))
        .from(Tokens::Table)
        .and_where(Expr::col(Tokens::Id).eq(token_id))
        .and_where(Expr::col(Tokens::UserId).eq(user_id as i64))
        .and_where(Expr::col(Tokens::ExpiresAt).is_null().or(Expr::col(Tokens::ExpiresAt).gt(now())))
        .to_owned()
}

fn authenticated(user_id: u64, token_id: &str, unscoped: bool) -> SimpleExpr {
    let mut token = token_query(user_id, token_id);
    if unscoped {
        token.and_where(Expr::col(Tokens::WorkspaceId).is_null());
    }
    Expr::exists(token)
}

pub fn session(user_id: u64, token_id: &str) -> Stmt {
    stmt(Query::select().expr(Expr::val(1i64)).and_where(authenticated(user_id, token_id, false)))
}

pub fn membership(workspace_id: &str, user_id: u64, token_id: &str) -> Stmt {
    let mut query = workspace_query(user_id, Some(workspace_id));
    query.and_where(authorized(workspace_id, user_id, token_id, &[Role::Owner, Role::Editor, Role::Viewer]));
    stmt(&query)
}

// ---- users and tokens ------------------------------------------------------

pub fn upsert_user(id: u64, login: &str) -> Stmt {
    stmt(
        Query::insert()
            .into_table(Users::Table)
            .columns([Users::Id, Users::Login])
            .values_panic([(id as i64).into(), login.into()])
            .on_conflict(OnConflict::column(Users::Id).update_column(Users::Login).to_owned()),
    )
}

/// The unexpired token with this hash and its user, as `auth::Caller`.
pub fn caller(hash: &str) -> Stmt {
    stmt(
        Query::select()
            .expr_as(Expr::col((Tokens::Table, Tokens::Id)), "token_id")
            .expr_as(Expr::col((Tokens::Table, Tokens::Name)), "token_name")
            .expr_as(Expr::col((Tokens::Table, Tokens::WorkspaceId)), "scope")
            .expr_as(Expr::col((Users::Table, Users::Id)), "user_id")
            .column((Users::Table, Users::Login))
            .from(Tokens::Table)
            .join(
                JoinType::InnerJoin,
                Users::Table,
                Expr::col((Users::Table, Users::Id)).equals((Tokens::Table, Tokens::UserId)),
            )
            .and_where(Expr::col(Tokens::Hash).eq(hash))
            .and_where(Expr::col(Tokens::ExpiresAt).is_null().or(Expr::col(Tokens::ExpiresAt).gt(now()))),
    )
}

const TOKEN_COLUMNS: [Tokens; 5] =
    [Tokens::Id, Tokens::Name, Tokens::WorkspaceId, Tokens::ExpiresAt, Tokens::CreatedAt];

/// Inserts a token and returns it as `wire::Token`.
pub fn insert_token(
    id: &str,
    hash: &str,
    user_id: u64,
    workspace_id: Option<&str>,
    name: &str,
    expires_in: Option<u64>,
) -> Stmt {
    token_insert(id, hash, Expr::val(user_id as i64), workspace_id, name, expires_in)
}

pub fn insert_token_authorized(
    id: &str,
    hash: &str,
    user_id: u64,
    workspace_id: Option<&str>,
    name: &str,
    expires_in: Option<u64>,
    token_id: &str,
) -> Stmt {
    let mut allowed = authenticated(user_id, token_id, true);
    if let Some(wid) = workspace_id {
        allowed = allowed.and(authorized(wid, user_id, token_id, &[Role::Owner, Role::Editor, Role::Viewer]));
    }
    let user_id = Expr::case(allowed, user_id as i64).finally(None::<i64>).into();
    token_insert(id, hash, user_id, workspace_id, name, expires_in)
}

fn token_insert(
    id: &str,
    hash: &str,
    user_id: SimpleExpr,
    workspace_id: Option<&str>,
    name: &str,
    expires_in: Option<u64>,
) -> Stmt {
    let expires_at = match expires_in {
        Some(seconds) => now().add(seconds as i64),
        None => Expr::val(None::<i64>),
    };
    stmt(
        Query::insert()
            .into_table(Tokens::Table)
            .columns([Tokens::Id, Tokens::Hash, Tokens::UserId, Tokens::WorkspaceId, Tokens::Name, Tokens::ExpiresAt])
            .values_panic([
                id.into(),
                hash.into(),
                user_id,
                workspace_id.map(str::to_owned).into(),
                name.into(),
                expires_at,
            ])
            .returning(Query::returning().columns(TOKEN_COLUMNS)),
    )
}

/// The user's tokens as `wire::Token`.
pub fn tokens(user_id: u64) -> Stmt {
    stmt(
        Query::select()
            .columns(TOKEN_COLUMNS)
            .from(Tokens::Table)
            .and_where(Expr::col(Tokens::UserId).eq(user_id as i64))
            .order_by(Tokens::CreatedAt, Order::Asc)
            .order_by(Tokens::Id, Order::Asc),
    )
}

/// Deletes the user's token and returns its id if it existed.
pub fn delete_token(id: &str, user_id: u64) -> Stmt {
    token_delete(id, user_id, None)
}

pub fn delete_token_authorized(id: &str, user_id: u64, token_id: &str) -> Stmt {
    token_delete(id, user_id, Some(authenticated(user_id, token_id, id != token_id)))
}

fn token_delete(id: &str, user_id: u64, allowed: Option<SimpleExpr>) -> Stmt {
    stmt(
        Query::delete()
            .from_table(Tokens::Table)
            .and_where(Expr::col(Tokens::Id).eq(id))
            .and_where(Expr::col(Tokens::UserId).eq(user_id as i64))
            .and_where_option(allowed)
            .returning_col(Tokens::Id),
    )
}

// ---- workspaces and members ------------------------------------------------

pub fn insert_workspace(id: &str, name: &str) -> Stmt {
    workspace_insert(id, Expr::val(name))
}

pub fn insert_workspace_authorized(id: &str, name: &str, user_id: u64, token_id: &str) -> Stmt {
    let name = Expr::case(authenticated(user_id, token_id, true), name).finally(None::<String>).into();
    workspace_insert(id, name)
}

fn workspace_insert(id: &str, name: SimpleExpr) -> Stmt {
    stmt(
        Query::insert()
            .into_table(Workspaces::Table)
            .columns([Workspaces::Id, Workspaces::Name])
            .values_panic([id.into(), name]),
    )
}

pub fn rename_workspace(id: &str, name: &str, user_id: u64, token_id: &str) -> Stmt {
    stmt(
        Query::update()
            .table(Workspaces::Table)
            .value(Workspaces::Name, name)
            .and_where(Expr::col(Workspaces::Id).eq(id))
            .and_where(authorized(id, user_id, token_id, &[Role::Owner]))
            .returning_col(Workspaces::Id),
    )
}

pub fn delete_workspace(id: &str, user_id: u64, token_id: &str) -> Stmt {
    stmt(
        Query::delete()
            .from_table(Workspaces::Table)
            .and_where(Expr::col(Workspaces::Id).eq(id))
            .and_where(authorized(id, user_id, token_id, &[Role::Owner]))
            .returning_col(Workspaces::Id),
    )
}

/// The workspaces the user belongs to, as `wire::WorkspaceInfo`: all of them, or only the given one.
pub fn workspaces(user_id: u64, only: Option<&str>) -> Stmt {
    stmt(&workspace_query(user_id, only))
}

fn workspace_query(user_id: u64, only: Option<&str>) -> sea_query::SelectStatement {
    Query::select()
        .column((Workspaces::Table, Workspaces::Id))
        .column((Workspaces::Table, Workspaces::Name))
        .column((WorkspaceMembers::Table, WorkspaceMembers::Role))
        .from(WorkspaceMembers::Table)
        .join(
            JoinType::InnerJoin,
            Workspaces::Table,
            Expr::col((Workspaces::Table, Workspaces::Id))
                .equals((WorkspaceMembers::Table, WorkspaceMembers::WorkspaceId)),
        )
        .and_where(Expr::col((WorkspaceMembers::Table, WorkspaceMembers::UserId)).eq(user_id as i64))
        .and_where_option(only.map(|id| Expr::col((Workspaces::Table, Workspaces::Id)).eq(id)))
        .order_by((Workspaces::Table, Workspaces::Name), Order::Asc)
        .order_by((Workspaces::Table, Workspaces::Id), Order::Asc)
        .to_owned()
}

/// The members of a workspace as `wire::Member`, with their `user_id`.
pub fn members(workspace_id: &str) -> Stmt {
    stmt(
        Query::select()
            .column((Users::Table, Users::Login))
            .column((WorkspaceMembers::Table, WorkspaceMembers::Role))
            .column((WorkspaceMembers::Table, WorkspaceMembers::UserId))
            .from(WorkspaceMembers::Table)
            .join(
                JoinType::InnerJoin,
                Users::Table,
                Expr::col((Users::Table, Users::Id)).equals((WorkspaceMembers::Table, WorkspaceMembers::UserId)),
            )
            .and_where(Expr::col((WorkspaceMembers::Table, WorkspaceMembers::WorkspaceId)).eq(workspace_id))
            .order_by((Users::Table, Users::Login), Order::Asc),
    )
}

pub fn upsert_member(workspace_id: &str, user_id: u64, role: Role) -> Stmt {
    member_upsert(workspace_id, user_id, Expr::val(role_text(role)))
}

pub fn upsert_member_authorized(workspace_id: &str, target: u64, role: Role, user_id: u64, token_id: &str) -> Stmt {
    let mut allowed = authorized(workspace_id, user_id, token_id, &[Role::Owner]);
    if role != Role::Owner {
        allowed = allowed.and(owner_survives(workspace_id, target));
    }
    // A denied write violates NOT NULL, rolling back the entire batch, including
    // its user update. The condition is evaluated under the database write lock.
    let role = Expr::case(allowed, role_text(role)).finally(None::<String>).into();
    member_upsert(workspace_id, target, role)
}

fn member_upsert(workspace_id: &str, user_id: u64, role: SimpleExpr) -> Stmt {
    stmt(
        Query::insert()
            .into_table(WorkspaceMembers::Table)
            .columns([WorkspaceMembers::WorkspaceId, WorkspaceMembers::UserId, WorkspaceMembers::Role])
            .values_panic([workspace_id.into(), (user_id as i64).into(), role])
            .on_conflict(
                OnConflict::columns([WorkspaceMembers::WorkspaceId, WorkspaceMembers::UserId])
                    .update_column(WorkspaceMembers::Role)
                    .to_owned(),
            ),
    )
}

fn owner_survives(workspace_id: &str, target: u64) -> SimpleExpr {
    let owner = || {
        Query::select()
            .expr(Expr::val(1i64))
            .from(WorkspaceMembers::Table)
            .and_where(Expr::col(WorkspaceMembers::WorkspaceId).eq(workspace_id))
            .and_where(Expr::col(WorkspaceMembers::Role).eq("owner"))
            .to_owned()
    };
    let mut target_owner = owner();
    target_owner.and_where(Expr::col(WorkspaceMembers::UserId).eq(target as i64));
    let mut other_owner = owner();
    other_owner.and_where(Expr::col(WorkspaceMembers::UserId).ne(target as i64));
    Expr::exists(target_owner).not().or(Expr::exists(other_owner))
}

pub fn delete_member_authorized(workspace_id: &str, target: u64, user_id: u64, token_id: &str) -> Stmt {
    stmt(
        Query::delete()
            .from_table(WorkspaceMembers::Table)
            .and_where(Expr::col(WorkspaceMembers::WorkspaceId).eq(workspace_id))
            .and_where(Expr::col(WorkspaceMembers::UserId).eq(target as i64))
            .and_where(authorized(workspace_id, user_id, token_id, &[Role::Owner]))
            .and_where(owner_survives(workspace_id, target))
            .returning_col(WorkspaceMembers::UserId),
    )
}

pub fn delete_member(workspace_id: &str, user_id: u64) -> Stmt {
    stmt(
        Query::delete()
            .from_table(WorkspaceMembers::Table)
            .and_where(Expr::col(WorkspaceMembers::WorkspaceId).eq(workspace_id))
            .and_where(Expr::col(WorkspaceMembers::UserId).eq(user_id as i64)),
    )
}

// ---- nodes and changes -----------------------------------------------------

/// One row `{"now": n}`: the time of the database, in Unix seconds. It is the
/// clock of the server, which has none of its own on Workers.
pub fn clock() -> Stmt {
    stmt(Query::select().expr_as(now(), "now"))
}

/// One row `{"version": n}`: the number of writes the workspace has received.
pub fn version(workspace_id: &str) -> Stmt {
    stmt(
        Query::select()
            .expr_as(Func::coalesce([Func::max(Expr::col(Changes::Version)).into(), Expr::val(0i64)]), "version")
            .from(Changes::Table)
            .and_where(Expr::col(Changes::WorkspaceId).eq(workspace_id)),
    )
}

const NODE_COLUMNS: [Nodes; 15] = [
    Nodes::Id,
    Nodes::Kind,
    Nodes::Title,
    Nodes::Status,
    Nodes::Due,
    Nodes::Tags,
    Nodes::DependsOn,
    Nodes::Milestones,
    Nodes::Body,
    Nodes::Priority,
    Nodes::Assignee,
    Nodes::Prs,
    Nodes::CreatedAt,
    Nodes::UpdatedAt,
    Nodes::CompletedAt,
];

/// The nodes of a workspace, to read with [`node`].
pub fn nodes(workspace_id: &str) -> Stmt {
    stmt(
        Query::select()
            .columns(NODE_COLUMNS)
            .from(Nodes::Table)
            .and_where(Expr::col(Nodes::WorkspaceId).eq(workspace_id)),
    )
}

/// A row of [`nodes`]. The four lists are stored as JSON text.
pub fn node(row: serde_json::Value) -> Result<Node, serde_json::Error> {
    #[derive(Deserialize)]
    struct Row {
        id: NodeId,
        kind: Kind,
        title: String,
        status: Status,
        due: Option<jiff::civil::Date>,
        tags: String,
        depends_on: String,
        milestones: String,
        body: String,
        priority: Option<Priority>,
        assignee: Option<String>,
        prs: String,
        created_at: Option<jiff::Timestamp>,
        updated_at: Option<jiff::Timestamp>,
        completed_at: Option<jiff::Timestamp>,
    }
    let row: Row = serde_json::from_value(row)?;
    Ok(Node {
        id: row.id,
        kind: row.kind,
        title: row.title,
        status: row.status,
        due: row.due,
        tags: serde_json::from_str(&row.tags)?,
        priority: row.priority,
        assignee: row.assignee,
        prs: serde_json::from_str(&row.prs)?,
        created_at: row.created_at,
        updated_at: row.updated_at,
        completed_at: row.completed_at,
        depends_on: serde_json::from_str(&row.depends_on)?,
        milestones: serde_json::from_str(&row.milestones)?,
        body: row.body,
    })
}

/// Inserts or replaces the given nodes with one statement, whatever their number.
pub fn upsert_nodes(workspace_id: &str, nodes: &[&Node]) -> Stmt {
    let rows: Vec<serde_json::Value> = nodes
        .iter()
        .map(|n| {
            json!({
                "id": n.id, "kind": n.kind, "title": n.title, "status": n.status, "due": n.due,
                "tags": n.tags, "depends_on": n.depends_on, "milestones": n.milestones, "body": n.body,
                "priority": n.priority, "assignee": n.assignee, "prs": n.prs,
                "created_at": n.created_at, "updated_at": n.updated_at, "completed_at": n.completed_at,
            })
        })
        .collect();
    // `value` is the column of `json_each` that holds one array element.
    let field = |column: Nodes| Expr::col("value").cast_json_field(column.to_string());
    let mut select = Query::select();
    select.expr(Expr::val(workspace_id));
    for column in NODE_COLUMNS {
        select.expr(field(column));
    }
    select
        .from_function(Func::cust("json_each").arg(serde_json::Value::Array(rows).to_string()), "rows")
        // SQLite needs a WHERE clause here to tell `ON CONFLICT` from a join constraint.
        .and_where(Expr::val(true));
    stmt(
        Query::insert()
            .into_table(Nodes::Table)
            .columns([Nodes::WorkspaceId].into_iter().chain(NODE_COLUMNS))
            .select_from(select)
            .expect("the select has one expression per column")
            .on_conflict(
                OnConflict::columns([Nodes::WorkspaceId, Nodes::Id])
                    .update_columns(NODE_COLUMNS.into_iter().skip(1))
                    .to_owned(),
            ),
    )
}

pub fn delete_nodes(workspace_id: &str, ids: &[&NodeId]) -> Stmt {
    let ids = Query::select()
        .column("value")
        .from_function(Func::cust("json_each").arg(json!(ids).to_string()), "ids")
        .to_owned();
    stmt(
        Query::delete()
            .from_table(Nodes::Table)
            .and_where(Expr::col(Nodes::WorkspaceId).eq(workspace_id))
            .and_where(Expr::col(Nodes::Id).in_subquery(ids)),
    )
}

/// The write recorded under this idempotency key, as `{"version", "created"}`.
pub fn change_by_key(workspace_id: &str, idempotency_key: &str) -> Stmt {
    stmt(
        Query::select()
            .columns([Changes::Version, Changes::Created])
            .from(Changes::Table)
            .and_where(Expr::col(Changes::WorkspaceId).eq(workspace_id))
            .and_where(Expr::col(Changes::IdempotencyKey).eq(idempotency_key)),
    )
}

pub struct NewChange<'a> {
    pub workspace_id: &'a str,
    pub version: u64,
    pub user_id: u64,
    pub token_id: &'a str,
    pub token_name: &'a str,
    pub agent: Option<&'a str>,
    pub idempotency_key: &'a str,
    /// JSON array of operations.
    pub ops: String,
    /// JSON object of created ids.
    pub created: String,
}

/// Records a write. The key `(workspace_id, version)` makes it fail when
/// another write already took this version.
pub fn insert_change(change: NewChange) -> Stmt {
    let user_id = Expr::val(change.user_id as i64);
    change_insert(change, user_id)
}

pub fn insert_change_authorized(change: NewChange) -> Stmt {
    let user_id = Expr::case(
        authorized(change.workspace_id, change.user_id, change.token_id, &[Role::Owner, Role::Editor]),
        change.user_id as i64,
    )
    .finally(None::<i64>)
    .into();
    change_insert(change, user_id)
}

fn change_insert(change: NewChange, user_id: SimpleExpr) -> Stmt {
    stmt(
        Query::insert()
            .into_table(Changes::Table)
            .columns([
                Changes::WorkspaceId,
                Changes::Version,
                Changes::UserId,
                Changes::TokenId,
                Changes::TokenName,
                Changes::Agent,
                Changes::IdempotencyKey,
                Changes::Ops,
                Changes::Created,
            ])
            .values_panic([
                change.workspace_id.into(),
                (change.version as i64).into(),
                user_id,
                change.token_id.into(),
                change.token_name.into(),
                change.agent.map(str::to_owned).into(),
                change.idempotency_key.into(),
                change.ops.into(),
                change.created.into(),
            ]),
    )
}

/// The writes after a version, oldest first: `wire::Change` with `ops` as JSON text.
pub fn changes(workspace_id: &str, after: u64, limit: u64) -> Stmt {
    stmt(
        Query::select()
            .column((Changes::Table, Changes::Version))
            .expr_as(Expr::col((Users::Table, Users::Login)), "user")
            .expr_as(Expr::col((Changes::Table, Changes::TokenName)), "token")
            .column((Changes::Table, Changes::Agent))
            .column((Changes::Table, Changes::CreatedAt))
            .column((Changes::Table, Changes::Ops))
            .from(Changes::Table)
            .join(
                JoinType::InnerJoin,
                Users::Table,
                Expr::col((Users::Table, Users::Id)).equals((Changes::Table, Changes::UserId)),
            )
            .and_where(Expr::col((Changes::Table, Changes::WorkspaceId)).eq(workspace_id))
            .and_where(Expr::col((Changes::Table, Changes::Version)).gt(after as i64))
            .order_by((Changes::Table, Changes::Version), Order::Asc)
            .limit(limit),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;
    use crate::sqlite::SqliteDb;

    fn run(db: &SqliteDb, statements: Vec<Stmt>) -> Vec<crate::db::Rows> {
        futures::executor::block_on(db.batch(statements)).unwrap()
    }

    #[test]
    fn nodes_round_trip_through_one_upsert_and_one_delete() {
        let db = SqliteDb::new();
        run(&db, vec![insert_workspace("w", "W")]);
        let mut a = Node::new(NodeId("a".into()), Kind::Milestone, "it's \"a\"; DROP TABLE nodes".into());
        a.due = Some(jiff::civil::date(2026, 10, 31));
        a.tags = vec!["x".into(), "y".into()];
        a.priority = Some(Priority::Urgent);
        a.assignee = Some("it's me".into());
        a.prs = vec!["https://github.com/o/r/pull/1".into(), "https://example.com/pr/2".into()];
        a.created_at = Some("2026-10-01T00:00:00Z".parse().unwrap());
        a.updated_at = Some("2026-10-02T03:04:05Z".parse().unwrap());
        let mut b = Node::new(NodeId("b".into()), Kind::Task, "b".into());
        b.depends_on = vec![a.id.clone()];
        b.milestones = vec![a.id.clone()];
        b.body = "notes\n".into();
        let load = |db: &SqliteDb| -> Vec<Node> {
            run(db, vec![nodes("w")]).remove(0).0.into_iter().map(|row| node(row).unwrap()).collect()
        };

        let upsert = upsert_nodes("w", &[&a, &b]);
        assert_eq!(
            upsert.sql(),
            r#"INSERT INTO "nodes" ("workspace_id", "id", "kind", "title", "status", "due", "tags", "depends_on", "milestones", "body", "priority", "assignee", "prs", "created_at", "updated_at", "completed_at") SELECT ?, "value" ->> ?, "value" ->> ?, "value" ->> ?, "value" ->> ?, "value" ->> ?, "value" ->> ?, "value" ->> ?, "value" ->> ?, "value" ->> ?, "value" ->> ?, "value" ->> ?, "value" ->> ?, "value" ->> ?, "value" ->> ?, "value" ->> ? FROM json_each(?) AS "rows" WHERE ? ON CONFLICT ("workspace_id", "id") DO UPDATE SET "kind" = "excluded"."kind", "title" = "excluded"."title", "status" = "excluded"."status", "due" = "excluded"."due", "tags" = "excluded"."tags", "depends_on" = "excluded"."depends_on", "milestones" = "excluded"."milestones", "body" = "excluded"."body", "priority" = "excluded"."priority", "assignee" = "excluded"."assignee", "prs" = "excluded"."prs", "created_at" = "excluded"."created_at", "updated_at" = "excluded"."updated_at", "completed_at" = "excluded"."completed_at""#
        );
        run(&db, vec![upsert]);
        assert_eq!(load(&db), [a.clone(), b.clone()]);

        b.status = Status::Done;
        b.completed_at = a.updated_at;
        b.milestones.clear();
        run(&db, vec![upsert_nodes("w", &[&b]), delete_nodes("w", &[&a.id])]);
        assert_eq!(load(&db), [b]);
        run(&db, vec![upsert_nodes("w", &[]), delete_nodes("w", &[])]);
    }

    #[test]
    fn a_version_can_be_taken_once() {
        let db = SqliteDb::new();
        run(&db, vec![upsert_user(1, "u"), insert_workspace("w", "W")]);
        let change = |version, key| {
            insert_change(NewChange {
                workspace_id: "w",
                version,
                user_id: 1,
                token_id: "t",
                token_name: "cli",
                agent: None,
                idempotency_key: key,
                ops: "[]".into(),
                created: "{}".into(),
            })
        };
        let version = |db: &SqliteDb| run(db, vec![version("w")]).remove(0).0[0]["version"].as_u64().unwrap();
        assert_eq!(version(&db), 0);
        run(&db, vec![change(1, "k1")]);
        let again = |c| futures::executor::block_on(db.batch(vec![c, insert_workspace("x", "X")]));
        assert!(matches!(again(change(1, "k2")), Err(crate::db::DbError::Constraint)));
        assert!(matches!(again(change(2, "k1")), Err(crate::db::DbError::Constraint)));
        // The failed batches left nothing behind.
        assert_eq!(version(&db), 1);
        run(&db, vec![insert_workspace("x", "X")]);
    }
}

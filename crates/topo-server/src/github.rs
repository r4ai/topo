use crate::db::BoxFuture;

#[derive(Debug, Clone, PartialEq)]
pub struct GitHubUser {
    pub id: u64,
    pub login: String,
}

/// The two questions the server asks GitHub. `Err` is a failure to get an answer.
pub trait GitHub: Send + Sync {
    /// The user an access token belongs to, if this server's OAuth app issued it.
    fn check_token<'a>(&'a self, access_token: &'a str) -> BoxFuture<'a, Result<Option<GitHubUser>, String>>;
    /// The user with this login, if there is one.
    fn user_by_login<'a>(&'a self, login: &'a str) -> BoxFuture<'a, Result<Option<GitHubUser>, String>>;
}

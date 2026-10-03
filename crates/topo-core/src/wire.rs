//! Bodies of the cloud API's requests and responses, shared by server and clients.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::model::{Node, NodeId};
use crate::ops::Op;

/// A node with its notes, which the frontmatter form of [`Node`] leaves out.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct WireNode {
    #[serde(flatten)]
    node: Node,
    /// Markdown notes.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    body: String,
}

impl From<Node> for WireNode {
    fn from(mut node: Node) -> Self {
        let body = std::mem::take(&mut node.body);
        Self { node, body }
    }
}

impl From<WireNode> for Node {
    fn from(wire: WireNode) -> Self {
        Node { body: wire.body, ..wire.node }
    }
}

/// Every node of a workspace at one version.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct Snapshot {
    /// Number of writes the workspace has received.
    pub version: u64,
    pub nodes: Vec<WireNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
pub struct ApplyRequest {
    pub ops: Vec<Op>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
pub struct ImportRequest {
    pub nodes: Vec<WireNode>,
}

/// Outcome of a write.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct ApplyResult {
    /// The workspace version this write produced.
    pub version: u64,
    /// Id of the node each `ref` of the batch created.
    pub created: BTreeMap<String, NodeId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct ErrorBody {
    pub error: ErrorDetail,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct ErrorDetail {
    /// Stable identifier of the failure, such as `not_found`.
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct AuthConfig {
    /// Client id of the GitHub OAuth app that the device flow signs in to.
    pub github_client_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
pub struct GithubSignIn {
    /// Access token the GitHub device flow issued for this server's OAuth app.
    pub access_token: String,
    /// Name of the token to create, such as the host name.
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct User {
    /// GitHub user id.
    pub id: u64,
    /// GitHub login.
    pub login: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
pub struct NewToken {
    pub name: String,
    /// Restricts the token to one workspace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
    /// Lifetime in seconds. A token without it does not expire.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_in: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct Token {
    pub id: String,
    pub name: String,
    pub workspace_id: Option<String>,
    /// Unix time in seconds.
    pub expires_at: Option<u64>,
    /// Unix time in seconds.
    pub created_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct CreatedToken {
    #[serde(flatten)]
    pub info: Token,
    /// The secret. It is shown only in this response.
    pub token: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// May also rename and delete the workspace and manage its members.
    Owner,
    /// May read and write nodes.
    Editor,
    /// May only read.
    Viewer,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct WorkspaceInfo {
    pub id: String,
    pub name: String,
    /// The caller's role.
    pub role: Role,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
pub struct WorkspaceName {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct Member {
    /// Stable GitHub user id. Older servers may omit it.
    #[serde(default)]
    pub user_id: u64,
    /// GitHub login.
    pub login: String,
    pub role: Role,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
pub struct MemberRole {
    pub role: Role,
}

/// One recorded write.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct Change {
    pub version: u64,
    /// GitHub login of the user whose token made the write.
    pub user: String,
    /// Name of the token that made the write.
    pub token: String,
    /// The `Topo-Agent` header of the request.
    pub agent: Option<String>,
    /// Unix time in seconds.
    pub created_at: u64,
    /// The operations as applied: references are ids.
    pub ops: Vec<Op>,
}

use std::fmt;
use std::str::FromStr;

use jiff::Timestamp;
use jiff::civil::Date;
use serde::{Deserialize, Serialize};

use crate::Error;

/// Short random identifier of a node. It is also the node's file stem.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(transparent)]
pub struct NodeId(pub String);

impl NodeId {
    const ALPHABET: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    const LEN: usize = 6;

    pub fn random() -> Self {
        let seed = getrandom::u64().expect("the host provides system randomness");
        let mut rng = fastrand::Rng::with_seed(seed);
        let id = (0..Self::LEN).map(|_| Self::ALPHABET[rng.usize(..Self::ALPHABET.len())] as char).collect();
        Self(id)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Checks an id that arrived from another machine. Such an id becomes a
    /// file name on export, so it must not be able to name a path.
    pub fn validate(&self) -> Result<(), Error> {
        let valid = (1..=32).contains(&self.0.len()) && self.0.bytes().all(|b| Self::ALPHABET.contains(&b));
        if valid { Ok(()) } else { Err(Error::InvalidId(self.0.clone())) }
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    #[default]
    Task,
    Milestone,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "lowercase")]
pub enum Status {
    #[default]
    Todo,
    Doing,
    Done,
    Dropped,
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.serialize(f)
    }
}

impl Status {
    /// A closed node no longer blocks the nodes depending on it.
    pub fn is_closed(self) -> bool {
        matches!(self, Status::Done | Status::Dropped)
    }
}

/// How much a node matters next to the others. It never changes what is ready:
/// a blocked node stays blocked whatever its priority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    Low,
    Medium,
    High,
    Urgent,
}

impl Priority {
    pub const ALL: [Priority; 4] = [Priority::Low, Priority::Medium, Priority::High, Priority::Urgent];
}

impl fmt::Display for Priority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.serialize(f)
    }
}

impl FromStr for Priority {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Error> {
        Self::ALL
            .into_iter()
            .find(|p| p.to_string() == s.trim().to_lowercase())
            .ok_or_else(|| Error::InvalidPriority(s.to_owned()))
    }
}

/// The canonical form of an assignee: a label of a person or an agent, such as
/// a GitHub login or a `TOPO_AGENT` value. It identifies and is what is shown.
pub fn normalize_assignee(input: &str) -> Result<String, Error> {
    let name = input.trim();
    let valid = (1..=64).contains(&name.chars().count()) && !name.chars().any(char::is_control);
    if valid { Ok(name.to_owned()) } else { Err(Error::InvalidAssignee(input.to_owned())) }
}

/// The canonical URL of a pull request reference.
///
/// Accepts an `http(s)` URL or the GitHub shorthand `owner/repo#123`. A GitHub
/// pull request URL loses everything after its number (`/files`, the query, the
/// fragment), so two links to one pull request are equal. Other URLs lose
/// their fragment and trailing slashes.
pub fn normalize_pr(input: &str) -> Result<String, Error> {
    let invalid = || Error::InvalidPr(input.to_owned());
    let text = input.trim();
    if text.is_empty() || text.len() > 2048 || text.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(invalid());
    }
    let is_name = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c));
    let is_number = |s: &str| !s.is_empty() && s.len() <= 10 && s.bytes().all(|b| b.is_ascii_digit());
    if let Some((repo, number)) = text.split_once('#')
        && let Some((owner, name)) = repo.split_once('/')
        && is_name(owner)
        && is_name(name)
        && is_number(number)
    {
        return Ok(format!("https://github.com/{owner}/{name}/pull/{number}"));
    }
    let (scheme, rest) = text.split_once("://").ok_or_else(invalid)?;
    let scheme = scheme.to_ascii_lowercase();
    if scheme != "https" && scheme != "http" {
        return Err(invalid());
    }
    let rest = rest.split('#').next().expect("split yields at least one item");
    let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
    let host = host.to_ascii_lowercase();
    if host.is_empty() || host.contains(['@', '?']) {
        return Err(invalid());
    }
    if host == "github.com" || host == "www.github.com" {
        let path = path.split('?').next().expect("split yields at least one item");
        if let [owner, name, "pull", number, ..] = path.split('/').collect::<Vec<_>>()[..]
            && is_name(owner)
            && is_name(name)
            && is_number(number)
        {
            return Ok(format!("https://github.com/{owner}/{name}/pull/{number}"));
        }
    }
    let path = path.trim_end_matches('/');
    Ok(if path.is_empty() { format!("{scheme}://{host}") } else { format!("{scheme}://{host}/{path}") })
}

/// A short label for a pull request URL: `owner/repo#123` on GitHub, else the URL without its scheme.
pub fn pr_label(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    match rest.split('/').collect::<Vec<_>>()[..] {
        ["github.com", owner, name, "pull", number] => format!("{owner}/{name}#{number}"),
        _ => rest.to_owned(),
    }
}

/// A task or milestone. `depends_on` orders work; a task's `milestones` puts it
/// in the set of work each of those milestones consists of.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct Node {
    pub id: NodeId,
    #[serde(default)]
    pub kind: Kind,
    pub title: String,
    #[serde(default)]
    pub status: Status,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due: Option<Date>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<Priority>,
    /// Who works on the node, in the form of [`normalize_assignee`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assignee: Option<String>,
    /// Pull requests related to the node, each in the form of [`normalize_pr`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prs: Vec<String>,
    /// When the node was created, in UTC. Absent on nodes older than this field;
    /// it is never guessed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "openapi", schema(value_type = Option<String>, format = DateTime))]
    pub created_at: Option<Timestamp>,
    /// When the node last changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "openapi", schema(value_type = Option<String>, format = DateTime))]
    pub updated_at: Option<Timestamp>,
    /// When the node became `done`. Present only while it is `done`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "openapi", schema(value_type = Option<String>, format = DateTime))]
    pub completed_at: Option<Timestamp>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends_on: Vec<NodeId>,
    /// Milestones this task belongs to (tasks only).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub milestones: Vec<NodeId>,
    /// Free-form Markdown notes (the file body, not part of the frontmatter).
    #[serde(skip)]
    pub body: String,
}

impl Node {
    pub fn new(id: NodeId, kind: Kind, title: String) -> Self {
        Self {
            id,
            kind,
            title,
            status: Status::Todo,
            due: None,
            tags: Vec::new(),
            priority: None,
            assignee: None,
            prs: Vec::new(),
            created_at: None,
            updated_at: None,
            completed_at: None,
            depends_on: Vec::new(),
            milestones: Vec::new(),
            body: String::new(),
        }
    }

    /// Whether the two nodes are equal apart from their timestamps.
    pub fn same_content(&self, other: &Node) -> bool {
        let timeless = |n: &Node| Node { created_at: None, updated_at: None, completed_at: None, ..n.clone() };
        timeless(self) == timeless(other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pr_references_normalize_to_one_url_per_pull_request() {
        let pr = "https://github.com/r4ai/topo/pull/12";
        for input in [
            "r4ai/topo#12",
            " https://github.com/r4ai/topo/pull/12 ",
            "https://GitHub.com/r4ai/topo/pull/12/files?diff=split#r1",
            "http://www.github.com/r4ai/topo/pull/12/",
        ] {
            assert_eq!(normalize_pr(input).unwrap(), pr, "{input}");
        }
        assert_eq!(
            normalize_pr("https://gitlab.com/a/b/-/merge_requests/3/#x").unwrap(),
            "https://gitlab.com/a/b/-/merge_requests/3"
        );
        for input in ["", "12", "ftp://github.com/a/b/pull/1", "https://", "https://a b/c", "javascript:alert(1)"] {
            assert!(normalize_pr(input).is_err(), "{input}");
        }
        assert_eq!(pr_label(pr), "r4ai/topo#12");
        assert_eq!(pr_label("https://gitlab.com/a/b"), "gitlab.com/a/b");
    }

    #[test]
    fn assignees_and_priorities_are_validated() {
        assert_eq!(normalize_assignee(" claude ").unwrap(), "claude");
        assert!(normalize_assignee("  ").is_err());
        assert!(normalize_assignee(&"x".repeat(65)).is_err());
        assert!(normalize_assignee("a\nb").is_err());
        assert_eq!("High".parse::<Priority>().unwrap(), Priority::High);
        assert!("p0".parse::<Priority>().is_err());
        assert!(Priority::Urgent > Priority::High && Priority::Low < Priority::Medium);
    }
}

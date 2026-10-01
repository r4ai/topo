use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::Error;
use crate::graph::Graph;
use crate::model::{Node, NodeId};

/// Name of the directory that marks a workspace, like `.git`.
pub const DIR_NAME: &str = ".topo";
const NODES_DIR: &str = "nodes";

/// A `.topo` directory on disk and the graph loaded from it.
///
/// The Markdown files are the single source of truth. Mutate `graph`, then call
/// [`Workspace::save`], which writes exactly the files whose node changed.
pub struct Workspace {
    dir: PathBuf,
    pub graph: Graph,
    saved: BTreeMap<NodeId, Node>,
}

impl Workspace {
    /// Creates `<parent>/.topo` and returns the empty workspace.
    pub fn init(parent: &Path) -> Result<Self, Error> {
        let dir = parent.join(DIR_NAME);
        if dir.exists() {
            return Err(Error::AlreadyInitialized(dir));
        }
        fs::create_dir_all(dir.join(NODES_DIR)).map_err(|e| Error::io(&dir, e))?;
        Self::open(dir)
    }

    /// Opens the nearest `.topo` directory in `start` or its ancestors.
    pub fn discover(start: &Path) -> Result<Self, Error> {
        let dir = start
            .ancestors()
            .map(|a| a.join(DIR_NAME))
            .find(|d| d.is_dir())
            .ok_or_else(|| Error::NoWorkspace(start.to_owned()))?;
        Self::open(dir)
    }

    /// Opens a `.topo` directory.
    pub fn open(dir: PathBuf) -> Result<Self, Error> {
        let nodes_dir = dir.join(NODES_DIR);
        let entries = fs::read_dir(&nodes_dir).map_err(|e| Error::io(&nodes_dir, e))?;
        let mut nodes = Vec::new();
        for entry in entries {
            let path = entry.map_err(|e| Error::io(&nodes_dir, e))?.path();
            if path.extension().is_some_and(|ext| ext == "md") {
                let text = fs::read_to_string(&path).map_err(|e| Error::io(&path, e))?;
                nodes.push(parse(&text, &path)?);
            }
        }
        let graph = Graph::from_nodes(nodes)?;
        let saved = graph.clone().into_nodes();
        Ok(Self { dir, graph, saved })
    }

    /// The `.topo` directory.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The directory that holds one Markdown file per node.
    pub fn nodes_dir(&self) -> PathBuf {
        self.dir.join(NODES_DIR)
    }

    /// Writes changed and new nodes and deletes removed ones.
    pub fn save(&mut self) -> Result<(), Error> {
        let current = self.graph.clone().into_nodes();
        for node in current.values().filter(|n| self.saved.get(&n.id) != Some(n)) {
            let path = self.node_path(&node.id);
            let tmp = path.with_extension("md.tmp");
            fs::write(&tmp, render(node)).map_err(|e| Error::io(&tmp, e))?;
            fs::rename(&tmp, &path).map_err(|e| Error::io(&path, e))?;
        }
        for id in self.saved.keys().filter(|id| !current.contains_key(*id)) {
            let path = self.node_path(id);
            fs::remove_file(&path).map_err(|e| Error::io(&path, e))?;
        }
        self.saved = current;
        Ok(())
    }

    pub fn node_path(&self, id: &NodeId) -> PathBuf {
        self.nodes_dir().join(format!("{id}.md"))
    }
}

/// `---\n<yaml frontmatter>---\n\n<markdown body>`
pub fn render(node: &Node) -> String {
    let yaml = serde_yaml_ng::to_string(node).expect("a node always serializes to YAML");
    match node.body.is_empty() {
        true => format!("---\n{yaml}---\n"),
        false => format!("---\n{yaml}---\n\n{}", node.body),
    }
}

pub fn parse(text: &str, path: &Path) -> Result<Node, Error> {
    let invalid = |message: String| Error::InvalidFile { path: path.to_owned(), message };
    let rest = text.strip_prefix("---\n").ok_or_else(|| invalid("missing opening `---`".into()))?;
    let (end, _) = rest
        .match_indices("---")
        .find(|(i, _)| {
            (*i == 0 || rest[..*i].ends_with('\n')) && matches!(rest[i + 3..].chars().next(), None | Some('\n'))
        })
        .ok_or_else(|| invalid("missing closing `---`".into()))?;
    let mut node: Node = serde_yaml_ng::from_str(&rest[..end]).map_err(|e| invalid(e.to_string()))?;
    let after = &rest[end + 3..];
    let after = after.strip_prefix('\n').unwrap_or(after);
    node.body = after.strip_prefix('\n').unwrap_or(after).to_owned();
    if path.file_stem().and_then(|s| s.to_str()) != Some(node.id.as_str()) {
        return Err(invalid(format!("file name must be `{}.md`", node.id)));
    }
    Ok(node)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Kind, Status};

    #[test]
    fn render_and_parse_round_trip() {
        let mut node = Node::new(NodeId("abc123".into()), Kind::Task, "Ship: v1 --- final".into());
        node.status = Status::Doing;
        node.due = Some(jiff::civil::date(2026, 10, 31));
        node.tags = vec!["release".into()];
        node.depends_on = vec![NodeId("zzz999".into())];
        node.milestones = vec![NodeId("mmm000".into())];
        node.body = "notes\n---\nmore\n".into();
        let path = Path::new("abc123.md");
        assert_eq!(parse(&render(&node), path).unwrap(), node);

        node.body.clear();
        assert_eq!(parse(&render(&node), path).unwrap(), node);
    }

    #[test]
    fn parse_rejects_malformed_files() {
        let path = Path::new("x.md");
        assert!(parse("id: x\n", path).is_err());
        assert!(parse("---\nid: x\ntitle: t\n", path).is_err());
        assert!(parse("---\nid: y\ntitle: t\n---\n", path).is_err());
        assert!(parse("---\nid: x\ntitle: t\nstatus: finished\n---\n", path).is_err());
        assert!(parse("---\nid: x\ntitle: t\n---\n", path).is_ok());
    }

    #[test]
    fn save_writes_only_changes_and_reloads() {
        let tmp = tempfile::tempdir().unwrap();
        let mut ws = Workspace::init(tmp.path()).unwrap();
        let m = Node::new(ws.graph.fresh_id(), Kind::Milestone, "m".into());
        let m_id = m.id.clone();
        ws.graph.insert(m).unwrap();
        let mut a = Node::new(ws.graph.fresh_id(), Kind::Task, "a".into());
        a.milestones.push(m_id.clone());
        let a_id = a.id.clone();
        ws.graph.insert(a).unwrap();
        ws.save().unwrap();

        let mut reopened = Workspace::discover(&tmp.path().join(DIR_NAME).join(NODES_DIR)).unwrap();
        assert_eq!(reopened.graph.progress(&m_id), (0, 1));

        reopened.graph.remove(&m_id).unwrap();
        reopened.save().unwrap();
        assert!(!reopened.node_path(&m_id).exists());
        let again = Workspace::open(reopened.dir().to_owned()).unwrap();
        assert!(again.graph.get(&a_id).unwrap().milestones.is_empty());
        assert!(matches!(Workspace::init(tmp.path()), Err(Error::AlreadyInitialized(_))));
    }
}

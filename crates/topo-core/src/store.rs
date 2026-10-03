use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::Error;
use crate::graph::Graph;
use crate::model::{Node, NodeId};
use crate::ops::{self, Op};
use crate::wire::{ApplyResult, Snapshot};

/// Name of the directory that marks a workspace, like `.git`.
pub const DIR_NAME: &str = ".topo";
const NODES_DIR: &str = "nodes";

/// A server that holds the nodes of a workspace in place of the Markdown files.
pub trait Remote: Send + Sync {
    /// The workspace, or `None` when it is still at version `known`.
    fn fetch(&self, known: Option<u64>) -> Result<Option<Snapshot>, Error>;
    /// Applies operations to the server's current graph.
    fn apply(&self, ops: &[Op]) -> Result<ApplyResult, Error>;
}

/// A remote in this process, holding a graph and its version. It applies
/// operations as a server does, which lets clients be tested without one.
#[derive(Default)]
pub struct MemoryRemote(Mutex<(Graph, u64)>);

impl Remote for MemoryRemote {
    fn fetch(&self, known: Option<u64>) -> Result<Option<Snapshot>, Error> {
        let (graph, version) = &*self.0.lock().expect("the lock is not poisoned");
        let nodes = graph.nodes().cloned().map(Into::into).collect();
        Ok((known != Some(*version)).then_some(Snapshot { version: *version, nodes }))
    }

    fn apply(&self, ops: &[Op]) -> Result<ApplyResult, Error> {
        let mut state = self.0.lock().expect("the lock is not poisoned");
        let mut graph = state.0.clone();
        let created = ops::apply(&mut graph, &mut ops.to_vec())?;
        *state = (graph, state.1 + 1);
        Ok(ApplyResult { version: state.1, created })
    }
}

/// A `.topo` directory on disk and the graph loaded from it.
///
/// The Markdown files are the single source of truth, unless the workspace is
/// linked to a [`Remote`], which then is. Mutate `graph`, then call
/// [`Workspace::save`], which writes exactly the nodes that changed.
pub struct Workspace {
    dir: PathBuf,
    pub graph: Graph,
    saved: BTreeMap<NodeId, Node>,
    /// The remote and the version of `saved`.
    remote: Option<(Arc<dyn Remote>, u64)>,
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

    /// The nearest `.topo` directory in `start` or its ancestors.
    pub fn find_dir(start: &Path) -> Result<PathBuf, Error> {
        start
            .ancestors()
            .map(|a| a.join(DIR_NAME))
            .find(|d| d.is_dir())
            .ok_or_else(|| Error::NoWorkspace(start.to_owned()))
    }

    /// Opens the nearest `.topo` directory in `start` or its ancestors.
    pub fn discover(start: &Path) -> Result<Self, Error> {
        Self::open(Self::find_dir(start)?)
    }

    /// Opens a `.topo` directory whose nodes are its Markdown files.
    pub fn open(dir: PathBuf) -> Result<Self, Error> {
        let graph = read_nodes(&dir.join(NODES_DIR))?;
        let saved = graph.clone().into_nodes();
        Ok(Self { dir, graph, saved, remote: None })
    }

    /// Opens a `.topo` directory whose nodes are held by `remote`.
    pub fn open_remote(dir: PathBuf, remote: Arc<dyn Remote>) -> Result<Self, Error> {
        let snapshot = remote.fetch(None)?.expect("a fetch without a known version returns the workspace");
        let mut ws = Self { dir, graph: Graph::default(), saved: BTreeMap::new(), remote: Some((remote, 0)) };
        ws.install(snapshot)?;
        Ok(ws)
    }

    /// The `.topo` directory.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The directory that holds one Markdown file per node.
    pub fn nodes_dir(&self) -> PathBuf {
        self.dir.join(NODES_DIR)
    }

    /// The remote and the version this workspace has of it, to fetch from another thread.
    pub fn remote(&self) -> Option<(Arc<dyn Remote>, u64)> {
        self.remote.clone()
    }

    /// Replaces the graph with a snapshot of the remote, discarding unsaved changes.
    pub fn install(&mut self, snapshot: Snapshot) -> Result<(), Error> {
        let (_, version) = self.remote.as_mut().expect("only a remote workspace installs snapshots");
        let nodes: Vec<Node> = snapshot.nodes.into_iter().map(Node::from).collect();
        nodes.iter().try_for_each(|node| node.id.validate())?;
        self.graph = Graph::from_nodes(nodes)?;
        self.saved = self.graph.clone().into_nodes();
        *version = snapshot.version;
        Ok(())
    }

    /// Loads the nodes again if the files or the remote may hold other ones.
    /// Returns whether the graph changed.
    pub fn reload(&mut self) -> Result<bool, Error> {
        let before = self.graph.clone();
        match self.remote.clone() {
            None => {
                self.graph = read_nodes(&self.nodes_dir())?;
                self.saved = self.graph.clone().into_nodes();
            }
            Some((remote, version)) => {
                if let Some(snapshot) = remote.fetch(Some(version))? {
                    self.install(snapshot)?;
                }
            }
        }
        Ok(self.graph != before)
    }

    /// Writes changed and new nodes and deletes removed ones.
    pub fn save(&mut self) -> Result<(), Error> {
        let current = self.graph.clone().into_nodes();
        let Some((remote, version)) = self.remote.clone() else {
            return self.save_files(current);
        };
        let ops = ops::diff(&self.saved, &current);
        if ops.is_empty() {
            return Ok(());
        }
        let result = remote.apply(&ops)?;
        if result.version == version + 1 {
            self.saved = current;
            self.remote = Some((remote, result.version));
            return Ok(());
        }
        // Another writer got in between, so the server's graph is more than `current`.
        self.reload().map(|_| ())
    }

    fn save_files(&mut self, current: BTreeMap<NodeId, Node>) -> Result<(), Error> {
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

    /// Applies a batch of operations and saves, all or nothing. Returns the ids
    /// created for each `ref`. Unlike a mutation of `graph` followed by
    /// [`Workspace::save`], a remote applies exactly these operations, so their
    /// preconditions are checked against its current graph.
    pub fn apply(&mut self, mut ops: Vec<Op>) -> Result<BTreeMap<String, NodeId>, Error> {
        match self.remote.clone() {
            None => {
                let mut graph = self.graph.clone();
                let created = ops::apply(&mut graph, &mut ops)?;
                self.graph = graph;
                self.save()?;
                Ok(created)
            }
            Some((remote, _)) => {
                let result = remote.apply(&ops)?;
                self.reload()?;
                Ok(result.created)
            }
        }
    }

    pub fn node_path(&self, id: &NodeId) -> PathBuf {
        self.nodes_dir().join(format!("{id}.md"))
    }
}

fn read_nodes(nodes_dir: &Path) -> Result<Graph, Error> {
    let entries = fs::read_dir(nodes_dir).map_err(|e| Error::io(nodes_dir, e))?;
    let mut nodes = Vec::new();
    for entry in entries {
        let path = entry.map_err(|e| Error::io(nodes_dir, e))?.path();
        if path.extension().is_some_and(|ext| ext == "md") {
            let text = fs::read_to_string(&path).map_err(|e| Error::io(&path, e))?;
            nodes.push(parse(&text, &path)?);
        }
    }
    Graph::from_nodes(nodes)
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

    fn add(id: &str) -> Op {
        serde_json::from_value(serde_json::json!({ "op": "add", "id": id, "title": id })).unwrap()
    }

    #[test]
    fn remote_workspace_sends_changes_and_merges_other_writers() {
        let tmp = tempfile::tempdir().unwrap();
        let remote = Arc::new(MemoryRemote::default());
        remote.apply(&[add("a")]).unwrap();
        let open = || Workspace::open_remote(tmp.path().to_owned(), remote.clone()).unwrap();
        let (mut one, mut two) = (open(), open());
        let a = NodeId("a".into());

        one.graph.set_status(&a, Status::Doing).unwrap();
        one.save().unwrap();
        assert!(!one.reload().unwrap());
        assert_eq!(remote.fetch(None).unwrap().unwrap().version, 2);

        // `two` has not seen that write. Its own change is applied on top of it.
        two.graph.insert(Node::new(NodeId("b".into()), Kind::Task, "b".into())).unwrap();
        two.save().unwrap();
        assert_eq!(two.graph.get(&a).unwrap().status, Status::Doing);
        assert!(one.reload().unwrap());
        assert_eq!(one.graph, two.graph);

        let claim = |ws: &mut Workspace| {
            ws.apply(vec![Op::Status { id: "b".into(), status: Status::Doing, if_status: Some(Status::Todo) }])
        };
        claim(&mut one).unwrap();
        assert!(matches!(claim(&mut two), Err(Error::Op { .. })));
        assert!(two.reload().unwrap());
        assert_eq!(one.graph, two.graph);
    }

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

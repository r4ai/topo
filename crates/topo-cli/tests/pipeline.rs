use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::process::{Child, Command, Output, Stdio};

use serde_json::{Value, json};
use tempfile::TempDir;
use topo_core::{Graph, Kind, Node, NodeId, Status, Workspace};

struct Fixture {
    root: TempDir,
}

#[test]
fn terminal_controls_are_visible_in_text_and_preserved_in_machine_output() {
    let title = "日本語\u{1b}]52;c;YWJj\u{7}\u{1b}[2J\r\u{9b}2J";
    let mut n = node("safe01", Kind::Task, title);
    n.body = "普通のメモ\n\t次の行\u{1b}[H".into();
    let fixture = Fixture::new(vec![n]);
    for args in [vec!["ls", "--format", "text"], vec!["show", "safe01"], vec!["graph", "--format", "tree"]] {
        let output = fixture.text(&args, "");
        assert!(output.contains("日本語"));
        assert!(!output.chars().any(|c| c.is_control() && c != '\n' && c != '\t'), "{output:?}");
    }
    assert_eq!(fixture.json(&["show", "safe01", "--json"], "")["title"], title);
    let jsonl = fixture.text(&["ls", "--format", "jsonl"], "");
    for line in jsonl.lines() {
        assert_eq!(serde_json::from_str::<Value>(line).unwrap()["title"], title);
    }
    assert!(!jsonl.chars().any(|c| c.is_control() && c != '\n'));
    assert!(fixture.text(&["show", "safe01"], "").contains("普通のメモ\n\t次の行"));
}

#[test]
fn deep_tree_output_keeps_all_nodes_with_bounded_indentation() {
    let count = 1000;
    let fixture = Fixture::new(
        (0..count)
            .map(|i| {
                let mut n = node(&format!("n{i:04}"), Kind::Task, "deep task");
                if i + 1 < count {
                    n.depends_on.push(id(&format!("n{:04}", i + 1)));
                }
                n
            })
            .collect(),
    );
    let output = fixture.text(&["graph", "--format", "tree"], "");
    assert_eq!(output.lines().count(), count);
    assert!(output.lines().all(|line| line.len() < 120));
    assert!(output.len() < count * 120);
    assert!(output.contains("depth 999"));
    assert!(output.contains("n0999"));
}

impl Fixture {
    fn new(nodes: Vec<Node>) -> Self {
        let root = tempfile::tempdir().unwrap();
        let mut ws = Workspace::init(root.path()).unwrap();
        ws.graph = Graph::from_nodes(nodes).unwrap();
        ws.save().unwrap();
        Self { root }
    }

    fn sample() -> Self {
        let mut start = node("start1", Kind::Task, "Prepare GUI");
        start.tags = vec!["gui".into(), "urgent".into()];
        start.due = Some(jiff::civil::date(2026, 10, 1));
        start.body = "Notes preserved in machine output.\n".into();
        let mut work = node("work01", Kind::Task, "Build UI");
        work.status = Status::Doing;
        work.depends_on = vec![id("start1")];
        work.milestones = vec![id("mile01"), id("mile02")];
        work.tags = vec!["gui".into(), "urgent".into()];
        work.due = Some(jiff::civil::date(2026, 10, 2));
        let mut cli = node("work02", Kind::Task, "CLI build");
        cli.depends_on = vec![id("start1")];
        cli.milestones = vec![id("mile01")];
        cli.tags = vec!["cli".into()];
        cli.due = Some(jiff::civil::date(2026, 10, 3));
        let mut done = node("done01", Kind::Task, "Completed UI");
        done.status = Status::Done;
        done.milestones = vec![id("mile01")];
        done.tags = vec!["gui".into()];
        let mut dropped = node("drop01", Kind::Task, "Old task");
        dropped.status = Status::Dropped;
        dropped.milestones = vec![id("mile02")];
        let mut free = node("free01", Kind::Task, "Unscheduled GUI review");
        free.tags = vec!["gui".into()];
        let mut final_task = node("final1", Kind::Task, "Finish release");
        final_task.depends_on = vec![id("mile01")];
        Self::new(vec![
            start,
            work,
            cli,
            done,
            dropped,
            free,
            final_task,
            node("mile01", Kind::Milestone, "Release"),
            node("mile02", Kind::Milestone, "Launch"),
        ])
    }

    fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_topo"));
        command.current_dir(self.root.path()).arg("--topo-dir").arg(self.root.path().join(".topo")).args(args);
        command
    }

    fn run(&self, args: &[&str], input: &str) -> Output {
        let mut child =
            self.command(args).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
        child.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();
        child.wait_with_output().unwrap()
    }

    fn text(&self, args: &[&str], input: &str) -> String {
        success(self.run(args, input))
    }

    fn json(&self, args: &[&str], input: &str) -> Value {
        serde_json::from_str(&self.text(args, input)).unwrap()
    }

    fn ids(&self, args: &[&str], input: &str) -> Vec<String> {
        self.text(args, input).lines().map(str::to_owned).collect()
    }

    fn workspace(&self) -> Workspace {
        Workspace::open(self.root.path().join(".topo")).unwrap()
    }

    fn files(&self) -> BTreeMap<String, Vec<u8>> {
        fs::read_dir(self.root.path().join(".topo/nodes"))
            .unwrap()
            .map(|entry| {
                let entry = entry.unwrap();
                (entry.file_name().into_string().unwrap(), fs::read(entry.path()).unwrap())
            })
            .collect()
    }
}

fn id(value: &str) -> NodeId {
    NodeId(value.into())
}

fn node(id: &str, kind: Kind, title: &str) -> Node {
    Node::new(NodeId(id.into()), kind, title.into())
}

fn success(output: Output) -> String {
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(output.stderr.is_empty(), "{}", String::from_utf8_lossy(&output.stderr));
    String::from_utf8(output.stdout).unwrap()
}

fn failure(output: Output, expected: &str) {
    assert!(!output.status.success());
    assert!(output.stdout.is_empty(), "failed commands must not emit success records");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains(expected), "expected {expected:?} in {stderr:?}");
}

#[test]
fn listing_formats_preserve_text_and_json_compatibility_when_piped() {
    let fixture = Fixture::sample();
    for command in ["ls", "ready", "milestones"] {
        let default = fixture.text(&[command], "");
        assert_eq!(default, fixture.text(&[command, "--format", "text"], ""));
        assert!(default.contains("[ ]"), "the default remains human-readable when stdout is a pipe");

        let records = fixture.json(&[command, "--json"], "");
        let lines = fixture.text(&[command, "--format", "jsonl"], "");
        let jsonl: Vec<Value> = lines.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
        assert_eq!(Value::Array(jsonl), records);
        let expected_ids: Vec<String> =
            records.as_array().unwrap().iter().map(|record| record["id"].as_str().unwrap().into()).collect();
        assert_eq!(fixture.ids(&[command, "--format", "ids"], ""), expected_ids);
        assert_eq!(fixture.text(&[command, "--format", "tsv"], "").lines().count(), expected_ids.len());
    }
    let ready = fixture.json(&["ready", "--json"], "");
    let start = ready.as_array().unwrap().iter().find(|n| n["id"] == "start1").unwrap();
    assert_eq!(start["notes"], "Notes preserved in machine output.\n");
    assert_eq!(start["ready"], true);
    let milestones = fixture.json(&["milestones", "--json"], "");
    assert_eq!(milestones[0]["done"], 1);
    assert_eq!(milestones[0]["total"], 3);
    assert!(milestones[0]["critical_path"].is_array());
}

#[test]
fn tsv_has_fixed_fields_and_escapes_titles_without_splitting_records() {
    let mut task = node("escape", Kind::Task, "Tab\tline\nreturn\rslash\\end");
    task.status = Status::Doing;
    let fixture = Fixture::new(vec![task]);
    assert_eq!(
        fixture.text(&["ls", "--format", "tsv"], ""),
        "escape\ttask\tdoing\t\tTab\\tline\\nreturn\\rslash\\\\end\n"
    );
    let jsonl = fixture.text(&["ls", "--format", "jsonl"], "");
    assert_eq!(jsonl.lines().count(), 1);
    let record: Value = serde_json::from_str(jsonl.trim()).unwrap();
    assert_eq!(record["title"], "Tab\tline\nreturn\rslash\\end");

    let mut milestone = node("due001", Kind::Milestone, "Due release");
    milestone.due = Some(jiff::civil::date(2026, 10, 2));
    let fixture = Fixture::new(vec![milestone]);
    assert_eq!(
        fixture.text(&["milestones", "--format", "tsv"], ""),
        "due001\tmilestone\ttodo\t2026-10-02\tDue release\n"
    );
}

#[test]
fn invalid_formats_and_conflicting_filters_fail_before_output() {
    let fixture = Fixture::sample();
    for args in [
        vec!["ls", "--format", "csv"],
        vec!["ready", "--format", "csv"],
        vec!["milestones", "--format", "csv"],
        vec!["deps", "work01", "--format", "csv"],
    ] {
        failure(fixture.run(&args, ""), "invalid value");
    }
    for args in [
        vec!["--json", "ls", "--format", "ids"],
        vec!["--json", "deps", "work01", "--format", "text"],
        vec!["ls", "--json", "--format", "ids"],
        vec!["ready", "--json", "--format", "text"],
        vec!["milestones", "--json", "--format", "jsonl"],
        vec!["members", "mile01", "--json", "--format", "tsv"],
        vec!["ls", "--ready", "--blocked"],
        vec!["ls", "--no-due", "--due-before", "2026-10-02"],
        vec!["ls", "--no-due", "--due-after", "2026-10-02"],
    ] {
        failure(fixture.run(&args, ""), "cannot be used with");
    }
}

#[test]
fn ls_filters_combine_and_respect_open_node_semantics() {
    let fixture = Fixture::sample();
    for (filters, expected) in [
        (vec!["--tag", "gui", "--tag", "urgent"], vec!["start1", "work01"]),
        (vec!["--due-before", "2026-10-02"], vec!["start1", "work01"]),
        (vec!["--due-after", "2026-10-02"], vec!["work01", "work02"]),
        (vec!["--due-before", "2026-10-02", "--due-after", "2026-10-02"], vec!["work01"]),
        (vec!["--no-due", "--kind", "task"], vec!["final1", "free01"]),
        (vec!["--in", "mile01", "--in", "mile02"], vec!["work01"]),
        (vec!["--title", "gUi"], vec!["free01", "start1"]),
        (vec!["--all", "--ready"], vec!["free01", "start1"]),
        (vec!["--all", "--blocked", "--kind", "task"], vec!["final1", "work01", "work02"]),
        (vec!["--tag", "gui", "--status", "done"], vec!["done01"]),
        (vec!["--under", "mile01", "--kind", "task", "--tag", "gui", "--title", "prepare"], vec!["start1"]),
    ] {
        let args: Vec<&str> = ["ls", "--format", "ids"].into_iter().chain(filters).collect();
        assert_eq!(fixture.ids(&args, ""), expected, "{args:?}");
    }
    failure(fixture.run(&["ls", "--in", "mile0", "--format", "ids"], ""), "ambiguous");
    failure(fixture.run(&["ls", "--in", "start1", "--format", "ids"], ""), "not a milestone");
}

#[test]
fn stdin_listing_resolves_prefixes_deduplicates_and_applies_filters() {
    let fixture = Fixture::sample();
    assert_eq!(
        fixture.ids(&["ls", "-", "--format", "ids"], "work02 start\nwork02\tfree01\n"),
        ["free01", "start1", "work02"]
    );
    assert_eq!(
        fixture
            .ids(&["ls", "-", "--tag", "gui", "--due-before", "2026-10-01", "--format", "ids"], "free01 work02 start1"),
        ["start1"]
    );
    assert!(fixture.text(&["ls", "-", "--format", "ids"], " \n\t").is_empty());
    assert_eq!(fixture.json(&["ls", "-", "--json"], ""), json!([]));
    failure(fixture.run(&["ls", "-", "--format", "ids"], "start1 missing"), "no node matches");
    failure(fixture.run(&["ls", "-", "--format", "ids"], "start1 work0"), "ambiguous");
}

fn pipe(fixture: &Fixture, args: &[&str], input: Stdio) -> Child {
    fixture.command(args).stdin(input).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap()
}

#[test]
fn real_pipeline_composes_traversal_filtering_and_bulk_status() {
    let fixture = Fixture::sample();
    let mut traversal = pipe(&fixture, &["deps", "mile01", "--transitive"], Stdio::null());
    let mut filter =
        pipe(&fixture, &["ls", "-", "--tag", "cli", "--format", "ids"], Stdio::from(traversal.stdout.take().unwrap()));
    let status = pipe(&fixture, &["status", "-", "doing"], Stdio::from(filter.stdout.take().unwrap()));
    assert!(success(status.wait_with_output().unwrap()).is_empty());
    assert!(success(filter.wait_with_output().unwrap()).is_empty());
    assert!(success(traversal.wait_with_output().unwrap()).is_empty());
    let ws = fixture.workspace();
    assert_eq!(ws.graph.get(&id("work02")).unwrap().status, Status::Doing);
    assert_eq!(ws.graph.get(&id("start1")).unwrap().status, Status::Todo);
    assert_eq!(ws.graph.get(&id("free01")).unwrap().status, Status::Todo);
}

#[test]
fn stdin_consumers_see_nodes_created_by_the_upstream_process() {
    for args in [vec!["status", "-", "doing"], vec!["ls", "-", "--format", "ids"], vec!["apply"]] {
        let fixture = Fixture::new(vec![]);
        let mut consumer = pipe(&fixture, &args, Stdio::piped());
        // Give the consumer time to start while its input remains open, then
        // create the node whose ID the upstream command sends down the pipe.
        std::thread::sleep(std::time::Duration::from_millis(250));
        let created = fixture.text(&["add", "Created upstream"], "").trim().to_owned();
        let input = if args[0] == "apply" {
            json!([{ "op": "status", "id": created, "status": "doing" }]).to_string()
        } else {
            format!("{created}\n")
        };
        consumer.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();
        let output = success(consumer.wait_with_output().unwrap());
        if args[0] == "ls" {
            assert_eq!(output, format!("{created}\n"));
        } else {
            assert!(output.is_empty());
            assert_eq!(fixture.workspace().graph.get(&id(&created)).unwrap().status, Status::Doing);
        }
    }
}

#[test]
fn closing_the_output_pipe_early_exits_without_a_panic_or_error() {
    let mut task = node("large1", Kind::Task, "Large output");
    task.body = "n".repeat(128 * 1024);
    let fixture = Fixture::new(vec![task]);
    let mut child = pipe(&fixture, &["ls", "--format", "jsonl"], Stdio::null());
    drop(child.stdout.take());
    assert!(success(child.wait_with_output().unwrap()).is_empty());
}

#[test]
fn bulk_mutations_preserve_input_order_and_apply_each_resolved_id_once() {
    for (args, input, key, expected) in [
        (vec!["status", "-", "done", "--json"], "work02 free work02 free01", "id", ["work02", "free01"]),
        (
            vec!["edit", "-", "--title", "Changed", "--tag", "new", "--no-due", "--note", "updated", "--json"],
            "work02 free work02",
            "id",
            ["work02", "free01"],
        ),
        (vec!["rm", "-", "--json"], "start1 free start1", "removed", ["start1", "free01"]),
        (vec!["link", "-", "done01", "--json"], "free01 work02 free", "from", ["free01", "work02"]),
        (vec!["unlink", "-", "start1", "--json"], "work02 work01 work02", "from", ["work02", "work01"]),
        (vec!["join", "-", "mile02", "--json"], "free01 start1 free", "task", ["free01", "start1"]),
        (vec!["leave", "-", "mile02", "--json"], "work01 drop01 work01", "task", ["work01", "drop01"]),
    ] {
        let fixture = Fixture::sample();
        let result = fixture.json(&args, input);
        let records = result.as_array().unwrap();
        assert_eq!(records.iter().map(|record| record[key].as_str().unwrap()).collect::<Vec<_>>(), expected);
        let ws = fixture.workspace();
        match args[0] {
            "status" => {
                for value in expected {
                    assert_eq!(ws.graph.get(&id(value)).unwrap().status, Status::Done);
                }
            }
            "edit" => {
                for value in expected {
                    let node = ws.graph.get(&id(value)).unwrap();
                    assert_eq!(node.title, "Changed");
                    assert_eq!(node.tags, ["new"]);
                    assert_eq!(node.due, None);
                    assert_eq!(node.body, "updated");
                }
            }
            "rm" => {
                for value in expected {
                    assert!(ws.graph.get(&id(value)).is_none());
                }
                assert!(ws.graph.get(&id("work01")).unwrap().depends_on.is_empty());
                assert!(ws.graph.get(&id("work02")).unwrap().depends_on.is_empty());
            }
            "link" | "unlink" => {
                for value in expected {
                    let linked = ws.graph.get(&id(value)).unwrap().depends_on.contains(&id(args[2]));
                    assert_eq!(linked, args[0] == "link");
                    assert_eq!(records[0]["to"], args[2]);
                }
            }
            "join" | "leave" => {
                for value in expected {
                    let member = ws.graph.get(&id(value)).unwrap().milestones.contains(&id(args[2]));
                    assert_eq!(member, args[0] == "join");
                    assert_eq!(records[0]["milestone"], args[2]);
                }
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn empty_stdin_is_a_noop_for_every_bulk_mutation() {
    let fixture = Fixture::sample();
    let before = fixture.files();
    for args in [
        vec!["status", "-", "done", "--json"],
        vec!["edit", "-", "--title", "Unused", "--json"],
        vec!["rm", "-", "--json"],
        vec!["link", "-", "start1", "--json"],
        vec!["unlink", "-", "start1", "--json"],
        vec!["join", "-", "mile01", "--json"],
        vec!["leave", "-", "mile01", "--json"],
    ] {
        assert_eq!(fixture.json(&args, " \n\t"), json!([]), "{args:?}");
        assert_eq!(fixture.files(), before);
    }
}

#[test]
fn bulk_mutations_keep_stdout_empty_without_json() {
    for (args, input) in [
        (vec!["status", "-", "done"], "free01 start1"),
        (vec!["edit", "-", "--title", "Changed"], "free01 start1"),
        (vec!["rm", "-"], "free01 start1"),
        (vec!["link", "-", "done01"], "free01 start1"),
        (vec!["unlink", "-", "start1"], "work01 work02"),
        (vec!["join", "-", "mile02"], "free01 start1"),
        (vec!["leave", "-", "mile01"], "work01 work02"),
    ] {
        let fixture = Fixture::sample();
        assert!(fixture.text(&args, input).is_empty(), "{args:?}");
        assert!(fixture.text(&args, "").is_empty(), "{args:?}");
    }
}

#[test]
fn bulk_mutations_reject_unknown_or_ambiguous_ids_without_writing_files() {
    for args in [
        vec!["status", "-", "done"],
        vec!["edit", "-", "--title", "Changed"],
        vec!["rm", "-"],
        vec!["link", "-", "done01"],
        vec!["unlink", "-", "start1"],
        vec!["join", "-", "mile02"],
        vec!["leave", "-", "mile01"],
    ] {
        let fixture = Fixture::sample();
        let before = fixture.files();
        failure(fixture.run(&args, "work01 missing"), "no node matches");
        assert_eq!(fixture.files(), before, "{args:?}");
        failure(fixture.run(&args, "work01 work0"), "ambiguous");
        assert_eq!(fixture.files(), before, "{args:?}");
    }
}

#[test]
fn domain_errors_roll_back_successful_earlier_mutations_in_the_batch() {
    for (args, input, error) in [
        (vec!["link", "-", "work01"], "free01 start1", "cycle"),
        (vec!["unlink", "-", "start1"], "work01 free01", "does not depend"),
        (vec!["join", "-", "mile02"], "free01 mile01", "only tasks can belong"),
        (vec!["join", "-", "mile01"], "free01 final1", "cycle"),
        (vec!["leave", "-", "mile01"], "work01 free01", "not in milestone"),
        (vec!["edit", "-", "--kind", "milestone"], "free01 work01", "cannot change the kind"),
    ] {
        let fixture = Fixture::sample();
        let before = fixture.files();
        failure(fixture.run(&args, input), error);
        assert_eq!(fixture.files(), before, "{args:?}");
    }
}

#[test]
fn single_id_mutations_keep_their_existing_json_object_shape() {
    for (args, key, value) in [
        (vec!["status", "free01", "done", "--json"], "id", "free01"),
        (vec!["edit", "free01", "--title", "Changed", "--json"], "id", "free01"),
        (vec!["rm", "free01", "--json"], "removed", "free01"),
        (vec!["link", "free01", "start1", "--json"], "from", "free01"),
        (vec!["unlink", "work01", "start1", "--json"], "from", "work01"),
        (vec!["join", "free01", "mile01", "--json"], "task", "free01"),
        (vec!["leave", "work01", "mile01", "--json"], "task", "work01"),
    ] {
        let fixture = Fixture::sample();
        let record = fixture.json(&args, "");
        assert!(record.is_object(), "{args:?}");
        assert_eq!(record[key], value);
    }
}

#[test]
fn traversal_commands_expose_requirement_membership_and_execution_order() {
    let fixture = Fixture::sample();
    for (args, expected) in [
        (vec!["deps", "work01"], vec!["start1"]),
        (vec!["deps", "mile01"], vec!["done01", "work01", "work02"]),
        (vec!["deps", "mile01", "--transitive"], vec!["done01", "start1", "work01", "work02"]),
        (vec!["dependents", "start1"], vec!["work01", "work02"]),
        (vec!["dependents", "start1", "--transitive"], vec!["final1", "mile01", "mile02", "work01", "work02"]),
        (vec!["members", "mile02"], vec!["drop01", "work01"]),
        (vec!["critical-path", "final1"], vec!["start1", "work02", "final1"]),
        (vec!["deps", "free01"], vec![]),
        (vec!["critical-path", "done01"], vec![]),
    ] {
        assert_eq!(fixture.ids(&args, ""), expected, "{args:?}");
        let json_args: Vec<&str> = args.iter().copied().chain(["--json"]).collect();
        let records = fixture.json(&json_args, "");
        assert_eq!(
            records.as_array().unwrap().iter().map(|record| record["id"].as_str().unwrap()).collect::<Vec<_>>(),
            expected
        );
        let jsonl_args: Vec<&str> = args.iter().copied().chain(["--format", "jsonl"]).collect();
        let jsonl: Vec<Value> =
            fixture.text(&jsonl_args, "").lines().map(|line| serde_json::from_str(line).unwrap()).collect();
        assert_eq!(Value::Array(jsonl), records);
    }
    failure(fixture.run(&["members", "work01"], ""), "not a milestone");
    failure(fixture.run(&["deps", "missing"], ""), "no node matches");
    failure(fixture.run(&["dependents", "work0"], ""), "ambiguous");
}

#[test]
fn empty_queries_have_no_phantom_lines_in_any_line_format() {
    let fixture = Fixture::new(vec![]);
    for command in ["ls", "ready", "milestones"] {
        for format in ["text", "ids", "tsv", "jsonl"] {
            assert!(fixture.text(&[command, "--format", format], "").is_empty());
        }
        assert_eq!(fixture.json(&[command, "--json"], ""), json!([]));
    }
}

#!/usr/bin/env python3
"""Exercise metadata through the real CLI in a disposable linked workspace.

Usage: metadata-smoke.py /path/to/topo /path/to/workspace
Creates four test nodes, then pulls the workspace to verify stored values.
The caller owns the workspace and is responsible for remote cleanup.
"""

import datetime
import json
import pathlib
import subprocess
import sys
import time


def main():
    cli, directory = sys.argv[1:]
    directory = pathlib.Path(directory)

    def run(*args, data=None, succeeds=True):
        result = subprocess.run(
            [cli, *args], cwd=directory, input=data, text=True, capture_output=True
        )
        if succeeds:
            if result.returncode:
                raise RuntimeError(result.stderr)
        else:
            assert result.returncode != 0, "invalid operation succeeded"
        return result.stdout

    def value(*args):
        return json.loads(run(*args, "--json"))

    def show(node):
        return value("show", node)

    def nodes():
        return value("ls", "--all")

    def moment(node, field):
        text = node[field]
        assert text.endswith("Z"), text
        return datetime.datetime.fromisoformat(text.replace("Z", "+00:00"))

    started = datetime.datetime.now(datetime.timezone.utc) - datetime.timedelta(seconds=1)
    task = value("add", "Metadata smoke: urgent", "--priority", "urgent", "--tag", "metadata-smoke")
    task_id = task["id"]
    low = value("add", "Metadata smoke: low", "--priority", "low", "--tag", "metadata-smoke")["id"]
    unset = value("add", "Metadata smoke: unset", "--tag", "metadata-smoke")["id"]
    blocked = value(
        "add", "Metadata smoke: blocked", "--priority", "high", "--dep", task_id, "--tag", "metadata-smoke"
    )["id"]
    assert task["created_at"] == task["updated_at"]
    assert started <= moment(task, "created_at") <= datetime.datetime.now(datetime.timezone.utc)
    assert "completed_at" not in task
    assert blocked not in {n["id"] for n in value("ready", "--sort", "priority")}
    assert [n["id"] for n in value("ls", "--tag", "metadata-smoke", "--sort", "priority")] == [
        task_id, blocked, low, unset
    ]
    assert [n["id"] for n in value("ls", "--tag", "metadata-smoke", "--no-priority")] == [unset]
    assert {n["id"] for n in value("ls", "--tag", "metadata-smoke", "--priority", "urgent", "--priority", "high")} == {
        task_id, blocked
    }
    incoming = run("ls", "-", "--no-priority", "--format", "jsonl", data=task_id + " " + unset)
    assert json.loads(incoming)["id"] == unset

    before = nodes()
    run("edit", task_id, "--priority", "urgent")
    run("status", task_id, "todo")
    assert nodes() == before, "no-op edits changed timestamps"
    run(
        "apply",
        data=json.dumps([
            {"op": "edit", "id": task_id, "priority": "medium"},
            {"op": "link", "from": task_id, "to": blocked},
        ]),
        succeeds=False,
    )
    assert nodes() == before, "failed batch changed nodes or timestamps"

    # The recorded precision is seconds; cross one boundary to check real updates.
    time.sleep(1.1)
    edited = value("edit", task_id, "--priority", "high", "--assignee", "smoke", "--pr", "r4ai/topo#7")
    assert edited["created_at"] == task["created_at"]
    assert moment(edited, "updated_at") > moment(task, "updated_at")
    assert edited["priority"] == "high" and edited["assignee"] == "smoke"
    assert edited["prs"] == ["https://github.com/r4ai/topo/pull/7"]

    # A lost claim must not update either assignee or timestamps.
    run("status", task_id, "doing", "--if", "todo", "--assign", "winner")
    claimed = show(task_id)
    assert claimed["assignee"] == "winner"
    run("status", task_id, "doing", "--if", "todo", "--assign", "loser", succeeds=False)
    assert show(task_id) == claimed

    done = value("status", task_id, "done")
    assert done["completed_at"] == done["updated_at"]
    assert blocked in {n["id"] for n in value("ready")}
    completed = done["completed_at"]
    assert value("edit", task_id, "--note", "Edited after completion")["completed_at"] == completed
    reopened = value("status", task_id, "doing")
    assert "completed_at" not in reopened
    assert "completed Not completed" in run("show", task_id)
    assert blocked not in {n["id"] for n in value("ready")}
    dropped = value("status", task_id, "dropped")
    assert "completed_at" not in dropped
    cleared = value("edit", task_id, "--no-priority", "--no-assignee", "--unpr", "r4ai/topo#7")
    assert all(field not in cleared for field in ["priority", "assignee", "prs"])

    # Leave a completed urgent node for rendering the cloud-backed inspector.
    run("edit", task_id, "--priority", "urgent")
    final = value("status", task_id, "done")
    assert final["created_at"] == task["created_at"]
    assert "completed " + final["completed_at"] in run("show", task_id)
    before = nodes()
    run("cloud", "pull")
    assert nodes() == before, "pull lost metadata or timestamps"
    assert (directory / ".topo" / "nodes" / (task_id + ".md")).is_file()
    print(json.dumps({"result": "metadata smoke passed", "node": task_id, "nodes": len(before)}))


if __name__ == "__main__":
    main()

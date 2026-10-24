"""Reject mismatched tags, versions, workflow refs, and commits outside main."""
import os
import re
import subprocess
import tomllib


def git(*args):
    return subprocess.check_output(["git", *args], text=True).strip()


tag = os.environ["RELEASE_TAG"]
if not re.fullmatch(r"v\d+\.\d+\.\d+", tag):
    raise SystemExit("Release tags must be stable versions such as v0.1.0")
release_branch = os.environ["GITHUB_REF"] == f"refs/heads/release/{tag}"
if not release_branch and os.environ["GITHUB_REF"] != f"refs/tags/{tag}":
    raise SystemExit("Run manual releases with --ref TAG and the same tag input")
sha = git("rev-parse", "HEAD") if release_branch else git("rev-parse", "--verify", f"refs/tags/{tag}^{{commit}}")
if release_branch:
    existing = subprocess.run(
        ["git", "rev-parse", "--verify", f"refs/tags/{tag}^{{commit}}"],
        text=True, capture_output=True,
    )
    if existing.returncode == 0 and existing.stdout.strip() != sha:
        raise SystemExit("An existing release tag must never be moved")
if sha != git("rev-parse", "HEAD"):
    raise SystemExit("The workflow must run from the release tag commit")
subprocess.run(["git", "merge-base", "--is-ancestor", sha, "origin/main"], check=True)
with open("Cargo.toml", "rb") as manifest:
    version = tomllib.load(manifest)["workspace"]["package"]["version"]
if tag != f"v{version}":
    raise SystemExit(f"Tag {tag} does not match workspace version {version}")
# A patch release with an unchanged schema must not touch the production D1 database.
previous = subprocess.run(
    ["git", "describe", "--tags", "--match", "v[0-9]*", "--abbrev=0", f"{sha}^"],
    text=True, capture_output=True,
)
migrations_changed = True
if previous.returncode == 0:
    comparison = subprocess.run(
        ["git", "diff", "--quiet", previous.stdout.strip(), sha, "--", "crates/topo-server/migrations"],
        check=False,
    )
    if comparison.returncode not in (0, 1):
        raise SystemExit("Could not determine whether database migrations changed")
    migrations_changed = comparison.returncode == 1
with open(os.environ["GITHUB_OUTPUT"], "a") as output:
    output.write(f"tag={tag}\nsha={sha}\n")
    output.write(f"migrations_changed={str(migrations_changed).lower()}\n")

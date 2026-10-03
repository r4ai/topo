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
with open(os.environ["GITHUB_OUTPUT"], "a") as output:
    output.write(f"tag={tag}\nsha={sha}\n")

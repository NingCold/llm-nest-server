"""Validate the server version and immutable migration before preparing a release."""
import argparse
import hashlib
import subprocess
from pathlib import Path
import tomllib
ROOT = Path(__file__).resolve().parents[1]
def validate_tag(version, tag, root=ROOT):
    if tag != "v" + version:
        raise ValueError(f"Tag {tag} does not match server version {version}")
    tagged = subprocess.check_output(["git", "rev-parse", "--verify", f"refs/tags/{tag}^{{commit}}"], cwd=root, text=True).strip()
    head = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip()
    if tagged != head:
        raise ValueError("Checkout does not match the requested release tag")

def validate(tag=None):
    workspace = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))["workspace"]
    assert set(workspace["members"]) == {"server", "connector", "crates/social"}
    for member in workspace["members"]:
        package = tomllib.loads((ROOT / member / "Cargo.toml").read_text(encoding="utf-8"))["package"]
        assert package["version"] == {"workspace": True}
    digest = hashlib.sha256((ROOT / "server/migrations/0008_migration.sql").read_bytes()).hexdigest().upper()
    assert digest == "E7E0726E1BB5B5ED3F3349E0971E13C645C8AF0354F88793B9760C7A52074175", "Deployed migration changed"
    if tag is not None:
        validate_tag(workspace["package"]["version"], tag)
    print("Server release inputs OK: " + workspace["package"]["version"])
if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["validate"])
    parser.add_argument("--tag", help="Existing release tag; must match version and checked out commit")
    args = parser.parse_args()
    validate(args.tag)

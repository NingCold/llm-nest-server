"""Check that this workspace contains only the server and public protocol packages."""
import json
from pathlib import Path
import subprocess
ROOT = Path(__file__).resolve().parents[1]
def main():
    metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--no-deps", "--format-version", "1", "--locked"], cwd=ROOT, text=True, encoding="utf-8"))
    packages = {p["name"]: p for p in metadata["packages"]}
    assert set(packages) == {"cloud-server", "connector", "social"}, "Unexpected workspace package"
    for name, package in packages.items():
        for dependency in package["dependencies"]:
            if dependency.get("path"):
                assert Path(dependency["path"]).resolve().is_relative_to(ROOT), "Dependency leaves repository"
                assert dependency["name"] in packages, "Unexpected local dependency"
                assert name == "cloud-server" or dependency["name"] != "cloud-server", "Protocol depends on server"
    docker = (ROOT / "server/Dockerfile").read_text(encoding="utf-8")
    assert not any(line.startswith("COPY ") and any(token in line for token in ("client/", "agent/")) for line in docker.splitlines())
    print("Server and protocol boundaries OK")
if __name__ == "__main__":
    main()

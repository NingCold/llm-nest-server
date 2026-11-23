"""Create an isolated cloud Docker context from an explicit, verified Web artifact."""
import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import shutil
import subprocess
import tarfile

ROOT = Path(__file__).resolve().parents[1]
MEMBERS = ('server', 'crates/social')
WEB_NOTICES = ('LICENSE.txt', 'NOTICE.txt', 'THIRD_PARTY_NOTICES.txt')


def create(output, web, manifest_path, version, notices, root=ROOT):
    if not notices.read_text(encoding='utf-8').strip():
        raise ValueError('Rust third-party notices are empty')
    manifest = json.loads(manifest_path.read_text(encoding='utf-8'))
    if not version or version == 'latest' or manifest.get('schemaVersion') != 1 or manifest.get('webVersion') != version:
        raise ValueError('Web artifact version mismatch')
    with web.open('rb') as stream:
        digest = hashlib.file_digest(stream, 'sha256').hexdigest()
    if digest != manifest.get('sha256'):
        raise ValueError('Web artifact SHA256 mismatch')
    with tarfile.open(web, 'r:gz') as tar:
        entries = tar.getmembers()
        if any(not (e.isfile() or e.isdir()) or PurePosixPath(e.name).is_absolute() or '..' in PurePosixPath(e.name).parts or '\\' in e.name or ':' in e.name for e in entries):
            raise ValueError('Web archive contains an unsafe path or link')
        if len({PurePosixPath(e.name) for e in entries}) != len(entries):
            raise ValueError('Web archive contains duplicate paths')
        if not any(e.name == 'index.html' and e.isfile() for e in entries):
            raise ValueError('Web archive has no index.html')
        for name in WEB_NOTICES:
            entry = next((e for e in entries if e.name == name and e.isfile()), None)
            if entry is None or not tar.extractfile(entry).read().strip():
                raise ValueError(f'Web artifact has missing or empty notice: {name}')
        if output.exists() and any(output.iterdir()):
            raise ValueError('Build context must be empty')
        output.mkdir(parents=True, exist_ok=True)
        (output / 'web').mkdir()
        for entry in entries:
            target = output / 'web' / entry.name
            if entry.isdir():
                target.mkdir(parents=True, exist_ok=True)
            else:
                target.parent.mkdir(parents=True, exist_ok=True)
                with tar.extractfile(entry) as source, target.open('wb') as destination:
                    shutil.copyfileobj(source, destination)
    for member in MEMBERS:
        # Copy the tracked working-tree snapshot, including staged new files and local edits; never build/data directories.
        tracked = subprocess.check_output(['git', 'ls-files', '-z', '--', member], cwd=root).decode().split('\0')
        for name in filter(None, tracked):
            source = root / name
            if source.is_file():
                destination = output / name
                destination.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(source, destination)
    shutil.copy2(root / 'Cargo.toml', output / 'Cargo.toml')
    shutil.copy2(root / 'Cargo.lock', output / 'Cargo.lock')
    shutil.copy2(root / 'server/Dockerfile', output / 'Dockerfile')
    shutil.copy2(manifest_path, output / 'web-manifest.json')
    shutil.copy2(root / 'LICENSE', output / 'LICENSE')
    shutil.copy2(root / 'NOTICE', output / 'NOTICE')
    shutil.copy2(notices, output / 'THIRD-PARTY-NOTICES.txt')
    return output


if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--output', type=Path, required=True)
    p.add_argument('--web-artifact', type=Path, required=True)
    p.add_argument('--web-manifest', type=Path, required=True)
    p.add_argument('--web-version', required=True)
    p.add_argument('--third-party-notices', type=Path, required=True, help='cargo-about generated Rust dependency notices')
    a = p.parse_args()
    print(create(a.output, a.web_artifact, a.web_manifest, a.web_version, a.third_party_notices))

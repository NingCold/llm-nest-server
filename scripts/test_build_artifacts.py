"""Artifact boundaries, using fixtures without Cargo, Docker or network."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import tarfile
import io
import hashlib


def load(name):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(name + '.py'))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class WebFixture:
    @staticmethod
    def package(dist, output, version):
        output.mkdir()
        archive = output / 'fixture.tar.gz'
        with tarfile.open(archive, 'w:gz') as tar:
            tar.add(dist / 'index.html', arcname='index.html')
            for name in ('LICENSE.txt', 'NOTICE.txt', 'THIRD_PARTY_NOTICES.txt'):
                entry = tarfile.TarInfo(name)
                body = f'Fixture Web {name}'.encode()
                entry.size = len(body)
                tar.addfile(entry, io.BytesIO(body))
        (output / 'web-manifest.json').write_text(json.dumps({'schemaVersion': 1, 'webVersion': version, 'sha256': hashlib.sha256(archive.read_bytes()).hexdigest()}))
        return archive
web = WebFixture()
cloud = load('prepare-cloud-context')


class ArtifactTests(unittest.TestCase):
    def test_archive_path_escape_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            notices = root / 'notices.txt'
            notices.write_text('Fixture dependency license text')
            for name in ('../escape', 'C:/escape', '/escape'):
                artifact = root / 'web.tar.gz'
                with tarfile.open(artifact, 'w:gz') as tar:
                    entry = tarfile.TarInfo(name)
                    entry.size = 1
                    tar.addfile(entry, io.BytesIO(b'x'))
                manifest = root / 'manifest.json'
                manifest.write_text(json.dumps({'schemaVersion': 1, 'webVersion': '1.2.3', 'sha256': hashlib.sha256(artifact.read_bytes()).hexdigest()}))
                with self.assertRaisesRegex(ValueError, 'unsafe path'):
                    cloud.create(root / 'out', artifact, manifest, '1.2.3', notices, lock=False)
                self.assertFalse((root / 'out').exists())

    def test_missing_or_empty_distribution_notices_are_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            notices = root / 'notices.txt'
            notices.write_text('Rust dependency license text')
            for missing in cloud.WEB_NOTICES:
                for empty in (False, True):
                    artifact = root / 'web.tar.gz'
                    with tarfile.open(artifact, 'w:gz') as tar:
                        for name in ('index.html', *cloud.WEB_NOTICES):
                            if name == missing and not empty:
                                continue
                            body = b' ' if name == missing else b'fixture'
                            entry = tarfile.TarInfo(name)
                            entry.size = len(body)
                            tar.addfile(entry, io.BytesIO(body))
                    manifest = root / 'manifest.json'
                    manifest.write_text(json.dumps({'schemaVersion': 1, 'webVersion': '1.2.3', 'sha256': hashlib.sha256(artifact.read_bytes()).hexdigest()}))
                    with self.assertRaisesRegex(ValueError, 'missing or empty notice'):
                        cloud.create(root / 'out', artifact, manifest, '1.2.3', notices, lock=False)
                    self.assertFalse((root / 'out').exists())
            notices.write_text('  ')
            with self.assertRaisesRegex(ValueError, 'Rust third-party notices are empty'):
                cloud.create(root / 'out', artifact, manifest, '1.2.3', notices, lock=False)

    def test_context_lock_resolution_keeps_existing_dependency_versions(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            notices = root / 'notices.txt'
            notices.write_text('Fixture dependency license text')
            repo = root / 'repository'
            repo.mkdir()
            (repo / 'Cargo.toml').write_text('[workspace.package]\nversion="1.2.3"\nedition="2024"\n[workspace.dependencies]\n')
            (repo / 'LICENSE').write_text('Fixture project license')
            (repo / 'NOTICE').write_text('Fixture project notice')
            (repo / 'Cargo.lock').write_text('[[package]]\nname="fixture"\nversion="1.2.3"\n')
            for member in cloud.MEMBERS:
                package = repo / member
                package.mkdir(parents=True)
                (package / 'Cargo.toml').write_text('[package]\nname="fixture"\nversion="1.2.3"\n')
            (repo / 'server/Dockerfile').write_text('FROM fixture')
            dist = root / 'dist'
            dist.mkdir()
            (dist / 'index.html').write_text('fixture')
            artifact = web.package(dist, root / 'artifact', '1.2.3')
            manifest = artifact.parent / 'web-manifest.json'
            def keep_lock(command, **kwargs):
                self.assertEqual(command, ['cargo', 'update', '--workspace', '--offline'])
            with patch.object(cloud.subprocess, 'check_output', return_value=b''), patch.object(cloud.subprocess, 'run', side_effect=keep_lock):
                cloud.create(root / 'same-lock', artifact, manifest, '1.2.3', notices, root=repo)
            def change_lock(command, **kwargs):
                self.assertEqual(command, ['cargo', 'update', '--workspace', '--offline'])
                self.assertEqual(kwargs['cwd'], root / 'changed-lock')
                (kwargs['cwd'] / 'Cargo.lock').write_text('[[package]]\nname="unexpected"\nversion="99.0.0"\n')
            with patch.object(cloud.subprocess, 'check_output', return_value=b''), patch.object(cloud.subprocess, 'run', side_effect=change_lock):
                with self.assertRaisesRegex(ValueError, 'changed pinned dependencies'):
                    cloud.create(root / 'changed-lock', artifact, manifest, '1.2.3', notices, root=repo)
            self.assertIn('name="fixture"', (repo / 'Cargo.lock').read_text())

    def test_web_digest_version_and_context_boundaries(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            notices = root / 'notices.txt'
            notices.write_text('Fixture dependency license text')
            dist = root / 'dist'
            dist.mkdir()
            (dist / 'index.html').write_text('fixture')
            artifact = web.package(dist, root / 'artifact', '1.2.3')
            manifest = artifact.parent / 'web-manifest.json'
            with self.assertRaisesRegex(ValueError, 'version mismatch'):
                cloud.create(root / 'bad', artifact, manifest, '2.0.0', notices, lock=False)
            context = cloud.create(root / 'context', artifact, manifest, '1.2.3', notices, lock=False)
            self.assertTrue((context / 'web/index.html').is_file())
            for name in cloud.WEB_NOTICES:
                self.assertEqual((context / 'web' / name).read_text(), f'Fixture Web {name}')
            self.assertEqual((context / 'LICENSE').read_bytes(), (cloud.ROOT / 'LICENSE').read_bytes())
            self.assertEqual((context / 'NOTICE').read_bytes(), (cloud.ROOT / 'NOTICE').read_bytes())
            self.assertEqual((context / 'THIRD-PARTY-NOTICES.txt').read_bytes(), notices.read_bytes())
            self.assertFalse((context / 'agent').exists())
            self.assertFalse((context / 'client').exists())
            self.assertFalse((context / 'crates/runtime').exists())
            import tomllib
            workspace = tomllib.loads((context / 'Cargo.toml').read_text())['workspace']
            self.assertEqual(workspace['members'], list(cloud.MEMBERS))
            artifact.write_bytes(b'tampered')
            with self.assertRaisesRegex(ValueError, 'SHA256 mismatch'):
                cloud.create(root / 'tampered', artifact, manifest, '1.2.3', notices, lock=False)


if __name__ == '__main__':
    unittest.main()

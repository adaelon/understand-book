"""Ensure the rehearsal verifier detects real data loss, not just schema changes."""
import importlib.util
from contextlib import contextmanager
import json
from pathlib import Path
import shutil
import sqlite3
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('upgrade', Path(__file__).with_name('verify-inv-upgrade.py'))
upgrade = importlib.util.module_from_spec(spec)
spec.loader.exec_module(upgrade)


@contextmanager
def database(path):
    connection = sqlite3.connect(path)
    try:
        with connection:
            yield connection
    finally:
        connection.close()


class UpgradeTest(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        root = Path(directory.name)
        self.snapshot, self.candidate = root / 'snapshot', root / 'candidate'
        source = self.snapshot / 'service'
        source.mkdir(parents=True)
        self.candidate.mkdir()
        (self.snapshot / 'snapshot.json').write_text(json.dumps({'metadata': {'source_root': str(source)}}))
        publication = source / 'publications/book'
        publication.mkdir(parents=True)
        (publication / 'publication.json').write_text('{}')
        (source / 'users/legacy').mkdir(parents=True)
        (source / 'users/legacy/note.md').write_text('Original private note')
        with database(source / 'control.sqlite') as db:
            db.executescript('PRAGMA user_version=8; CREATE TABLE users(user_id TEXT, password_hash TEXT);'
                             "INSERT INTO users VALUES('legacy','original');"
                             'CREATE TABLE book_publications(directory TEXT);'
                             'CREATE TABLE allowance_periods(amount INTEGER); INSERT INTO allowance_periods VALUES(123);')
            db.execute('INSERT INTO book_publications VALUES(?)', (str(publication),))
        shutil.copytree(source, self.candidate, dirs_exist_ok=True)
        with database(self.candidate / 'control.sqlite') as db:
            db.executescript('PRAGMA user_version=9; ALTER TABLE users ADD email TEXT; ALTER TABLE users ADD email_verified_at INTEGER;')
            for table in ('invite_batches', 'beta_invites', 'registration_requests', 'email_binding_requests', 'password_reset_requests'):
                db.execute('CREATE TABLE ' + table + '(value TEXT)')
            db.execute('UPDATE book_publications SET directory=?', (str(self.candidate / 'publications/book'),))

    def test_upgrade_preserves_rows_files_and_relocates_publications(self):
        result = upgrade.verify(self.snapshot, self.candidate)
        self.assertEqual(result['status'], 'passed')
        self.assertEqual(result['preserved_private_files'], 1)
        self.assertEqual(result['preserved_control_tables']['allowance_periods'], 1)

    def test_lost_allowance_is_rejected(self):
        with database(self.candidate / 'control.sqlite') as db:
            db.execute('UPDATE allowance_periods SET amount=0')
        with self.assertRaisesRegex(ValueError, 'allowance_periods'):
            upgrade.verify(self.snapshot, self.candidate)

    def test_wal_snapshot_read_does_not_create_sidecar_files(self):
        with database(self.snapshot / 'service/control.sqlite') as db:
            db.execute('PRAGMA journal_mode=WAL')
        before = {p.relative_to(self.snapshot) for p in self.snapshot.rglob('*') if p.is_file()}
        upgrade.verify(self.snapshot, self.candidate)
        after = {p.relative_to(self.snapshot) for p in self.snapshot.rglob('*') if p.is_file()}
        self.assertEqual(after, before)

    def test_private_note_change_is_rejected(self):
        (self.candidate / 'users/legacy/note.md').write_text('Changed')
        with self.assertRaisesRegex(ValueError, 'Restored file differs'):
            upgrade.verify(self.snapshot, self.candidate)


if __name__ == '__main__':
    unittest.main()

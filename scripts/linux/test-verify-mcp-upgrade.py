"""Exercise the release check against loss of existing and new business facts."""
import importlib.util
import json
from pathlib import Path
import shutil
import sqlite3
import tempfile
import unittest
from contextlib import closing

spec = importlib.util.spec_from_file_location('upgrade', Path(__file__).with_name('verify-mcp-upgrade.py'))
upgrade = importlib.util.module_from_spec(spec)
spec.loader.exec_module(upgrade)


class UpgradeTest(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        root = Path(directory.name)
        self.snapshot, self.candidate = root / 'snapshot', root / 'candidate'
        self.source = self.snapshot / 'service'
        publication = self.source / 'publications/book'
        publication.mkdir(parents=True)
        (publication / 'publication.json').write_text('{}')
        (self.source / 'users/legacy').mkdir(parents=True)
        (self.source / 'users/legacy/note.md').write_text('Original private note')
        (self.snapshot / 'snapshot.json').write_text(json.dumps({'metadata': {'source_root': str(self.source)}}))
        with closing(sqlite3.connect(self.source / 'control.sqlite')) as db, db:
            db.executescript('PRAGMA user_version=9; CREATE TABLE users(user_id TEXT, password_hash TEXT);'
                             "INSERT INTO users VALUES('legacy','original');"
                             'CREATE TABLE book_publications(directory TEXT);'
                             'CREATE TABLE manual_receipts(receipt_id TEXT, paid_fen INTEGER);'
                             "INSERT INTO manual_receipts VALUES('existing-payment',1100);"
                             'CREATE TABLE allowance_periods(amount INTEGER); INSERT INTO allowance_periods VALUES(123);')
            db.execute('INSERT INTO book_publications VALUES(?)', (str(publication),))
        shutil.copytree(self.source, self.candidate)
        with closing(sqlite3.connect(self.candidate / 'control.sqlite')) as db, db:
            db.execute('PRAGMA user_version=12')
            db.execute('UPDATE book_publications SET directory=?', (str(self.candidate / 'publications/book'),))
            for table in upgrade.MCP_TABLES:
                db.execute('CREATE TABLE ' + table + '(value TEXT)')

    def test_upgrade_preserves_existing_business_and_private_files(self):
        result = upgrade.verify(self.snapshot, self.candidate)
        self.assertEqual(result['preserved_control_tables']['manual_receipts'], 1)
        self.assertEqual(result['preserved_private_files'], 1)
        self.assertEqual(result['source_schema'], 9)

    def test_lost_model_allowance_is_rejected(self):
        with closing(sqlite3.connect(self.candidate / 'control.sqlite')) as db, db:
            db.execute('UPDATE allowance_periods SET amount=0')
        with self.assertRaisesRegex(ValueError, 'allowance_periods'):
            upgrade.verify(self.snapshot, self.candidate)

    def test_changed_receipt_is_rejected(self):
        with closing(sqlite3.connect(self.candidate / 'control.sqlite')) as db, db:
            db.execute('UPDATE manual_receipts SET paid_fen=0')
        with self.assertRaisesRegex(ValueError, 'manual_receipts'):
            upgrade.verify(self.snapshot, self.candidate)

    def test_schema12_restore_preserves_mcp_and_detects_lost_consumption(self):
        with closing(sqlite3.connect(self.source / 'control.sqlite')) as db, db:
            db.execute('PRAGMA user_version=12')
            for table in upgrade.MCP_TABLES:
                db.execute('CREATE TABLE ' + table + '(value TEXT)')
                db.execute('INSERT INTO ' + table + ' VALUES(?)', ('preserved-business-fact',))
        with closing(sqlite3.connect(self.candidate / 'control.sqlite')) as db, db:
            for table in upgrade.MCP_TABLES:
                db.execute('INSERT INTO ' + table + ' VALUES(?)', ('preserved-business-fact',))
        self.assertEqual(upgrade.verify(self.snapshot, self.candidate)['mcp_table_counts']['mcp_usage'], 1)
        with closing(sqlite3.connect(self.candidate / 'control.sqlite')) as db, db:
            db.execute('DELETE FROM mcp_usage')
        with self.assertRaisesRegex(ValueError, 'mcp_usage'):
            upgrade.verify(self.snapshot, self.candidate)

    def test_private_note_change_is_rejected(self):
        (self.candidate / 'users/legacy/note.md').write_text('Changed')
        with self.assertRaisesRegex(ValueError, 'Restored file differs'):
            upgrade.verify(self.snapshot, self.candidate)


if __name__ == '__main__':
    unittest.main()

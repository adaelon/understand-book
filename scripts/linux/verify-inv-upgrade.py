"""Compare an INV9 restored candidate with its immutable schema-8 snapshot.

Run reader_maintenance restore first. This command only reads both roots and
prints counts; passwords, account details and private contents are never output.
"""
import argparse
from collections import Counter
from contextlib import closing
import filecmp
import json
import os
from pathlib import Path
import sqlite3


def read_db(path):
    # Both roots are offline. Reading a WAL-mode snapshot with mode=ro alone
    # can still create -wal/-shm files, which would invalidate its inventory.
    return closing(sqlite3.connect(path.resolve().as_uri() + '?mode=ro&immutable=1', uri=True))


def quote(name):
    return '"' + name.replace('"', '""') + '"'


def tables(db):
    return [row[0] for row in db.execute(
        "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'")]


def compare_database(before, after, *, source_root=None, candidate=None):
    counts = {}
    for table in tables(before):
        columns = [row[1] for row in before.execute('PRAGMA table_info(' + quote(table) + ')')]
        query = 'SELECT ' + ','.join(map(quote, columns)) + ' FROM ' + quote(table)
        rows = before.execute(query).fetchall()
        if table == 'book_publications' and source_root is not None:
            index = columns.index('directory')
            relocated = []
            for row in rows:
                row = list(row)
                relative = Path(row[index]).relative_to(source_root)
                moved = candidate / relative
                if not (moved / 'publication.json').is_file():
                    raise ValueError('Restored publication is missing')
                row[index] = str(moved)
                relocated.append(tuple(row))
            rows = relocated
        actual = after.execute(query).fetchall()
        # Canonical Windows paths may include the extended-length prefix.
        if table == 'book_publications' and source_root is not None:
            def paths(values):
                return [tuple(os.path.normcase(str(Path(value).resolve()).removeprefix('\\\\?\\')) if i == index else value
                              for i, value in enumerate(row)) for row in values]
            rows, actual = paths(rows), paths(actual)
        if Counter(rows) != Counter(actual):
            raise ValueError('Restored rows differ: ' + table)
        counts[table] = len(rows)
    return counts


def verify(snapshot, candidate):
    snapshot, candidate = snapshot.resolve(), candidate.resolve()
    metadata = json.loads((snapshot / 'snapshot.json').read_text(encoding='utf-8'))['metadata']
    original = snapshot / 'service'
    with read_db(original / 'control.sqlite') as before, read_db(candidate / 'control.sqlite') as after:
        old_version = before.execute('PRAGMA user_version').fetchone()[0]
        new_version = after.execute('PRAGMA user_version').fetchone()[0]
        if (old_version, new_version) != (8, 9):
            raise ValueError(f'Expected schema 8 -> 9, got {old_version} -> {new_version}')
        counts = compare_database(before, after, source_root=Path(metadata['source_root']), candidate=candidate)
        if after.execute('SELECT count(*) FROM users WHERE email IS NOT NULL OR email_verified_at IS NOT NULL').fetchone()[0]:
            raise ValueError('Upgrade unexpectedly bound existing accounts')
        for table in ('invite_batches', 'beta_invites', 'registration_requests', 'email_binding_requests', 'password_reset_requests'):
            if after.execute('SELECT count(*) FROM ' + table).fetchone()[0]:
                raise ValueError('Upgrade unexpectedly created INV facts: ' + table)
    copied = private = databases = 0
    for path in original.rglob('*'):
        if not path.is_file():
            continue
        relative = path.relative_to(original)
        if str(relative) in ('control.sqlite', 'service.lock', 'maintenance.json'):
            continue
        other = candidate / relative
        with path.open('rb') as file:
            is_database = file.read(16) == b'SQLite format 3\0'
        if is_database:
            with read_db(path) as before, read_db(other) as after:
                compare_database(before, after)
            databases += 1
        elif not filecmp.cmp(path, other, shallow=False):
            raise ValueError('Restored file differs: ' + str(relative))
        copied += 1
        private += relative.parts[0] == 'users'
    return {'status': 'passed', 'source_schema': old_version, 'candidate_schema': new_version,
            'preserved_control_tables': counts, 'preserved_files': copied,
            'preserved_private_files': private, 'compared_file_databases': databases,
            'new_inv_tables_empty': True, 'legacy_emails_unbound': True}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('snapshot', type=Path)
    parser.add_argument('candidate', type=Path)
    args = parser.parse_args()
    print(json.dumps(verify(args.snapshot, args.candidate), indent=2))

"""Verify an offline MCP10 upgrade (schema 9 -> 12) or schema-12 restore.

Run reader_maintenance restore first. Compare all old business rows and private
files directly; only published-package root relocation is allowed. No account
details, credentials, receipt contents or private text are printed.
"""
import argparse
import filecmp
import importlib.util
import json
from pathlib import Path

spec = importlib.util.spec_from_file_location('inv_comparison', Path(__file__).with_name('verify-inv-upgrade.py'))
comparison = importlib.util.module_from_spec(spec)
spec.loader.exec_module(comparison)
# The production host's Python links SQLite 3.34; its existing pysqlite3 runtime
# reads the STRICT tables introduced by MCP. Normal newer Python needs no extra package.
if comparison.sqlite3.sqlite_version_info < (3, 37, 0):
    import pysqlite3
    comparison.sqlite3 = pysqlite3

MCP_TABLES = ('mcp_credentials', 'mcp_free_months', 'mcp_free_gifts',
              'mcp_usage', 'mcp_call_packs', 'mcp_pack_adjustments')


def verify(snapshot, candidate):
    snapshot, candidate = snapshot.resolve(), candidate.resolve()
    metadata = json.loads((snapshot / 'snapshot.json').read_text(encoding='utf-8'))['metadata']
    original = snapshot / 'service'
    with comparison.read_db(original / 'control.sqlite') as before, comparison.read_db(candidate / 'control.sqlite') as after:
        old_version = before.execute('PRAGMA user_version').fetchone()[0]
        new_version = after.execute('PRAGMA user_version').fetchone()[0]
        if old_version not in (9, 12) or new_version != 12:
            raise ValueError(f'Expected schema 9 or 12 -> 12, got {old_version} -> {new_version}')
        counts = comparison.compare_database(before, after, source_root=Path(metadata['source_root']), candidate=candidate)
        mcp_counts = {table: after.execute('SELECT count(*) FROM ' + table).fetchone()[0] for table in MCP_TABLES}
        if old_version == 9 and any(mcp_counts.values()):
            raise ValueError('Upgrade unexpectedly created MCP business facts')
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
            with comparison.read_db(path) as before, comparison.read_db(other) as after:
                comparison.compare_database(before, after)
            databases += 1
        elif not filecmp.cmp(path, other, shallow=False):
            raise ValueError('Restored file differs: ' + str(relative))
        copied += 1
        private += relative.parts[0] == 'users'
    return {'status': 'passed', 'source_schema': old_version, 'candidate_schema': new_version,
            'preserved_control_tables': counts, 'preserved_files': copied,
            'preserved_private_files': private, 'compared_file_databases': databases,
            'mcp_table_counts': mcp_counts}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('snapshot', type=Path)
    parser.add_argument('candidate', type=Path)
    args = parser.parse_args()
    print(json.dumps(verify(args.snapshot, args.candidate), indent=2))

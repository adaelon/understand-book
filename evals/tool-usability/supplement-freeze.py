"""Complete compile-time asset capture before any trial starts."""
import io
import json
import zipfile
from pathlib import Path
import subprocess

root = Path(__file__).resolve().parents[2]
out = root / 'docs/performance/tool-usability/to14'
manifest = json.loads((out / 'source-manifest.json').read_text())
paths = ['skills', 'assets', 'packages/web/src',
    'docs/performance/linux-multi-reader-mu0-20260930/routes.csv',
    'docs/performance/presentation-critical-moments-ex14/materials/N3/version.json',
    'docs/performance/ex11-local-demonstrations/ex11.7/batch12/runs/disk-1/delivered-raw.html']
for name, metadata in manifest.items():
    target = root / metadata['source_directory']
    data = subprocess.check_output(['git', 'archive', '--format=zip', metadata['commit'], *paths], cwd=root)
    with zipfile.ZipFile(io.BytesIO(data)) as archive:
        archive.extractall(target)
    metadata['compile_assets'] = paths
(out / 'source-manifest.json').write_text(json.dumps(manifest, ensure_ascii=False, indent=2), encoding='utf8')
print('Saved compile-time skills, presentation assets and generated Web types for both revisions')

"""Save repeatable source inputs without touching the shared checkout or inventing hashes."""
import io
import json
from pathlib import Path
import shutil
import subprocess
import zipfile

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / 'docs/performance/tool-usability/to14'
SNAPSHOTS = OUT / 'snapshots'
BASE = '67efb605f5d45157eeb769f0074b218eacd76dba'
PATHS = ['Cargo.toml', 'Cargo.lock', 'crates', 'apps/desktop/src-tauri', 'packages/core', 'packages/web/src', 'assets', 'skills', 'evals', 'package.json', 'pnpm-lock.yaml', 'pnpm-workspace.yaml', 'tsconfig.json']
PATHS += ['docs/performance/linux-multi-reader-mu0-20260930/routes.csv',
          'docs/performance/presentation-critical-moments-ex14/materials/N3/version.json',
          'docs/performance/ex11-local-demonstrations/ex11.7/batch12/runs/disk-1/delivered-raw.html']

def git(*args):
    return subprocess.check_output(['git', *args], cwd=ROOT)

def freeze():
    SNAPSHOTS.mkdir(parents=True, exist_ok=False)
    tracked = set(git('ls-files', '-z').decode().split('\0'))
    paths = [p for p in PATHS if p in tracked or any(t.startswith(p + '/') for t in tracked)]
    head = git('rev-parse', 'HEAD').decode().strip()
    saved = {}
    for name, revision in [('baseline', BASE), ('candidate', head)]:
        target = SNAPSHOTS / name
        archive = git('archive', '--format=zip', revision, *paths)
        with zipfile.ZipFile(io.BytesIO(archive)) as stream:
            stream.extractall(target)
        changed = []
        if name == 'candidate':
            files = git('ls-files', '-z', '--modified', '--others', '--exclude-standard', '--', *paths).decode().split('\0')
            for file in filter(None, files):
                source = ROOT / file
                if source.is_file():
                    dest = target / file
                    dest.parent.mkdir(parents=True, exist_ok=True)
                    shutil.copy2(source, dest)
                    changed.append(file)
                elif (target / file).is_file():
                    (target / file).unlink()
            (OUT / 'candidate.patch').write_bytes(git('diff', '--', *paths))
        harness = 'crates/server/src/tests/tool_usability_trials.rs'
        shutil.copy2(ROOT / harness, target / harness)
        owner = target / 'crates/server/src/tests/tutor_continuation_tests.rs'
        text = owner.read_text(encoding='utf8')
        if 'mod to14;' not in text:
            owner.write_text(text + '\n#[path = "tool_usability_trials.rs"]\nmod to14;\n', encoding='utf8')
        saved[name] = {'commit': revision, 'modified_or_untracked_inputs': changed, 'test_overlay': harness,
                       'source_directory': str(target.relative_to(ROOT)).replace('\\', '/')}
    (OUT / 'source-manifest.json').write_text(json.dumps(saved, ensure_ascii=False, indent=2), encoding='utf8')
    print(json.dumps({k: len(v['modified_or_untracked_inputs']) for k, v in saved.items()}))

if __name__ == '__main__':
    freeze()

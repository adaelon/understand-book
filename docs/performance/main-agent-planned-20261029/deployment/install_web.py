from pathlib import Path
import json,tarfile
base=Path('/opt/understand-book');stage=base/'acceptance/main-agent-planned-20261029';release=base/'releases/main-agent-planned-20261029'
assert (stage/'build.exit').read_text().strip()=='0'
source_changes=[]
with tarfile.open(stage/'source.tar.gz') as archive:
 for entry in archive.getmembers():
  p=release/entry.name
  assert p.is_relative_to(release) and entry.isfile()
  data=archive.extractfile(entry).read()
  if p.exists() and p.read_bytes()==data:continue
  assert entry.name.startswith(('packages/','apps/')), 'Compiled source changed after the Linux build: '+entry.name
  p.parent.mkdir(parents=True,exist_ok=True)
  if p.exists():p.unlink()
  p.write_bytes(data);source_changes.append(entry.name)
count=0
with tarfile.open(stage/'web-dist.tar.gz') as archive:
 for entry in archive.getmembers():
  assert entry.isfile()
  p=release/'packages/web/dist-multi'/entry.name
  assert p.is_relative_to(release/'packages/web/dist-multi')
  data=archive.extractfile(entry).read()
  p.parent.mkdir(parents=True,exist_ok=True)
  if p.exists():p.unlink()
  p.write_bytes(data);p.chmod(0o644);count+=1
report={'status':'installed','files':count,'source_corrections':source_changes,'previous_assets_preserved':True}
(stage/'web-install.json').write_text(json.dumps(report,indent=2));print(json.dumps(report))

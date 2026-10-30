from pathlib import Path
import json,shutil
base=Path('/opt/understand-book/acceptance/main-agent-planned-20261029');removed=[]
assert json.loads((base/'public-verification.json').read_text())['status']=='passed'
for name in ['candidate-data-4','candidate-data-5']:
 p=(base/name).resolve();assert p.parent==base and p.name==name
 for proc in Path('/proc').glob('[0-9]*/cmdline'):
  try:args=proc.read_bytes().split(b'\0')
  except (OSError,PermissionError):continue
  if args and (args[0].endswith(b'/server') or args[0]==b'runuser'):
   assert str(p).encode() not in args,(proc,p)
 if p.exists():shutil.rmtree(p);removed.append(name)
report={'status':'cleaned','isolated_data_copies_removed':removed,'production_data_removed':False,'free_bytes':shutil.disk_usage(base).free}
(base/'cleanup.json').write_text(json.dumps(report,indent=2));print(json.dumps(report))

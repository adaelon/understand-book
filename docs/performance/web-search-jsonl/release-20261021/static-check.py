from pathlib import Path
from datetime import datetime,timezone
import json,runpy,urllib.request
base=Path('/opt/understand-book');stage=base/'acceptance/search-cache-20261021'
old=base/'releases/cloud-20261021';new=base/'releases/search-cache-20261021'
assets=runpy.run_path(str(base/'acceptance/full-release-20261019/static-check.py'))['verify'](new)
installer=(new/'downloads/UnderstandBookSetup.exe').read_bytes()
assert installer==(old/'downloads/UnderstandBookSetup.exe').read_bytes()
with urllib.request.urlopen(urllib.request.Request('https://understandbook.top/downloads/UnderstandBookSetup.exe',headers={'User-Agent':'UnderstandBook-MCP10-Verification'}),timeout=60) as response:
    assert response.read()==installer
report={'status':'passed','assets':assets,'installer':{'version':'0.3.1','bytes':len(installer),'unchanged_from_previous_release':True,'public_matches_release':True},'verified_at_utc':datetime.now(timezone.utc).isoformat()}
(stage/'static-check.json').write_text(json.dumps(report,indent=2));print(json.dumps(report,indent=2))

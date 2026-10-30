from pathlib import Path
from datetime import datetime,timezone
import json,tarfile
base=Path('/opt/understand-book');stage=base/'acceptance/main-agent-planned-20261029';release=base/'releases/main-agent-planned-20261029'
assert json.loads((stage/'build-completion.json').read_text())['status']=='built'
binary=(release/'target/release/server').read_bytes()
markers=['First teaching principle: assume no prior knowledge of the current concept.',
 'resident-agent.policy.learner-starting-point','resident-agent.teaching.explanation',
 'teaching-design.v30','teaching-quality.v4',
 'Select validates and saves the exact delivery through the host',
 'begin by recording a few concrete outcomes through goal.update operation=working',
 'workspace.notes','create_mind_map',
 'Do not deliver a whole unsolicited next lesson and then change only its numbers for a final quiz.',
 '当前答案完整交付，下一问题留待参与，不在结尾顺带讲完下一课。',
 'create_mind_map','preview_delivery_grace.v1','Board source citations: Substantive claims about the book']
for marker in markers: assert marker.encode() in binary,marker
files=0
with tarfile.open(stage/'web-dist.tar.gz') as archive:
 for entry in archive.getmembers():
  assert (release/'packages/web/dist-multi'/entry.name).read_bytes()==archive.extractfile(entry).read(),entry.name
  files+=1
report={'status':'passed','verified_at_utc':datetime.now(timezone.utc).isoformat(),
 'compiled_teaching_and_feature_markers_present':len(markers),'frontend_files_equal_local_verified_build':files,'model_requests':0}
(stage/'release-verification.json').write_text(json.dumps(report,indent=2));print(json.dumps(report))

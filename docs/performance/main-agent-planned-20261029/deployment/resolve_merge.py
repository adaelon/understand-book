from pathlib import Path
import re,json,subprocess,shutil,tarfile
s=Path('tmp/main-agent-planned-20261029/deployment');pat=r'<<<<<<<[^\n]*\n(.*?)\|\|\|\|\|\|\|[^\n]*\n(.*?)=======\n(.*?)>>>>>>>[^\n]*\n'
for n in json.loads((s/'merge-report.json').read_text())['conflicts']:
 p=s/'merged'/n; i=[0]
 def resolve(m):
  j=i[0];i[0]+=1;online,base,local=m.groups()
  if n in ['crates/runtime/src/board_author.rs','packages/web/src/board-workspace.test.ts']:return online
  if n=='packages/web/src/board-workspace.ts':return online if j<2 else local
  if n=='crates/runtime/src/tool_exposure.rs':return local.replace('board_slot + progress_slot +','board_slot + progress_slot + workspace_notes_slot +')
  if n=='crates/runtime/src/orchestrator.rs':
   if j==3:return online.replace('reviewed native','selected native').replace('reviewed_delivery','selected_delivery').replace('ReviewedDelivery','SelectedDelivery')
   if j==4:return online
   if j==5:return local.replace('.without_workspace_records()','.without_workspace_records().without_workspace_notes()')
  if n=='crates/server/src/board_author.rs':return online.replace('commit_review','commit_selection').replace('exact review','exact selection')
  return local
 p.write_text(re.sub(pat,resolve,p.read_text(encoding='utf-8'),flags=re.S),encoding='utf-8')
# Keep live-only board editing/confirmation paths consistent with the renamed selection contract.
for n in ['board_author_confirmation.rs','board_author_edits.rs','board_author_edit_tests.rs','board_conversation.rs']:
 name='crates/server/src/'+n;p=s/'merged'/name;p.parent.mkdir(parents=True,exist_ok=True)
 t=(s/'online'/name).read_text(encoding='utf-8').replace('commit_review','commit_selection').replace('ReviewedDelivery','SelectedDelivery').replace('reviewed_delivery','selected_delivery')
 p.write_text(t,encoding='utf-8')
# Server tests from the already tested MA patch; these do not contain live-only edits.
paths=subprocess.check_output(['git','diff','--name-only','HEAD','--','crates/server/src']).decode().splitlines()
for n in paths:
 if 'test' in n:
  p=s/'merged'/n;p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(Path(n).read_bytes())
# Build source overlay only; no unrelated source replacement.
files=sorted(p.relative_to(s/'merged').as_posix() for p in (s/'merged').rglob('*') if p.is_file())
for n in files:assert '<<<<<<<' not in (s/'merged'/n).read_text(encoding='utf-8'),n
with tarfile.open(s/'source.tar.gz','w:gz') as t:
 for n in files:t.add(s/'merged'/n,arcname=n)
manifest={'release':'main-agent-planned-20261029','previous_release':'reader-outline-20261029','files':files,'removed':[],
 'source_commit':subprocess.check_output(['git','rev-parse','HEAD']).decode().strip(),
 'selection':'Actual online reader-outline source plus MA0-MA5 and required teaching revision/quality dependencies; live mind maps, shared notes, outline, titles, monitoring and account behavior preserved. Conflict decisions retained in resolve_merge.py.'}
(s/'release-source.json').write_text(json.dumps(manifest,indent=2))
print('source files',len(files))

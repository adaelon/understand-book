from pathlib import Path
import json,os,runpy,secrets,signal,subprocess,time,urllib.error
base=Path('/opt/understand-book');stage=base/'acceptance/board-source-window-20261028'
release=base/'releases/editable-mind-map-20261028';root=stage/'candidate-data-3'
active=json.loads(subprocess.check_output([str(base/'tools/node-v24.20.0-linux-x64/bin/node'),'--disable-warning=ExperimentalWarning','--input-type=module','-e',"import {DatabaseSync} from 'node:sqlite';const d=new DatabaseSync(process.argv[1],{readOnly:true});console.log(JSON.stringify(d.prepare('SELECT user_id FROM users WHERE disabled=0').all()));d.close()",str(root/'control.sqlite')],text=True))
active={row['user_id'] for row in active}
found=None
for file in (root/'users').glob('*/memory/boards/**/*.json'):
 if file.relative_to(root/'users').parts[0] not in active:continue
 doc=json.loads(file.read_text())
 if not isinstance(doc,dict) or not doc.get('publication_id'):continue
 for item in doc.get('items',[]):
  for binding in item.get('source_bindings',[]):
   if binding.get('book_id')=='ai-infra-book-complete' and not binding.get('publication_id'):
    found=(file.relative_to(root/'users').parts[0],doc,item,binding);break
  if found:break
 if found:break
assert found
owner,doc,item,binding=found;password=secrets.token_urlsafe(24)
subprocess.run([str(release/'target/release/manage_reader'),str(root),'password',owner],input=password+'\n',text=True,check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
Client=runpy.run_path(str(release/'scripts/linux/smoke-mcp-release.py'))['Client']
config={'origin':'https://understandbook.top','transport_url':'http://127.0.0.1:8884'}
env={k:v for k,v in os.environ.items() if not k.startswith(('UNDERSTAND_BOOK_','OPENCODE_','FLUID_','UB_','OPENAI_','DEEPSEEK_'))}
env.update(UNDERSTAND_BOOK_NODE=str(base/'tools/node-v24.20.0-linux-x64/bin/node'),UNDERSTAND_BOOK_WEB_DIST=str(release/'packages/web/dist-multi'),UB_OBSERVABILITY_MODE='off',OPENCODE_BASE_URL='http://127.0.0.1:9',OPENCODE_API_KEY='isolated-no-model')
with (stage/'legacy-source-server.log').open('w') as log:
 process=subprocess.Popen(['runuser','-u','understand-book','--',str(release/'target/release/server'),'--multi-user',str(root),'--origin',config['origin'],'--addr','127.0.0.1:8884'],cwd=release,env=env,stdout=log,stderr=log,start_new_session=True)
 try:
  for attempt in range(30):
   try:Client(config).request('/api/auth/me',expected=401);break
   except urllib.error.URLError:
    if process.poll() is not None or attempt==29:raise
    time.sleep(.5)
  client=Client(config);client.login({'username':owner,'password':password})
  try:
   scene=client.json('/api/workspaces',{'published_book_ref':{'book_id':doc['book_id'],'publication_id':doc['publication_id']},'learning_workspace_id':doc.get('learning_workspace_id'),'attachment_id':'legacy-source-qa'})
   request={'generation':scene['generation'],'expected_revision':scene['revision'],'attachment_id':'legacy-source-qa','board_id':doc['board_id'],'item_id':item['item_id'],'source_ref_id':binding['source_ref_id']}
   result=json.loads(client.request('/api/workspaces/'+scene['workspace_id']+'/boards/source.resolve',request,expected=404))
   assert result['error_code']=='OBJECT_NOT_FOUND'
   report={'status':'confirmed_preexisting','old_release':str(release),'book_id':binding['book_id'],'saved_publication_id':None,'old_response':result,'new_response_matches':True,'production_mutated':False}
   (stage/'legacy-source-verification.json').write_text(json.dumps(report,indent=2));print(json.dumps(report))
  finally:client.json('/api/auth/logout',{})
 finally:
  if process.poll() is None:os.killpg(process.pid,signal.SIGTERM);process.wait(timeout=20)

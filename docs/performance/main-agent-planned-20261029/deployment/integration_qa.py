"""Read existing accounts and boards using the final binary on an isolated data copy."""
from pathlib import Path
from datetime import datetime,timezone
import json,os,runpy,secrets,shutil,signal,subprocess,time,urllib.error,urllib.parse
base=Path('/opt/understand-book');stage=base/'acceptance/main-agent-planned-20261029';release=base/'releases/main-agent-planned-20261029'
production=base/'data/multi-reader-to-tb-20261012';root=stage/'candidate-data-5';node=base/'tools/node-v24.20.0-linux-x64/bin/node'
run=lambda *args:subprocess.check_output(list(map(str,args)),text=True).strip()
assert json.loads((stage/'build-completion.json').read_text())['status']=='built'
state=runpy.run_path(str(base/'acceptance/capacity-fixes-20261027/ops.py'))['state']()
assert state['schema']==21 and all(state[k]==0 for k in ['active','model_inflight','service_inflight','web_inflight','background_inflight']),state
assert not root.exists();root.mkdir(mode=0o700);stage.chmod(0o711)
backup_js="import {DatabaseSync} from 'node:sqlite';const d=new DatabaseSync(process.argv[1],{readOnly:true});d.prepare('VACUUM INTO ?').run(process.argv[2]);d.close()"
def copy_private(source,dest):
 if str(source).endswith(('.sqlite','.db')):run(node,'--disable-warning=ExperimentalWarning','--input-type=module','-e',backup_js,source,dest)
 else:shutil.copy2(source,dest)
for entry in production.iterdir():
 if entry.name in ['service.lock','publish.sock','shared-cache'] or entry.name.endswith(('-wal','-shm')):continue
 target=root/entry.name
 if entry.name=='library':shutil.copytree(entry,target,copy_function=os.link)
 elif entry.is_dir():shutil.copytree(entry,target,copy_function=copy_private,ignore=lambda p,names:[n for n in names if n.endswith(('-wal','-shm'))])
 elif entry.is_file():copy_private(entry,target)
sandbox=root/'presentation-sandbox.json'
if sandbox.exists():
 settings=json.loads(sandbox.read_text())
 settings['instance']='main-agent-planned-qa-20261029'
 settings['worker']=str(release/'target/release/presentation_worker')
 sandbox.write_text(json.dumps(settings));sandbox.chmod(0o600)
def query(sql):
 js="import {DatabaseSync} from 'node:sqlite';const d=new DatabaseSync(process.argv[1],{readOnly:true});console.log(JSON.stringify(d.prepare(process.argv[2]).all()));d.close()"
 return json.loads(run(node,'--disable-warning=ExperimentalWarning','--input-type=module','-e',js,root/'control.sqlite',sql))
before_histories={p:p.read_bytes() for p in (root/'users').glob('*/memory/agent-sessions/*.jsonl')}
before_boards={p:p.read_bytes() for p in (root/'users').glob('*/memory/boards/**/*.json')}
old_periods=query('SELECT * FROM allowance_periods ORDER BY period_id')
calls_before=query('SELECT count(*) n FROM model_call_charges')[0]['n']
credentials=json.loads((base/'multi-reader-credentials.json').read_text());credentials['admin']=json.loads((base/'adm10-admin-credentials.json').read_text())['password']
# Use the existing board owner only in the copied data. No production password is changed.
board_files=list((root/'users').glob('*/memory/boards/workspaces/*/*/head.json'))
if not board_files:board_files=list((root/'users').glob('*/memory/boards/**/*.json'))
owners=sorted({p.relative_to(root/'users').parts[0] for p in board_files})
active_owners={row['user_id'] for row in query('SELECT user_id FROM users WHERE disabled=0')}
disabled_board_owners=len(set(owners)-active_owners)
owners=[owner for owner in owners if owner in active_owners]
for owner in owners:
 if owner not in ['reader','admin']:
  password=secrets.token_urlsafe(24)
  subprocess.run([str(release/'target/release/manage_reader'),str(root),'password',owner],input=password+'\n',text=True,check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
  credentials[owner]=password
run('chown','understand-book:understand-book',root)
for entry in root.iterdir():
 if entry.name!='library':run('chown','-R','understand-book:understand-book',entry)
for directory,_,_ in os.walk(root/'library'):shutil.chown(directory,user='understand-book',group='understand-book')
Client=runpy.run_path(str(release/'scripts/linux/smoke-mcp-release.py'))['Client'];config={'origin':'https://understandbook.top','transport_url':'http://127.0.0.1:8883'}
env={k:v for k,v in os.environ.items() if not k.startswith(('UNDERSTAND_BOOK_','OPENCODE_','FLUID_','UB_','OPENAI_','DEEPSEEK_'))}
env.update(UNDERSTAND_BOOK_MODE='multi-user',UNDERSTAND_BOOK_SERVICE_ROOT=str(root),UNDERSTAND_BOOK_ORIGIN=config['origin'],UNDERSTAND_BOOK_ADDR='127.0.0.1:8883',UNDERSTAND_BOOK_WEB_DIST=str(release/'packages/web/dist-multi'),UNDERSTAND_BOOK_NODE=str(node),UB_OBSERVABILITY_MODE='off',OPENCODE_BASE_URL='http://127.0.0.1:9',OPENCODE_API_KEY='isolated-no-model')
checks={};board_count=0;source_windows=0;source_publications=set();source_navigation=0;legacy_sources=set()
with (stage/'integration-server.log').open('w') as log:
 process=subprocess.Popen(['runuser','-u','understand-book','--',str(release/'target/release/server'),'--multi-user',str(root),'--origin',config['origin'],'--addr','127.0.0.1:8883'],cwd=release,env=env,stdout=log,stderr=log,start_new_session=True)
 try:
  for attempt in range(40):
   try:Client(config).request('/api/auth/me',expected=401);break
   except urllib.error.URLError:
    if process.poll() is not None or attempt==39:raise
    time.sleep(.5)
  for username in sorted(set(['reader','admin',*owners])):
   client=Client(config);client.login({'username':username,'password':credentials[username]})
   try:
    books=client.json('/api/library')['books'];assert books
    ref=next((b for b in books if b.get('is_default')),books[0])['published_book_ref']
    path='/api/books/'+ref['book_id']+'/publications/'+ref['publication_id']
    manifest=client.json(path+'/manifest');leaf=next(n for n in manifest['tree'] if not n.get('children'))
    assert client.json(path+'/text?'+urllib.parse.urlencode({'lid':leaf['lid']}))['text']
    scopes=client.json('/api/learning-workspaces')['workspaces'];client.json('/api/me/chats');client.json('/api/account/allowance')
    for scope in scopes:
     scope_id=scope['learning_workspace_id']
     client.json('/api/learning-workspaces/'+scope_id+'/notes/list',{})
    # Restore each saved board through the public workspace API, using copied data only.
    seen=set()
    for file in (root/'users'/username/'memory/boards').glob('**/*.json'):
     doc=json.loads(file.read_text())
     if not isinstance(doc,dict) or 'items' not in doc or 'board_id' not in doc:continue
     key=(doc['board_id'],doc.get('learning_workspace_id'))
     if key in seen:continue
     seen.add(key)
     if not doc.get('publication_id'):continue
     scene=client.json('/api/workspaces',{'published_book_ref':{'book_id':doc['book_id'],'publication_id':doc['publication_id']},'learning_workspace_id':doc.get('learning_workspace_id'),'attachment_id':'board-visual-release-qa'})
     data={'generation':scene['generation'],'expected_revision':scene['revision'],'attachment_id':'board-visual-release-qa','board_id':doc['board_id']}
     current=client.json('/api/workspaces/'+scene['workspace_id']+'/boards/read',data)['result']['document']
     assert current['board_id']==doc['board_id'] and current['revision']>=doc['revision']
     board_count+=1
     for item in current['items']:
      for binding in item.get('source_bindings',[]):
       source_key=(binding.get('book_id'),binding.get('publication_id'))
       if source_key in source_publications or source_windows>=5:continue
       request={**data,'item_id':item['item_id'],'source_ref_id':binding['source_ref_id']}
       endpoint='/api/workspaces/'+scene['workspace_id']+'/boards/'
       if binding.get('publication_id') is None:
        assert json.loads((base/'acceptance/board-source-window-20261028/legacy-source-verification.json').read_text())['status']=='confirmed_preexisting'
        if binding['source_ref_id'] not in legacy_sources:
         detail=json.loads(client.request(endpoint+'source.resolve',request,expected=404))
         assert detail['error_code']=='OBJECT_NOT_FOUND'
         legacy_sources.add(binding['source_ref_id'])
        continue
       try:resolved=client.json(endpoint+'source.resolve',request)['result']
       except AssertionError:
        detail=json.loads(client.request(endpoint+'source.resolve',request,expected=404))
        print(json.dumps({'unavailable_saved_source':source_key,'current_board_publication':[doc['book_id'],doc['publication_id']],'listed_publication':any(b['published_book_ref']=={'book_id':source_key[0],'publication_id':source_key[1]} for b in books),'response':detail}),flush=True)
        raise
       if resolved.get('stale') or resolved.get('media') or resolved.get('music') or not resolved.get('can_open_in_reader'):continue
       first=client.json(endpoint+'source.read',request)
       assert first['reader']==scene['reader'] and first['revision']==scene['revision']
       window=first['result'];assert window['segments'] and window['outline']
       assert any(segment['highlights'] for segment in window['segments'])
       first_lids={segment['lid'] for segment in window['segments']}
       for direction,cursor in [('before',window['previous_lid']),('after',window['next_lid'])]:
        if cursor:
         page=client.json(endpoint+'source.read',{**request,'anchor_lid':cursor,'direction':direction,'include_outline':False})
         assert page['reader']==scene['reader'] and page['revision']==scene['revision']
         assert page['result']['segments'] and not first_lids.intersection(segment['lid'] for segment in page['result']['segments'])
       chapter=client.json(endpoint+'source.read',{**request,'anchor_lid':window['outline'][-1]['lid']})
       assert chapter['reader']==scene['reader'] and chapter['revision']==scene['revision']
       lid=chapter['result']['focus_lid']
       opened=client.json(endpoint+'source.open',{**request,'lid':lid})
       if source_key==(doc['book_id'],doc['publication_id']):
        assert opened['reader']['selection']==lid
       else:
        assert urllib.parse.parse_qs(urllib.parse.urlparse(opened['result']['source_url']).query)['start_lid']==[lid]
       source_navigation+=1;source_windows+=1;source_publications.add(source_key)
       scene=opened;data.update(generation=scene['generation'],expected_revision=scene['revision'])
       break
   finally:client.json('/api/auth/logout',{})
  checks['existing_accounts_books_history_notes_and_boards_readable']=True
  assert source_windows>0,'No current text citation exercised'
  checks['source_windows_continuous_read_chapters_highlights_without_reader_mutation']=source_windows
  checks['source_open_targets_current_reading_position']=source_navigation
  checks['preexisting_sources_without_publication_remain_unavailable']=len(legacy_sources)
  reader_client=Client(config);admin_client=Client(config)
  reader_client.login({'username':'reader','password':credentials['reader']})
  admin_client.login({'username':'admin','password':credentials['admin']})
  try:
   Client(config).request('/api/admin/monitor',expected=401)
   reader_client.request('/api/admin/monitor',expected=403)
   reader_client.json('/api/account/presence',{})
   reader_client.json('/api/account/presence',{})
   admin_client.json('/api/account/presence',{})
   first=admin_client.json('/api/admin/monitor')
   assert first['current']['activity']['online_users']==2
   assert first['current']['memory']['total_bytes']>0 and first['current']['disk']['total_bytes']>0
   time.sleep(5.2)
   second=admin_client.json('/api/admin/monitor')
   assert second['current']['network']['receive_bytes_per_second'] is not None
   assert second['current']['network']['transmit_bytes_per_second'] is not None
   assert len(second['history'])==2
   checks['linux_monitor_memory_disk_network_presence_and_authorization']=True
   books=reader_client.json('/api/library')['books']
   ref=next(b for b in books if b.get('is_default'))['published_book_ref']
   scene=reader_client.json('/api/workspaces',{'published_book_ref':ref,'attachment_id':'combined-release-mind-map-qa'})
   def board_action(action,payload):
    global scene
    reply=reader_client.json('/api/workspaces/'+scene['workspace_id']+'/boards/'+action,
     {**payload,'generation':scene['generation'],'expected_revision':scene['revision'],'attachment_id':'combined-release-mind-map-qa'})
    scene=reply
    return reply['result']
   board=board_action('create',{'title':'发布验收 · 可编辑思维导图'})
   board=board_action('edit',{'board_id':board['board_id'],'board_revision':board['revision'],'action':'创建主题树','operations':[
    {'kind':'create_mind_map','map':{'map_id':'release-map','root_item_id':'release-root','layout':'balanced','anchor':{'x':400,'y':200}},'root':{'item_id':'release-root','title':'Rust 切片'}},
    {'kind':'insert_topic','parent_item_id':'release-root','index':0,'topic':{'item_id':'release-child','title':'借用一段连续元素'}}]})
   assert len(board['mind_maps'])==1 and len(board['items'])==2
   old_revision=board['revision']
   board=board_action('edit',{'board_id':board['board_id'],'board_revision':board['revision'],'action':'人工改名','operations':[
    {'kind':'update_topic','item_id':'release-child','title':'保留人工编辑'}]})
   reader_client.request('/api/workspaces/'+scene['workspace_id']+'/boards/edit',
    {'generation':scene['generation'],'expected_revision':scene['revision'],'attachment_id':'combined-release-mind-map-qa',
     'board_id':board['board_id'],'board_revision':old_revision,'action':'旧窗口覆盖','operations':[{'kind':'update_topic','item_id':'release-child','title':'旧标题'}]},expected=409)
   board=board_action('undo',{'board_id':board['board_id'],'board_revision':board['revision']})
   assert next(i for i in board['items'] if i['item_id']=='release-child')['title']=='借用一段连续元素'
   board=board_action('redo',{'board_id':board['board_id'],'board_revision':board['revision']})
   restored=board_action('read',{'board_id':board['board_id']})['document']
   assert next(i for i in restored['items'] if i['item_id']=='release-child')['title']=='保留人工编辑'
   checks['multi_user_mind_map_create_edit_stale_conflict_undo_redo_read']=True
   (stage/'candidate-board.json').write_text(json.dumps({'board_id':board['board_id'],'workspace_id':scene['workspace_id'],'book':ref}))
  finally:
   reader_client.json('/api/auth/logout',{});admin_client.json('/api/auth/logout',{})
  assert query('SELECT count(*) n FROM model_call_charges')[0]['n']==calls_before
 finally:
  if process.poll() is None:
   os.killpg(process.pid,signal.SIGTERM)
   try:process.wait(timeout=20)
   except subprocess.TimeoutExpired:os.killpg(process.pid,signal.SIGKILL);process.wait();raise
for file,data in before_histories.items():assert file.read_bytes()==data,'Historical chat changed'
for file,data in before_boards.items():assert file.read_bytes()==data,'Saved board changed'
assert query('SELECT * FROM allowance_periods ORDER BY period_id')==old_periods
assert query('PRAGMA user_version')[0]['user_version']==21
checks.update(historical_chat_files_preserved=len(before_histories),board_files_preserved=len(before_boards),boards_opened=board_count,allowance_rows_preserved=len(old_periods),accounts_checked=len(set(['reader','admin',*owners])),disabled_board_owners_preserved_without_login=disabled_board_owners)
binary=release/'target/release/server'
report={'status':'passed','verified_at_utc':datetime.now(timezone.utc).isoformat(),'schema':21,'checks':checks,'binary_stat':{'size':binary.stat().st_size,'mtime_ns':binary.stat().st_mtime_ns},'production_mutated':False,'model_requests':0}
(stage/'integration-qa.json').write_text(json.dumps(report,indent=2));print(json.dumps(report))

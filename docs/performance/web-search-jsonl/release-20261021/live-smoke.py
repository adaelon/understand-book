from pathlib import Path
from datetime import datetime,timezone
import json,runpy,secrets,time,uuid,urllib.request,urllib.error

base=Path('/opt/understand-book');stage=base/'acceptance/search-cache-20261021';release=base/'releases/search-cache-20261021'
Client=runpy.run_path(str(release/'scripts/linux/smoke-mcp-release.py'))['Client']
admin=Client({'origin':'https://understandbook.top'})
password=json.loads((base/'adm10-admin-credentials.json').read_text())['password']
admin.login({'username':'admin','password':password})
owner='search-release-20261021';private=stage/'private';private.mkdir(mode=0o700,exist_ok=True)
credentials={'username':owner,'password':secrets.token_urlsafe(36)}
resume=(private/'smoke-account.json').exists()
if resume:
    previous=json.loads((stage/'live-smoke.json').read_text())
    assert previous['status']=='failed' and not previous.get('turn_id') and not previous['charges'],'Preserve prior paid run; inspect it before another request'
    credentials=json.loads((private/'smoke-account.json').read_text())
    index=1
    while (stage/f'live-smoke-attempt{index}.json').exists():index+=1
    (stage/'live-smoke.json').rename(stage/f'live-smoke-attempt{index}.json')
else:
    (private/'smoke-account.json').write_text(json.dumps(credentials));(private/'smoke-account.json').chmod(0o600)
reader=Client({'origin':'https://understandbook.top'})
result={'status':'preparing','started_at_utc':datetime.now(timezone.utc).isoformat(),'user_id':owner,'allowance_micro_cny':4_000_000,'turn':None,'charges':[]}
def save():
    (stage/'live-smoke.json').write_text(json.dumps(result,ensure_ascii=False,indent=2))
def charges():return admin.json('/api/admin/charges?user_id='+owner+'&limit=100')['items']
run_id=None
try:
    if not resume:
        admin.json('/api/admin/users',{'operation_id':'search-release-create-20261021','user_id':owner,'password':credentials['password']})
        now=int(time.time())
        period=admin.json('/api/admin/users/'+owner+'/allowance-periods',{'operation_id':'search-release-period-20261021','starts_at':now-1,'expires_at':now+3600,'basis':{'version':'search-release-20261021','basis_micro_cny':4_000_000}})
        admin.json('/api/admin/users/'+owner+'/allowance-adjustments',{'operation_id':'search-release-budget-20261021','period_id':period['allowance_period']['period_id'],'revision':0,'delta_micro_cny':4_000_000,'reason':'Authorized Search/Fetch production release verification; dedicated account, maximum CNY 4.'})
        library=admin.json('/api/library')['books'];selected=max((book for book in library if book['published_book_ref']['book_id']=='understandbook'),key=lambda book:book['published_book_ref']['publication_id']);reference=selected['published_book_ref']
        admin.json('/api/admin/users/'+owner+'/book-grants',{'operation_id':'search-release-book-20261021','published_book_ref':reference})
    else:
        assert not charges()
        library=admin.json('/api/library')['books'];selected=max((book for book in library if book['published_book_ref']['book_id']=='understandbook'),key=lambda book:book['published_book_ref']['publication_id']);reference=selected['published_book_ref']
        admin.json('/api/admin/users/'+owner+'/book-grants',{'operation_id':'search-release-understandbook-20261021','published_book_ref':reference})
    reader.login(credentials)
    attachment=str(uuid.uuid4())
    if resume:
        attempts=[json.loads(p.read_text()) for p in sorted(stage.glob('live-smoke-attempt*.json'),key=lambda p:p.stat().st_mtime)]
        existing=next(r['workspace_id'] for r in reversed(attempts) if r.get('workspace_id'))
        scene=reader.json('/api/workspaces/'+existing)
        scene=reader.json('/api/workspaces/'+existing+'/takeover',{'attachment_id':attachment,'generation':scene['generation'],'expected_revision':scene['revision']})
        scene=reader.json('/api/workspaces/'+existing+'/book/open',{'attachment_id':attachment,'generation':scene['generation'],'expected_revision':scene['revision'],'published_book_ref':reference})
    else:
        scene=reader.json('/api/workspaces',{'attachment_id':attachment,'published_book_ref':reference})
    def stamp():return {'attachment_id':attachment,'generation':scene['generation'],'expected_revision':scene['revision']}
    scene=reader.json('/api/workspaces/'+scene['workspace_id']+'/chat/new',stamp())
    session=scene['selected_chat']
    question='这是一项已授权的上线验收。请先发现联网工具，再实际调用一次 web.search 查询 DeepSeek Search Fetch 官方 API 文档，设置 need_content=false、refresh=true；然后实际调用一次 web.fetch，设置 refresh=true，读取 https://api-docs.deepseek.com/zh-cn/guides/search/ 的正文。最后用不超过三句话说明 Search 与 Fetch 的区别并给出来源链接。无需查书、创建笔记或演示。'
    result.update(status='running',question=question,workspace_id=scene['workspace_id'],session_id=session,reference=reference);save()
    payload={**stamp(),'session_id':session,'published_book_ref':reference,'client_request_id':'search-cache-release-20261021','expected_chat_head':None,'message':question}
    result['admission_payload']=payload;save()
    request=urllib.request.Request(reader.transport+'/api/workspaces/'+scene['workspace_id']+'/agent/runs',data=json.dumps(payload).encode(),headers={'Origin':reader.origin,'Cookie':reader.cookie,'X-CSRF-Token':reader.csrf,'Content-Type':'application/json','User-Agent':'UnderstandBook-MCP10-Verification'})
    try:
        with urllib.request.urlopen(request,timeout=60) as response:admission=json.loads(response.read())
    except urllib.error.HTTPError as error:
        raise RuntimeError('Run admission HTTP '+str(error.code)+': '+error.read().decode()) from None
    run_id=admission['turn_id'];result['turn_id']=run_id;save()
    deadline=time.monotonic()+300
    while time.monotonic()<deadline:
        status=reader.json('/api/agent/runs/'+run_id)
        if status['dispatch_state'] in ['settled','admission_failed']:
            result['turn']=status.get('turn');result['dispatch_state']=status['dispatch_state'];save();break
        time.sleep(1)
    else:
        reader.json('/api/agent/runs/'+run_id+'/cancel',{});raise RuntimeError('Live verification timed out; cancellation requested')
    deadline=time.monotonic()+60;quiet_since=None;previous=None
    while time.monotonic()<deadline:
        records=charges();signature=[(r['call_id'],r['state'],r.get('provider_cost_micro_cny')) for r in records]
        active=[r for r in records if r['state'] in ['reserved','sent']]
        if active or signature!=previous:quiet_since=None
        elif quiet_since is None:quiet_since=time.monotonic()
        elif time.monotonic()-quiet_since>=3:break
        previous=signature;time.sleep(1)
    result['charges']=records
    result['charge_details']=[admin.json('/api/admin/charges/'+r['call_id']) for r in records]
    result['cost_summary']=admin.json('/api/admin/cost-summary?user_id='+owner)
    result['reader_cost_summary']=reader.json('/api/account/cost-summary')
    result['allowance']=reader.json('/api/account/allowance')
    categories={kind:[r for r in records if (r.get('entry_kind') or 'model')==kind] for kind in ['model','web_search','web_fetch']}
    result['costs']={kind:{'requests':len(rows),'confirmed_cost_micro_cny':sum(r.get('provider_cost_micro_cny') or 0 for r in rows),'unknown':sum(r.get('provider_cost_micro_cny') is None for r in rows),'states':[r['state'] for r in rows]} for kind,rows in categories.items()}
    result['total_provider_cost_micro_cny']=sum(r.get('provider_cost_micro_cny') or 0 for r in records)
    trace=result['turn'].get('outcome',{}).get('trace',[])
    result['tools']=[step['tool'] for step in trace]
    assert result['turn']['status']=='completed',result['turn'].get('error')
    assert all(categories.values()),'Model/Search/Fetch must each have real physical request ledger entries'
    assert all(r['state'] in ['settled','released'] and r.get('provider_cost_micro_cny') is not None for r in records),'Unknown or in-flight costs remain'
    for kind in ['web_search','web_fetch']:
        assert any(r['state']=='settled' and r['provider_cost_micro_cny']==5000 for r in categories[kind]),'Expected successful paid '+kind
        assert all(r.get('run_ref')==run_id and r['user_id']==owner for r in categories[kind])
    for kind,rows in categories.items():
        summary=result['cost_summary']['account']['cost_breakdown'][kind]
        assert summary['confirmed_cost_micro_cny']==result['costs'][kind]['confirmed_cost_micro_cny'],kind+' summary differs'
    assert result['total_provider_cost_micro_cny']<=4_000_000
    result['status']='passed'
except BaseException as error:
    result['status']='failed';result['error']=str(error)
    raise
finally:
    try:
        result['charges']=charges()
        result['cost_summary']=admin.json('/api/admin/cost-summary?user_id='+owner)
    except Exception as error:result['final_accounting_error']=str(error)
    result['finished_at_utc']=datetime.now(timezone.utc).isoformat();save()
    print(json.dumps({key:result.get(key) for key in ['status','user_id','turn_id','tools','costs','total_provider_cost_micro_cny','error']},ensure_ascii=False,indent=2))
    try:reader.json('/api/auth/logout',{})
    except Exception:pass
    admin.json('/api/auth/logout',{})

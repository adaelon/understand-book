from pathlib import Path
from datetime import datetime,timezone
import json,os,runpy,shutil,subprocess,time
base=Path('/opt/understand-book');stage=base/'acceptance/board-source-window-20261028'
old=base/'releases/editable-mind-map-20261028';new=base/'releases/board-source-window-20261028'
root=base/'data/multi-reader-to-tb-20261012';backup=base/'backups/board-source-window-20261028'
service='understand-book-multi';unit=Path('/etc/systemd/system/understand-book-multi.service')
nginx=Path('/etc/nginx/conf.d/understand-book-multi.conf');envfile=base/'multi-reader.env';sandbox=root/'presentation-sandbox.json'
run=lambda *args:subprocess.check_output(list(map(str,args)),text=True).strip()
ops=runpy.run_path(str(base/'acceptance/capacity-fixes-20261027/ops.py'))
ready=runpy.run_path(str(stage/'health.py'))['ready']
node=base/'tools/node-v24.20.0-linux-x64/bin/node'
def state():
    value=ops['state']();value['review_running']=0
    for file in (root/'users').glob('*/memory/memory.json'):
        value['review_running']+=sum(j['status']=='running' for j in json.loads(file.read_text()).get('review_state',{}).get('review_jobs',[]))
    return value
def idle(value):assert value['schema']==21 and all(value[k]==0 for k in ['active','model_inflight','service_inflight','web_inflight','background_inflight','review_running']),value
def periods():
    code="import {DatabaseSync} from 'node:sqlite';const d=new DatabaseSync(process.argv[1],{readOnly:true});console.log(JSON.stringify(d.prepare('SELECT * FROM allowance_periods ORDER BY period_id').all()));d.close()"
    return json.loads(run(node,'--disable-warning=ExperimentalWarning','--input-type=module','-e',code,root/'control.sqlite'))
assert not (stage/'deployment.json').exists()
assert json.loads((stage/'build-completion.json').read_text())['status']=='built'
qa=json.loads((stage/'integration-qa.json').read_text());assert qa['status']=='passed' and qa['schema']==21
assert json.loads((stage/'release-verification.json').read_text())['status']=='passed'
binary=new/'target/release/server';assert qa['binary_stat']=={'size':binary.stat().st_size,'mtime_ns':binary.stat().st_mtime_ns}
assert run('systemctl','show',service,'-p','WorkingDirectory','--value')==str(old)
before=state();idle(before);health_before=ready('http://127.0.0.1:8788')
original={p:p.read_bytes() for p in [unit,nginx,envfile,sandbox]}
mutable_bytes=int(run('du','-sb','--exclude=library/published','--exclude=shared-cache',root).split()[0]);assert shutil.disk_usage(base).free>mutable_bytes+128*1024**2
backup.mkdir(mode=0o700,exist_ok=False)
for p,data in original.items():saved=backup/p.name;saved.write_bytes(data);saved.chmod(0o600)
maintenance=original[nginx].decode().replace('    server_name understandbook.top;','    server_name understandbook.top;\n    return 503;',1)
published=original[nginx].replace(str(old).encode(),str(new).encode());assert maintenance!=original[nginx].decode() and published!=original[nginx]
started=time.monotonic();stopped=False;exposed=False
try:
    nginx.write_text(maintenance);run('nginx','-t');run('systemctl','reload','nginx');time.sleep(.5);idle(state())
    run('systemctl','stop',service);stopped=True;idle(state());old_periods=periods()
    histories={p:p.read_bytes() for p in (root/'users').glob('*/memory/agent-sessions/*.jsonl')}
    boards={p:p.read_bytes() for p in (root/'users').glob('*/memory/boards/**/*.json')}
    run('tar','--exclude=./library/published','--exclude=./shared-cache','--exclude=./publish.sock','--exclude=./service.lock','-C',root,'-czf',backup/'mutable.tar.gz','.')
    (backup/'mutable.tar.gz').chmod(0o600)
    for p in [unit,envfile]:p.write_bytes(original[p].replace(str(old).encode(),str(new).encode()))
    envfile.chmod(0o600);settings=json.loads(original[sandbox]);settings['worker']=str(new/'target/release/presentation_worker');sandbox.write_text(json.dumps(settings))
    run('systemctl','daemon-reload');run('systemctl','start',service)
    assert ready('http://127.0.0.1:8788')==health_before
    pid=run('systemctl','show',service,'-p','MainPID','--value');assert os.readlink('/proc/'+pid+'/exe')==str(binary)
    after=state();idle(after);assert periods()==old_periods
    for p,data in {**histories,**boards}.items():assert p.read_bytes()==data,'Saved chat or board changed'
    nginx.write_bytes(published);run('nginx','-t');run('systemctl','reload','nginx');exposed=True
    report={'status':'published','finished_at_utc':datetime.now(timezone.utc).isoformat(),'maintenance_seconds':round(time.monotonic()-started,2),'release':str(new),'previous_release':str(old),'service_pid':pid,'schema':21,'data_migrations':0,'before_state':before,'after_state':after,'existing_allowance_rows_preserved':len(old_periods),'existing_reader_and_admin_health_preserved':True,'backup':str(backup),'model_requests':0}
    report.update(historical_chat_files_preserved=len(histories),saved_board_files_preserved=len(boards))
    (stage/'deployment.json').write_text(json.dumps(report,indent=2));print(json.dumps(report),flush=True)
    runpy.run_path(str(stage/'verify_public.py'))
except BaseException as failure:
    if exposed:
        (stage/'post-exposure-failure.json').write_text(json.dumps({'failure':str(failure),'production_data_rewound':False}));raise
    if stopped:
        run('systemctl','stop',service)
        for p in [unit,envfile,sandbox]:p.write_bytes(original[p])
        envfile.chmod(0o600);run('systemctl','daemon-reload');run('systemctl','start',service);ready('http://127.0.0.1:8788')
    nginx.write_bytes(original[nginx]);run('nginx','-t');run('systemctl','reload','nginx')
    (stage/'rollback.json').write_text(json.dumps({'status':'restored','failure':str(failure),'production_data_rewound':False},indent=2));raise

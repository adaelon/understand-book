from pathlib import Path
from datetime import datetime,timezone
import json,os,runpy,shlex,shutil,subprocess,tarfile,time

base=Path('/opt/understand-book');stage=base/'acceptance/search-cache-20261021'
old=base/'releases/cloud-20261021';new=base/'releases/search-cache-20261021'
root=base/'data/multi-reader-to-tb-20261012';backup=base/'backups/search-cache-20261021';rollback=base/'data/search-cache-20261021-preupgrade-recovery'
unit=Path('/etc/systemd/system/understand-book-multi.service');nginx=Path('/etc/nginx/conf.d/understand-book-multi.conf')
envfile=base/'multi-reader.env';sandbox=root/'presentation-sandbox.json';service='understand-book-multi'
node=base/'tools/node-v24.20.0-linux-x64/bin/node'
def run(*args):return subprocess.check_output(list(map(str,args)),text=True).strip()
def database(target,expression):
    code="import {DatabaseSync} from 'node:sqlite';const d=new DatabaseSync(process.argv[1],{readOnly:true});"+expression+";d.close();"
    return json.loads(run('runuser','-u','understand-book','--',node,'--disable-warning=ExperimentalWarning','--input-type=module','-e',code,target/'control.sqlite'))
def state(target=root):
    return database(target,"console.log(JSON.stringify({schema:d.prepare('PRAGMA user_version').get().user_version,active:d.prepare(\"SELECT count(*) n FROM run_admissions WHERE dispatch_state IN ('preparing','queued','claimed') OR unsaved!=0\").get().n,model_inflight:d.prepare(\"SELECT count(*) n FROM model_call_charges WHERE state IN ('reserved','sent')\").get().n,service_inflight:d.prepare(\"SELECT count(*) n FROM service_usage WHERE state='sent'\").get().n}))")
def tables(target):
    result=database(target,"const tables={};for(const {name} of d.prepare(\"SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'\").all()){if(!['auth_sessions','sessions'].includes(name))tables[name]=d.prepare('SELECT * FROM \"'+name+'\"').all();}console.log(JSON.stringify(tables))")
    for row in result.get('cloud_storage_adoptions',[]):row.pop('maintained_at',None)
    return {name:sorted(rows,key=repr) for name,rows in result.items()}
def publication_inventory():
    published=root/'library/published'
    return {str(path.relative_to(published)):[path.stat().st_size,path.stat().st_mtime_ns] for path in published.rglob('*') if path.is_file()}

assert (stage/'build.exit').read_text().strip()=='0'
assert (stage/'preflight.exit').read_text().strip()=='0'
assert json.loads((stage/'api-preflight.json').read_text())['status']=='passed'
assert json.loads((stage/'published-backup.json').read_text())['status']=='passed'
assert publication_inventory()==json.loads((stage/'published-inventory.json').read_text()),'Published materials changed since their backup'
assert run('systemctl','show',service,'-p','WorkingDirectory','--value')==str(old)
assert run('systemctl','is-active',service)=='active'
expected={'schema':19,'active':0,'model_inflight':0,'service_inflight':0}
assert state()==expected,'Wait for live work and in-flight charges before switching'
assert shutil.disk_usage(root).free>300*1024**2
ready=runpy.run_path(str(base/'acceptance/cloud-20261021/health.py'))['ready']
before_health=ready('http://127.0.0.1:8788')
pid=run('systemctl','show',service,'-p','MainPID','--value')
environment={part.split(b'=',1)[0].decode():part.split(b'=',1)[1].decode() for part in Path('/proc/'+pid+'/environ').read_bytes().split(b'\0') if b'=' in part}
assert environment.get('OPENCODE_API_KEY') and 'api.deepseek.com' in environment.get('OPENCODE_BASE_URL','')
search_key=environment['OPENCODE_API_KEY']
original={path:path.read_bytes() for path in [unit,nginx,envfile,sandbox]}
configuration=stage/'previous-configuration';configuration.mkdir(mode=0o700)
for path,data in original.items():saved=configuration/path.name;saved.write_bytes(data);saved.chmod(0o600)
assert not (root/'web-rates.json').exists(),'Preserve and explicitly merge an existing tariff before proceeding'
published=original[nginx].decode().replace(str(old),str(new))
maintenance=original[nginx].decode().replace('    server_name understandbook.top;','    server_name understandbook.top;\n    return 503;',1)
assert published!=original[nginx].decode() and maintenance!=original[nginx].decode()
backup.mkdir(mode=0o700)
stopped=False;exposed=False;complete=False;started=datetime.now(timezone.utc).isoformat()
try:
    nginx.write_text(maintenance);run('nginx','-t');run('systemctl','reload','nginx')
    maintenance_start=time.monotonic()
    assert state()==expected,'An answer started before maintenance; do not stop it'
    run('systemctl','stop',service);stopped=True
    before=tables(root)
    subprocess.run(['tar','--exclude=./library/published','-C',str(root),'-czf',str(backup/'mutable.tar.gz'),'.'],check=True)
    validation=stage/'backup-validation';validation.mkdir(mode=0o700)
    with tarfile.open(backup/'mutable.tar.gz') as archive:
        members=archive.getmembers()
        db_members=[member for member in members if member.name.removeprefix('./') in ['control.sqlite','control.sqlite-wal','control.sqlite-shm']]
        archive.extractall(validation,members=db_members,filter='data')
        mutable_files=sum(member.isfile() for member in members)
    run('chown','-R','understand-book:understand-book',validation)
    assert database(validation,"console.log(JSON.stringify(d.prepare('PRAGMA integrity_check').get()))")=={'integrity_check':'ok'}
    assert tables(validation)==before,'Control database backup did not preserve the stopped state'
    snapshot={'status':'complete','source_root':str(root),'schema':19,'published_backup':json.loads((stage/'published-backup.json').read_text()),
      'mutable_archive':str(backup/'mutable.tar.gz'),'mutable_files':mutable_files,'mutable_archive_bytes':(backup/'mutable.tar.gz').stat().st_size,
      'configuration_backup':str(configuration),'finished_at_utc':datetime.now(timezone.utc).isoformat(),
      'restore':'Extract mutable data into a fresh root, copy or link library/published from /opt/understand-book/backups/search-cache-20261021-publications/published, and relocate book_publications.directory.'}
    (backup/'snapshot.json').write_text(json.dumps(snapshot,indent=2));complete=True
    config=json.loads(original[sandbox]);config['worker']=str(new/'target/release/presentation_worker');sandbox.write_text(json.dumps(config))
    next_environment=original[envfile].decode().replace(str(old),str(new))
    next_environment='\n'.join(line for line in next_environment.splitlines() if not line.split('=',1)[0].strip() in ['DEEPSEEK_SEARCH_API_KEY','DEEPSEEK_SEARCH_BASE_URL','DEEPSEEK_SEARCH_QPS','DEEPSEEK_FETCH_QPS'])
    next_environment+='\nDEEPSEEK_SEARCH_API_KEY='+json.dumps(search_key)+'\nDEEPSEEK_SEARCH_BASE_URL=https://api.deepseek.com\nDEEPSEEK_SEARCH_QPS=100\nDEEPSEEK_FETCH_QPS=25\n'
    envfile.write_text(next_environment);envfile.chmod(0o600)
    shutil.copyfile(stage/'web-rates.json',root/'web-rates.json');shutil.chown(root/'web-rates.json',user='understand-book',group='understand-book');(root/'web-rates.json').chmod(0o600)
    unit.write_bytes(original[unit].replace(str(old).encode(),str(new).encode()))
    run('systemctl','daemon-reload');run('systemctl','start',service)
    for attempt in range(40):
        try:
            current=state()
            if current['schema']==20:break
        except Exception:pass
        time.sleep(0.5)
    assert state()=={**expected,'schema':20}
    after=tables(root)
    changed=[name for name,rows in before.items() if rows!=after[name]];assert not changed,changed
    after_health=ready('http://127.0.0.1:8788');assert after_health==before_health
    new_pid=int(run('systemctl','show',service,'-p','MainPID','--value'))
    assert os.readlink('/proc/'+str(new_pid)+'/exe')==str(new/'target/release/server')
    process_env={part.split(b'=',1)[0].decode():part.split(b'=',1)[1].decode() for part in Path('/proc/'+str(new_pid)+'/environ').read_bytes().split(b'\0') if b'=' in part}
    assert process_env.get('DEEPSEEK_SEARCH_API_KEY')==search_key==process_env.get('OPENCODE_API_KEY')
    nginx.write_text(published);run('nginx','-t');run('systemctl','reload','nginx');exposed=True
    public=ready();assert public==before_health
    result={'status':'passed','started_at_utc':started,'finished_at_utc':datetime.now(timezone.utc).isoformat(),
      'release':str(new),'previous_release':str(old),'service_pid':new_pid,'data_root':str(root),
      'schema_before':19,'schema_after':20,'legacy_tables_preserved':{name:len(rows) for name,rows in before.items()},'excluded_volatile_fields':['cloud_storage_adoptions.maintained_at'],
      'backup':snapshot,'maintenance_seconds':round(time.monotonic()-maintenance_start,2),
      'search_key_reused':True,'search_qps':100,'fetch_qps':25,'rates':json.loads((root/'web-rates.json').read_text()),
      'local':after_health,'public':public,'model_requests':0,'web_requests':0}
    (stage/'deployment.json').write_text(json.dumps(result,indent=2));print(json.dumps({key:result[key] for key in ['status','release','service_pid','schema_after','maintenance_seconds','search_key_reused']},indent=2))
except BaseException:
    if exposed:
        print('Public traffic has resumed. Keep all current data and diagnose without restoring the old backup.');raise
    if stopped:
        run('systemctl','stop',service)
        if complete and state()['schema']==20:
            assert not rollback.exists();rollback.mkdir(mode=0o700)
            with tarfile.open(backup/'mutable.tar.gz') as archive:archive.extractall(rollback,filter='data')
            subprocess.run(['cp','-al',str(root/'library/published'),str(rollback/'library/published')],check=True)
            code="""import {DatabaseSync} from 'node:sqlite';const d=new DatabaseSync(process.argv[1]);const old=process.argv[2],next=process.argv[3];for(const row of d.prepare('SELECT book_id,publication_id,directory FROM book_publications').all()){if(!row.directory.startsWith(old+'/'))throw Error('Unexpected publication root');d.prepare('UPDATE book_publications SET directory=? WHERE book_id=? AND publication_id=?').run(next+row.directory.slice(old.length),row.book_id,row.publication_id);}d.close();"""
            run(node,'--disable-warning=ExperimentalWarning','--input-type=module','-e',code,rollback/'control.sqlite',root,rollback)
            (rollback/'presentation-sandbox.json').write_bytes(original[sandbox])
            run('chown','-R','understand-book:understand-book',rollback)
            envfile.write_bytes(original[envfile].replace(str(root).encode(),str(rollback).encode()))
            (stage/'rollback.json').write_text(json.dumps({'data_root':str(rollback),'preserved_failed_root':str(root),'immutable_publications':'hard links to the unchanged originals'}))
        else:
            envfile.write_bytes(original[envfile]);sandbox.write_bytes(original[sandbox])
            if (root/'web-rates.json').exists():(root/'web-rates.json').unlink()
        unit.write_bytes(original[unit]);run('systemctl','daemon-reload');run('systemctl','start',service);ready('http://127.0.0.1:8788')
    nginx.write_bytes(original[nginx]);run('nginx','-t');run('systemctl','reload','nginx')
    print('Previous service restored before public exposure; failed data is preserved.');raise

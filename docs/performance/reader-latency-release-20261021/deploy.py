from pathlib import Path
from datetime import datetime,timezone
import json,os,runpy,shutil,subprocess,time

base=Path('/opt/understand-book');stage=base/'acceptance/reader-latency-20261021'
old=base/'releases/search-cache-20261021';new=base/'releases/reader-latency-20261021'
root=base/'data/multi-reader-to-tb-20261012';backup=base/'backups/reader-latency-20261021'
unit=Path('/etc/systemd/system/understand-book-multi.service');nginx=Path('/etc/nginx/conf.d/understand-book-multi.conf')
envfile=base/'multi-reader.env';sandbox=root/'presentation-sandbox.json';service='understand-book-multi'
node=base/'tools/node-v24.20.0-linux-x64/bin/node'
def run(*args):return subprocess.check_output(list(map(str,args)),text=True).strip()
def state():
    code="import {DatabaseSync} from 'node:sqlite';const d=new DatabaseSync(process.argv[1],{readOnly:true});console.log(JSON.stringify({schema:d.prepare('PRAGMA user_version').get().user_version,active:d.prepare(\"SELECT count(*) n FROM run_admissions WHERE dispatch_state IN ('preparing','queued','claimed') OR unsaved!=0\").get().n,model_inflight:d.prepare(\"SELECT count(*) n FROM model_call_charges WHERE state IN ('reserved','sent')\").get().n,service_inflight:d.prepare(\"SELECT count(*) n FROM service_usage WHERE state='sent'\").get().n}));d.close();"
    return json.loads(run('runuser','-u','understand-book','--',node,'--disable-warning=ExperimentalWarning','--input-type=module','-e',code,root/'control.sqlite'))
ready=runpy.run_path(str(base/'acceptance/cloud-20261021/health.py'))['ready']
Client=runpy.run_path(str(old/'scripts/linux/smoke-mcp-release.py'))['Client']
def catalog(transport):
    client=Client({'origin':'https://understandbook.top','transport_url':transport})
    password=json.loads((base/'multi-reader-credentials.json').read_text())['reader']
    client.login({'username':'reader','password':password})
    try:
        samples=[]
        for _ in range(2):
            start=time.perf_counter();value=client.json('/api/workspace-releases?limit=12&offset=0')
            samples.append(round((time.perf_counter()-start)*1000,2))
            assert value['items'],'Course catalog disappeared'
            assert all(item['availability']['status']=='ready' for item in value['items'])
        return {'milliseconds':samples,'total':value['total'],'all_ready':True,'items':value['items']}
    finally:client.json('/api/auth/logout',{})

assert (stage/'build.exit').read_text().strip()=='0'
assert json.loads((stage/'local-verification.json').read_text())['status']=='passed'
assert run('systemctl','show',service,'-p','WorkingDirectory','--value')==str(old)
assert run('systemctl','is-active',service)=='active'
expected={'schema':20,'active':0,'model_inflight':0,'service_inflight':0}
assert state()==expected,'Wait for live work and in-flight charges before switching'
assert shutil.disk_usage(root).free>300*1024**2
before_health=ready('http://127.0.0.1:8788')
original={path:path.read_bytes() for path in [unit,nginx,envfile,sandbox]}
backup.mkdir(mode=0o700)
for path,data in original.items():saved=backup/path.name;saved.write_bytes(data);saved.chmod(0o600)
published=original[nginx].decode().replace(str(old),str(new))
maintenance=original[nginx].decode().replace('    server_name understandbook.top;','    server_name understandbook.top;\n    return 503;',1)
assert published!=original[nginx].decode() and maintenance!=original[nginx].decode()
stopped=False;exposed=False;started=datetime.now(timezone.utc).isoformat()
try:
    nginx.write_text(maintenance);run('nginx','-t');run('systemctl','reload','nginx')
    maintenance_start=time.monotonic()
    assert state()==expected,'An answer started before maintenance; do not stop it'
    run('systemctl','stop',service);stopped=True
    subprocess.run(['tar','--exclude=./library/published','-C',str(root),'-czf',str(backup/'mutable.tar.gz'),'.'],check=True)
    config=json.loads(original[sandbox]);config['worker']=str(new/'target/release/presentation_worker');sandbox.write_text(json.dumps(config))
    envfile.write_bytes(original[envfile].replace(str(old).encode(),str(new).encode()));envfile.chmod(0o600)
    unit.write_bytes(original[unit].replace(str(old).encode(),str(new).encode()))
    run('systemctl','daemon-reload');run('systemctl','start',service)
    after_health=ready('http://127.0.0.1:8788');assert after_health==before_health
    assert state()==expected
    new_pid=int(run('systemctl','show',service,'-p','MainPID','--value'))
    assert os.readlink('/proc/'+str(new_pid)+'/exe')==str(new/'target/release/server')
    local=catalog('http://127.0.0.1:8788')
    assert max(local['milliseconds'])<15000,'Catalog still exceeds the UI timeout'
    nginx.write_text(published);run('nginx','-t');run('systemctl','reload','nginx');exposed=True
    maintenance_seconds=round(time.monotonic()-maintenance_start,2)
    public_health=ready();assert public_health==before_health
    public=catalog('https://understandbook.top')
    assert local['items']==public['items'],'Public and origin course catalogs differ'
    local.pop('items');public.pop('items')
    result={'status':'passed','started_at_utc':started,'finished_at_utc':datetime.now(timezone.utc).isoformat(),
      'release':str(new),'previous_release':str(old),'service_pid':new_pid,'data_root':str(root),
      'schema':20,'active_before':expected,'backup':str(backup),'maintenance_seconds':maintenance_seconds,
      'catalog_origin':local,'catalog_public_from_server':public,'accounts':public_health,'model_requests':0,'web_requests':0}
    (stage/'deployment.json').write_text(json.dumps(result,indent=2));print(json.dumps(result,indent=2))
except BaseException:
    if exposed:
        print('Public traffic has resumed. Preserve current data and diagnose without restoring a database backup.');raise
    if stopped:
        run('systemctl','stop',service)
        for path in [unit,envfile,sandbox]:path.write_bytes(original[path])
        run('systemctl','daemon-reload');run('systemctl','start',service);ready('http://127.0.0.1:8788')
    nginx.write_bytes(original[nginx]);run('nginx','-t');run('systemctl','reload','nginx')
    print('Previous service restored before public exposure; all current data preserved.');raise

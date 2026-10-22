from pathlib import Path
from datetime import datetime,timezone
import json,os,runpy,shutil,sqlite3,subprocess,time,urllib.request
B=Path('/opt/understand-book');S=B/'acceptance/cloud-20261021'
OLD=B/'releases/om6-20261020';NEW=B/'releases/cloud-20261021'
ROOT=B/'data/multi-reader-to-tb-20261012';BACKUP=B/'backups/cloud-20261021';ROLLBACK=B/'data/cloud-20261021-preupgrade-recovery'
UNIT=Path('/etc/systemd/system/understand-book-multi.service');NGINX=Path('/etc/nginx/conf.d/understand-book-multi.conf')
ENV=B/'multi-reader.env';SANDBOX=ROOT/'presentation-sandbox.json';SERVICE='understand-book-multi'
def run(*args):return subprocess.check_output(list(map(str,args)),text=True).strip()
def state():
 code="""import {DatabaseSync} from 'node:sqlite';const d=new DatabaseSync(process.argv[1],{readOnly:true});console.log(JSON.stringify({schema:d.prepare('PRAGMA user_version').get().user_version,active:d.prepare("SELECT count(*) n FROM run_admissions WHERE dispatch_state IN ('preparing','queued','claimed') OR unsaved!=0").get().n}));d.close();"""
 return json.loads(run(B/'tools/node-v24.20.0-linux-x64/bin/node','--disable-warning=ExperimentalWarning','--input-type=module','-e',code,ROOT/'control.sqlite'))
ready=runpy.run_path(str(S/'health.py'))['ready']
old_ready=runpy.run_path(str(B/'acceptance/om6-20261020/health.py'))['ready']
assert (S/'build.exit').read_text().strip()=='0'
assert (S/'preflight.exit').read_text().strip()=='0'
assert json.loads((S/'api-preflight.json').read_text())['status']=='passed'
assert json.loads((S/'conversion-check.json').read_text())['status']=='passed'
assert run('systemctl','show',SERVICE,'-p','WorkingDirectory','--value')==str(OLD)
assert run('systemctl','is-active',SERVICE)=='active'
assert state()=={'schema':18,'active':0},'Wait for active or unsaved answers before deployment'
assert shutil.disk_usage(ROOT).free>2*1024**3
before=old_ready('http://127.0.0.1:8788')
original={p:p.read_bytes() for p in [UNIT,NGINX,ENV,SANDBOX]}
configuration=S/'previous-configuration';configuration.mkdir(mode=0o700)
for p,data in original.items():q=configuration/p.name;q.write_bytes(data);q.chmod(0o600)
nginx=original[NGINX].decode();published=nginx.replace(str(OLD),str(NEW))
download=f'''    location = /downloads/UnderstandBookSetup.exe {{
        alias {NEW}/downloads/UnderstandBookSetup.exe;
        default_type application/octet-stream;
        add_header Content-Disposition 'attachment; filename="UnderstandBookSetup.exe"';
        add_header Cache-Control "no-store";
    }}
'''
published=published.replace('    server_name understandbook.top;','    server_name understandbook.top;\n'+download,1)
maintenance=nginx.replace('    server_name understandbook.top;','    server_name understandbook.top;\n    return 503;',1)
assert str(NEW) in published and maintenance!=nginx
stopped=False;public_exposed=False;backup_complete=False;started=datetime.now(timezone.utc).isoformat()
try:
 NGINX.write_text(maintenance);run('nginx','-t');run('systemctl','reload','nginx')
 assert state()=={'schema':18,'active':0},'An answer started before maintenance; leave it running'
 stop_at=time.monotonic();run('systemctl','stop',SERVICE);stopped=True
 backup=json.loads(run('runuser','-u','understand-book','--',OLD/'target/release/reader_maintenance','backup',ROOT,BACKUP));assert backup['state']=='complete';backup_complete=True
 config=json.loads(original[SANDBOX]);config['worker']=str(NEW/'target/release/presentation_worker');SANDBOX.write_text(json.dumps(config))
 unit=original[UNIT].replace(str(OLD).encode(),str(NEW).encode());assert unit!=original[UNIT];UNIT.write_bytes(unit)
 environment=original[ENV].decode().replace(str(OLD),str(NEW));assert 'UNDERSTAND_BOOK_LIBREOFFICE=' not in environment
 ENV.write_text(environment.rstrip()+'\nUNDERSTAND_BOOK_LIBREOFFICE=/opt/libreoffice26.8/program/soffice\n')
 run('systemctl','daemon-reload');run('systemctl','start',SERVICE)
 after=ready('http://127.0.0.1:8788');assert after==before
 assert state()=={'schema':19,'active':0}
 pid=int(run('systemctl','show',SERVICE,'-p','MainPID','--value'));assert os.readlink('/proc/'+str(pid)+'/exe')==str(NEW/'target/release/server')
 NGINX.write_text(published);run('nginx','-t');run('systemctl','reload','nginx');public_exposed=True
 interruption=round(time.monotonic()-stop_at,2)
 public=ready();assert public==before
 frontend=runpy.run_path(str(B/'acceptance/full-release-20261019/static-check.py'))['verify'](NEW)
 with urllib.request.urlopen(urllib.request.Request('https://understandbook.top/downloads/UnderstandBookSetup.exe',headers={'User-Agent':'UnderstandBook-MCP10-Verification'}),timeout=60) as response:
  installer=response.read();assert installer==(NEW/'downloads/UnderstandBookSetup.exe').read_bytes()
  assert response.headers.get_content_type()=='application/octet-stream'
 report={'status':'passed','started_at_utc':started,'finished_at_utc':datetime.now(timezone.utc).isoformat(),
  'release':str(NEW),'previous_release':str(OLD),'base_commit':'33c19c7fa8aec9a4f1031122bc177743b234b9fd',
  'data_root':str(ROOT),'schema_before':18,'schema_after':19,'backup':str(BACKUP),'configuration_backup':str(configuration),
  'service_pid':pid,'stop_start_seconds':interruption,'local':after,'public':public,'frontend':frontend,
  'installer':{'version':'0.3.0','bytes':len(installer),'public_matches_candidate':True},
  'commercial_products':0,'model_calls':0,'free_bytes':shutil.disk_usage(ROOT).free}
 (S/'deployment.json').write_text(json.dumps(report,indent=2));print(json.dumps(report,indent=2))
except BaseException:
 if public_exposed:
  print('Verification failed after reopening. Keep the new version and preserve all current user data for diagnosis.');raise
 if stopped:
  run('systemctl','stop',SERVICE)
  if backup_complete and state()['schema']==19:
   assert not ROLLBACK.exists()
   result=json.loads(run('runuser','-u','understand-book','--',OLD/'target/release/reader_maintenance','restore',BACKUP,ROLLBACK));assert result['state']=='complete'
   ENV.write_bytes(original[ENV].replace(str(ROOT).encode(),str(ROLLBACK).encode()))
   (ROLLBACK/'presentation-sandbox.json').write_bytes(original[SANDBOX])
   (S/'rollback.json').write_text(json.dumps({'data_root':str(ROLLBACK),'preserved_failed_data':str(ROOT),'backup':str(BACKUP)}))
  else:ENV.write_bytes(original[ENV]);SANDBOX.write_bytes(original[SANDBOX])
  UNIT.write_bytes(original[UNIT]);run('systemctl','daemon-reload');run('systemctl','start',SERVICE);old_ready('http://127.0.0.1:8788')
 NGINX.write_bytes(original[NGINX]);run('nginx','-t');run('systemctl','reload','nginx')
 print('Previous version restored before public exposure. Schema-19 data is never opened by the old binary.');raise

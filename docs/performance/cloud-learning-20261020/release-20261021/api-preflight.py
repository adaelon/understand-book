from pathlib import Path
import json,os,runpy,shlex,signal,sqlite3,subprocess
B=Path('/opt/understand-book');S=B/'acceptance/cloud-20261021';N=B/'releases/cloud-20261021';OLD=B/'releases/om6-20261020'
C=S/'preflight-copy';ROOT=B/'data/multi-reader-to-tb-20261012'
def data_state(root):
 code="""import {DatabaseSync} from 'node:sqlite';const d=new DatabaseSync(process.argv[1],{readOnly:true});const names=d.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'").all().map(x=>x.name);const tables={};for(const n of names){if(!['auth_sessions','sessions'].includes(n))tables[n]=d.prepare('SELECT * FROM "'+n+'"').all();}console.log(JSON.stringify({schema:d.prepare('PRAGMA user_version').get().user_version,tables}));d.close();"""
 result=json.loads(subprocess.check_output([str(B/'tools/node-v24.20.0-linux-x64/bin/node'),'--disable-warning=ExperimentalWarning','--input-type=module','-e',code,str(root/'control.sqlite')],text=True))
 return result['schema'],{n:sorted(rows,key=repr) for n,rows in result['tables'].items()}
assert (S/'build.exit').read_text().strip()=='0'
if not C.exists():
 restored=json.loads(subprocess.check_output([str(OLD/'target/release/reader_maintenance'),'restore',str(B/'backups/om6-20261020'),str(C)],text=True))
 assert restored['state']=='complete'
before_schema,before=data_state(C);assert before_schema==18
config=json.loads((ROOT/'presentation-sandbox.json').read_text());config['worker']=str(N/'target/release/presentation_worker');config['instance']='cloud-preflight'
(C/'presentation-sandbox.json').write_text(json.dumps(config))
subprocess.run(['chown','-R','understand-book:understand-book',str(C)],check=True)
env=os.environ.copy()
for line in (B/'multi-reader.env').read_text().splitlines():
 line=line.strip()
 if line and not line.startswith('#') and '=' in line:
  key,value=line.split('=',1);env[key]=' '.join(shlex.split(value))
for key,value in list(env.items()):env[key]=value.replace(str(ROOT),str(C)).replace(str(OLD),str(N))
env.update({'UNDERSTAND_BOOK_SERVICE_ROOT':str(C),'UNDERSTAND_BOOK_ADDR':'127.0.0.1:8879','UNDERSTAND_BOOK_LIBREOFFICE':'/opt/libreoffice26.8/program/soffice',
 'UNDERSTAND_BOOK_PRESENTATION_SANDBOX_CONFIG':str(C/'presentation-sandbox.json'),
 'OPENCODE_API_KEY':'isolated-preflight-no-model-call','OPENCODE_BASE_URL':'http://127.0.0.1:9','UB_OBSERVABILITY_MODE':'off'})
with (S/'preflight-server.log').open('w') as log:
 process=subprocess.Popen(['runuser','-u','understand-book','--',str(N/'target/release/server'),'--multi-user',str(C),'--origin','https://understandbook.top','--addr','127.0.0.1:8879'],env=env,stdout=log,stderr=log,cwd=N,start_new_session=True)
 try:
  health=runpy.run_path(str(S/'health.py'))['ready']('http://127.0.0.1:8879')
  after_schema,after=data_state(C);assert after_schema==19
  changed=[n for n,rows in before.items() if rows!=after[n]]
  assert not changed,changed
  report={'status':'passed','release':str(N),'schema_before':before_schema,'schema_after':after_schema,'legacy_tables_preserved':{n:len(rows) for n,rows in before.items()},'accounts':health,'model_calls':0}
  (S/'api-preflight.json').write_text(json.dumps(report,indent=2));print(json.dumps(report,indent=2))
 finally:
  os.killpg(process.pid,signal.SIGTERM)
  try:process.wait(timeout=20)
  except subprocess.TimeoutExpired:os.killpg(process.pid,signal.SIGKILL);process.wait()

from pathlib import Path
import json,os,runpy,shlex,signal,subprocess,time
base=Path('/opt/understand-book');stage=base/'acceptance/search-cache-20261021';new=base/'releases/search-cache-20261021';old=base/'releases/cloud-20261021'
copy=stage/'preflight-copy-attempt5';root=base/'data/multi-reader-to-tb-20261012'
def data_state(target):
    code="""import {DatabaseSync} from 'node:sqlite';const d=new DatabaseSync(process.argv[1],{readOnly:true});const names=d.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'").all().map(x=>x.name);const tables={};for(const n of names){if(!['auth_sessions','sessions'].includes(n))tables[n]=d.prepare('SELECT * FROM "'+n+'"').all();}console.log(JSON.stringify({schema:d.prepare('PRAGMA user_version').get().user_version,tables}));d.close();"""
    result=json.loads(subprocess.check_output([str(base/'tools/node-v24.20.0-linux-x64/bin/node'),'--disable-warning=ExperimentalWarning','--input-type=module','-e',code,str(target/'control.sqlite')],text=True))
    # /account/storage refreshes this maintenance timestamp in both releases.
    for row in result['tables'].get('cloud_storage_adoptions',[]):row.pop('maintained_at',None)
    return result['schema'],{name:sorted(rows,key=repr) for name,rows in result['tables'].items()}
assert (stage/'build.exit').read_text().strip()=='0'
assert not copy.exists()
restored=json.loads(subprocess.check_output([str(old/'target/release/reader_maintenance'),'restore',str(base/'backups/cloud-20261021'),str(copy)],text=True));assert restored['state']=='complete'
config=json.loads((root/'presentation-sandbox.json').read_text());config['instance']='search-cache-preflight'
(copy/'presentation-sandbox.json').write_text(json.dumps(config))
(copy/'web-rates.json').write_bytes((stage/'web-rates.json').read_bytes())
subprocess.run(['chown','-R','understand-book:understand-book',str(copy)],check=True)
env=os.environ.copy()
for line in (base/'multi-reader.env').read_text().splitlines():
    line=line.strip()
    if line and not line.startswith('#') and '=' in line:
        key,value=line.split('=',1);env[key]=' '.join(shlex.split(value))
for key,value in list(env.items()):env[key]=value.replace(str(root),str(copy)).replace(str(old),str(new))
env.update({'UNDERSTAND_BOOK_SERVICE_ROOT':str(copy),'UNDERSTAND_BOOK_ADDR':'127.0.0.1:8879',
 'UNDERSTAND_BOOK_PRESENTATION_SANDBOX_CONFIG':str(copy/'presentation-sandbox.json'),
 'OPENCODE_API_KEY':'isolated-preflight-no-model-call','OPENCODE_BASE_URL':'http://127.0.0.1:9',
 'DEEPSEEK_SEARCH_API_KEY':'isolated-preflight-no-web-call','DEEPSEEK_SEARCH_BASE_URL':'http://127.0.0.1:9','UB_OBSERVABILITY_MODE':'off'})
def check_server(release):
    config['worker']=str(release/'target/release/presentation_worker')
    (copy/'presentation-sandbox.json').write_text(json.dumps(config))
    subprocess.run(['chown','-R','understand-book:understand-book',str(copy)],check=True)
    log=(stage/('preflight-'+release.name+'-server.log')).open('w')
    process=subprocess.Popen(['runuser','-u','understand-book','--',str(release/'target/release/server'),'--multi-user',str(copy),'--origin','https://understandbook.top','--addr','127.0.0.1:8879'],env=env,stdout=log,stderr=log,cwd=release,start_new_session=True)
    try:
        for attempt in range(3):
            try:return runpy.run_path(str(base/'acceptance/cloud-20261021/health.py'))['ready']('http://127.0.0.1:8879')
            except AssertionError as error:
                if 'got 503' not in str(error) or attempt==2:raise
                time.sleep(2)
    finally:
        os.killpg(process.pid,signal.SIGTERM)
        try:process.wait(timeout=20)
        except subprocess.TimeoutExpired:os.killpg(process.pid,signal.SIGKILL);process.wait()
        log.close()
# The archived schema-18 data has not completed the already-published cloud
# material adoption. Run the current schema-19 service first, then measure only
# the Search/Fetch upgrade against that fully initialized baseline.
before_health=check_server(old)
before_schema,before=data_state(copy);assert before_schema==19
health=check_server(new);assert health==before_health
after_schema,after=data_state(copy);assert after_schema==20
changed=[name for name,rows in before.items() if rows!=after[name]];assert not changed,changed
added=sorted(set(after)-set(before));assert 'web_usage' in added
report={'status':'passed','release':str(new),'schema_before':19,'schema_after':20,
  'legacy_tables_preserved':{name:len(rows) for name,rows in before.items()},'added_tables':added,
  'accounts':health,'model_requests':0,'web_requests':0,'excluded_volatile_fields':['cloud_storage_adoptions.maintained_at'],
  'source':'Existing complete backup restored and initialized by the currently deployed schema-19 service, then upgraded by the candidate.'}
(stage/'api-preflight.json').write_text(json.dumps(report,indent=2));print(json.dumps(report,indent=2))

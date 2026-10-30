from pathlib import Path
import json,os,runpy,subprocess,shlex
stage=Path(__file__).resolve().parent
client=runpy.run_path('tmp/mu12/remote.py')['connect']()
try:
 with client.open_sftp() as sftp:
  with sftp.open('/opt/understand-book/multi-reader-credentials.json') as f: password=json.loads(f.read())['reader']
 script="""from pathlib import Path
import json
result=[]
for p in Path('/opt/understand-book/data/multi-reader-to-tb-20261012/users/reader/memory/boards').glob('**/head.json'):
 d=json.loads(p.read_text())
 if isinstance(d,dict) and d.get('items') and d.get('publication_id'):
  result.append({'board_id':d['board_id'],'book_id':d['book_id'],'publication_id':d['publication_id'],'learning_workspace_id':d.get('learning_workspace_id'),'items':len(d['items'])})
print(json.dumps(sorted(result,key=lambda d:d['items'],reverse=True)))
"""
 _,out,err=client.exec_command('python3 -c '+shlex.quote(script)); boards=json.loads(out.read())
finally:client.close()
env=os.environ.copy();env['TEMP']=str(stage);env['TMP']=str(stage)
result=subprocess.run(['node',str(stage/'public_browser.mjs')],input=json.dumps({'account':{'username':'reader','password':password},'boards':boards}),text=True,env=env)
raise SystemExit(result.returncode)

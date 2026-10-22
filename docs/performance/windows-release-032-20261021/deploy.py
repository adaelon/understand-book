from pathlib import Path
from datetime import datetime, timezone
import json, sys, urllib.request, time, logging

root = Path(__file__).parent
installer = root/'source/dist/UnderstandBookSetup.exe'
assert 'Finished 1 bundle' in (root/'build.log').read_text('utf-8')
assert json.loads((root/'webview2.json').read_text('utf-8'))['status'] == 'passed'
sys.path.insert(0,str(Path('tmp/mu12').resolve()))
from remote import connect
logging.getLogger('paramiko.transport').setLevel(logging.CRITICAL)
for attempt in range(3):
    try:
        client=connect();break
    except Exception:
        if attempt==2: raise
        time.sleep(10)
remote='/opt/understand-book/acceptance/windows-032-20261021'
script=r'''
from pathlib import Path
import shutil, subprocess, os, json
root=Path('/opt/understand-book/acceptance/windows-032-20261021')
release=Path('/opt/understand-book/releases/search-cache-20261021')
run=lambda *args:subprocess.check_output(args,text=True).strip()
assert run('systemctl','show','understand-book-multi','-p','WorkingDirectory','--value')==str(release)
assert run('systemctl','is-active','understand-book-multi')=='active'
pid=run('systemctl','show','understand-book-multi','-p','MainPID','--value')
download=release/'downloads/UnderstandBookSetup.exe'
backup=root/'UnderstandBookSetup-0.3.1.exe'
if not backup.exists():shutil.copy2(download,backup)
shutil.copy2(root/'UnderstandBookSetup-0.3.2.exe',download.with_suffix('.next.exe'))
os.replace(download.with_suffix('.next.exe'),download)
assert download.read_bytes()==(root/'UnderstandBookSetup-0.3.2.exe').read_bytes()
assert pid==run('systemctl','show','understand-book-multi','-p','MainPID','--value')
assert run('systemctl','is-active','understand-book-multi')=='active'
print(json.dumps({'status':'published','release':str(release),'backup':str(backup),'service_pid':pid,'server_restarted':False,'database_changes':False,'installer_bytes':download.stat().st_size}))
'''
try:
    _,out,err=client.exec_command('mkdir -p '+remote)
    assert out.channel.recv_exit_status()==0,err.read().decode()
    with client.open_sftp() as sftp:
        sftp.put(str(installer),remote+'/UnderstandBookSetup-0.3.2.exe')
        with sftp.open(remote+'/publish.py','w') as f:f.write(script)
    _,out,err=client.exec_command('python3 '+remote+'/publish.py')
    stdout=out.read().decode();stderr=err.read().decode()
    assert out.channel.recv_exit_status()==0,stderr
    report=json.loads(stdout)
finally:client.close()
(root/'deployment.json').write_text(json.dumps(report,indent=2),'utf-8')
checks=[]
for suffix in ['?v=0.3.2','']:
    url='https://understandbook.top/downloads/UnderstandBookSetup.exe'+suffix
    request=urllib.request.Request(url,headers={'User-Agent':'UnderstandBook-MCP10-Verification','Cache-Control':'no-cache'})
    with urllib.request.urlopen(request,timeout=60) as response:
        body=response.read()
        checks.append({'url':url,'bytes':len(body),'status':response.status,'cache_status':response.headers.get('CF-Cache-Status'),'matches':body==installer.read_bytes()})
    assert checks[-1]['matches'], 'Public download is not the 0.3.2 candidate: '+url
report.update(status='passed',version='0.3.2',public_downloads=checks,finished_at_utc=datetime.now(timezone.utc).isoformat(),model_requests=0,search_requests=0,fetch_requests=0)
(root/'deployment.json').write_text(json.dumps(report,ensure_ascii=False,indent=2),'utf-8')
print(json.dumps(report,ensure_ascii=False,indent=2))

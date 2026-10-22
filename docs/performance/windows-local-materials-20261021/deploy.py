from pathlib import Path
from datetime import datetime, timezone
import json, sys, tarfile, urllib.request, re
sys.path.insert(0, str(Path('tmp/mu12').resolve()))
from remote import connect
root = Path(__file__).parent
web = root/'frontend/packages/web/dist'
assert 'built in' in (root/'web-build.log').read_text(encoding='utf-8')
assert 'Finished 1 bundle' in (root/'windows-build.log').read_text(encoding='utf-8')
assert json.loads((root/'webview2.json').read_text())['status'] == 'passed'
installer = Path('dist/UnderstandBookSetup.exe')
assert installer.stat().st_size == 4899391
archive = root/'static.tar.gz'
with tarfile.open(archive, 'w:gz') as target:
    for file in web.rglob('*'):
        if file.is_file(): target.add(file, arcname='web/'+file.relative_to(web).as_posix())
    target.add(installer, arcname='UnderstandBookSetup.exe')
remote_root = '/opt/understand-book/acceptance/local-library-20261021'
script = r'''
from pathlib import Path
import json, subprocess, shutil, tarfile, os
root=Path('/opt/understand-book/acceptance/local-library-20261021')
release=Path('/opt/understand-book/releases/cloud-20261021')
run=lambda *args:subprocess.check_output(args,text=True).strip()
assert run('systemctl','show','understand-book-multi','-p','WorkingDirectory','--value')==str(release)
assert run('systemctl','is-active','understand-book-multi')=='active'
pid=run('systemctl','show','understand-book-multi','-p','MainPID','--value')
backup=root/'backup';backup.mkdir(exist_ok=True)
web=release/'packages/web/dist-multi'
for name in ['index.html','home.html']:
    if not (backup/name).exists():shutil.copy2(web/name,backup/name)
if not (backup/'UnderstandBookSetup.exe').exists():shutil.copy2(release/'downloads/UnderstandBookSetup.exe',backup/'UnderstandBookSetup.exe')
staged=root/'staged';staged.mkdir(exist_ok=True)
with tarfile.open(root/'static.tar.gz') as archive:archive.extractall(staged,filter='data')
# Keep previous assets for already-open clients; switch entry pages after assets and installer.
download=release/'downloads/UnderstandBookSetup.exe'
shutil.copy2(staged/'UnderstandBookSetup.exe',download.with_suffix('.next.exe'))
os.replace(download.with_suffix('.next.exe'),download)
for source in (staged/'web').rglob('*'):
    if source.is_file() and source.suffix!='.html':
        dest=web/source.relative_to(staged/'web');dest.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(source,dest)
for name in ['home.html','index.html']:
    source=staged/'web'/name;dest=web/name;shutil.copy2(source,dest.with_suffix('.next.html'));os.replace(dest.with_suffix('.next.html'),dest)
assert pid==run('systemctl','show','understand-book-multi','-p','MainPID','--value')
assert run('systemctl','is-active','understand-book-multi')=='active'
print(json.dumps({'status':'published','release':str(release),'backup':str(backup),'service_pid':pid,'server_restarted':False,'database_changes':False,'installer_bytes':download.stat().st_size}))
'''
client = connect()
try:
    _, out, err = client.exec_command('mkdir -p '+remote_root)
    assert out.channel.recv_exit_status()==0, err.read().decode()
    with client.open_sftp() as sftp:
        sftp.put(str(archive), remote_root+'/static.tar.gz')
        with sftp.open(remote_root+'/publish.py','w') as stream: stream.write(script)
    _, out, err = client.exec_command('python3 '+remote_root+'/publish.py')
    stdout=out.read().decode();stderr=err.read().decode();assert out.channel.recv_exit_status()==0,stderr
    report=json.loads(stdout)
    (root/'deployment.json').write_text(json.dumps(report,ensure_ascii=False,indent=2),encoding='utf-8')
finally: client.close()
headers={'User-Agent':'UnderstandBook-MCP10-Verification','Cache-Control':'no-cache'}
def get(url):
    with urllib.request.urlopen(urllib.request.Request(url,headers=headers),timeout=60) as response:return response.read()
for name in ['index.html','home.html']:
    body=get('https://understandbook.top/'+name+'?release=local-library-031')
    clean=re.sub(rb'<script\b[^>]*src="https://static\.cloudflareinsights\.com/[^\"]*"[^>]*></script>\s*(?=</body>)',b'',body)
    assert clean==(web/name).read_bytes(), name+' differs beyond the CDN analytics script'
    for link in re.findall(rb'(?:src|href)="(/assets/[^"#]+)"',body):
        asset=link.decode();assert get('https://understandbook.top'+asset)==(web/asset.lstrip('/')).read_bytes(),asset
assert get('https://understandbook.top/downloads/UnderstandBookSetup.exe?v=0.3.1')==installer.read_bytes()
report.update(status='passed',version='0.3.1',public_entry_assets_match=True,public_installer_matches=True,finished_at_utc=datetime.now(timezone.utc).isoformat(),model_requests=0)
(root/'deployment.json').write_text(json.dumps(report,ensure_ascii=False,indent=2),encoding='utf-8')
print(json.dumps(report,ensure_ascii=False,indent=2))

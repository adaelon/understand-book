from pathlib import Path
import json,tarfile,sys
sys.path.insert(0,str(Path('tmp/mu12').resolve()))
from remote import connect
S=Path(__file__).parent;N=S/'frontend'
for kind in ['web','admin']:assert 'built in' in (S/(kind+'-build.log')).read_text(encoding='utf-8')
with tarfile.open(S/'static.tar.gz','w:gz') as a:
 for source,dest in [('packages/web/dist','packages/web/dist-multi'),('apps/admin/dist','apps/admin/dist'),('apps/admin/dist','packages/web/dist-multi/admin')]:
  for p in (N/source).rglob('*'):
   if p.is_file():a.add(p,arcname=dest+'/'+p.relative_to(N/source).as_posix())
 a.add('dist/UnderstandBookSetup.exe',arcname='downloads/UnderstandBookSetup.exe')
c=connect()
try:
 with c.open_sftp() as f:
  for name in ['static.tar.gz','health.py','api-preflight.py']:
   f.put(str(S/name),'/opt/understand-book/acceptance/cloud-20261021/'+name)
 _,o,e=c.exec_command('tar -xzf /opt/understand-book/acceptance/cloud-20261021/static.tar.gz -C /opt/understand-book/releases/cloud-20261021')
 print(o.read().decode());print(e.read().decode());assert o.channel.recv_exit_status()==0
 print(json.dumps({'static_bytes':(S/'static.tar.gz').stat().st_size,'installer_bytes':Path('dist/UnderstandBookSetup.exe').stat().st_size}))
finally:c.close()

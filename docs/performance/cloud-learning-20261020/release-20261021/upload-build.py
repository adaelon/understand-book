from pathlib import Path
import json,shlex,sys
sys.path.insert(0,str(Path('tmp/mu12').resolve()))
from remote import connect
S=Path(__file__).parent
m=json.loads((S/'source.json').read_text());old=m['previous_release'];new=m['release']
stage='/opt/understand-book/acceptance/cloud-20261021'
c=connect()
def run(cmd):
 _,o,e=c.exec_command(cmd);out=o.read().decode();err=e.read().decode()
 if o.channel.recv_exit_status():raise RuntimeError(out+err)
 return out+err
try:
 print(run('mkdir -p '+stage))
 with c.open_sftp() as ftp:
  for name in ['cloud-overlay.tar.gz','source.json']:ftp.put(str(S/name),stage+'/'+name)
 script=f'''from pathlib import Path
import json,shutil,subprocess,tarfile
old=Path({old!r});new=Path({new!r});stage=Path({stage!r})
assert subprocess.check_output(['systemctl','show','understand-book-multi','-p','WorkingDirectory','--value'],text=True).strip()==str(old)
assert shutil.disk_usage(stage).free>2*1024**3
if new.exists():
 assert (stage/'build.exit').read_text().strip()!='0'
 shutil.copyfile(stage/'build.log',stage/'build-attempt1.log')
 (stage/'build.exit').unlink()
 subprocess.run(['systemctl','reset-failed','ub-cloud-20261021-build'],check=True)
else:
 new.mkdir()
 subprocess.run(['cp','-a','--reflink=auto',str(old)+'/.',str(new)],check=True)
with tarfile.open(stage/'cloud-overlay.tar.gz') as a:a.extractall(new,filter='data')
shutil.copyfile(stage/'source.json',new/'cloud-source.json')
print(json.dumps({{'release':str(new),'free_bytes':shutil.disk_usage(stage).free}}))
'''
 print(run('python3 -c '+shlex.quote(script)))
 build=f'''#!/bin/bash
set -euo pipefail
trap 'printf "%s\\n" "$?" > {stage}/build.exit' EXIT
export CARGO_HOME=/opt/understand-book/tools/cargo
export RUSTUP_HOME=/opt/understand-book/tools/rustup
export PATH="$CARGO_HOME/bin:$PATH"
cd {new}
cargo build --locked --release -j 1 -p server --bin server --bin manage_reader --bin publish_book --bin reader_maintenance --bin presentation_worker --bin video_audio_review
'''
 with c.open_sftp() as ftp:
  with ftp.open(stage+'/build.sh','w') as f:f.write(build)
 print(run(f'systemd-run --unit=ub-cloud-20261021-build --property=MemoryMax=2200M --property=MemoryHigh=1900M --property=CPUQuota=100% --property=StandardOutput=append:{stage}/build.log --property=StandardError=append:{stage}/build.log /bin/bash {stage}/build.sh'))
finally:c.close()

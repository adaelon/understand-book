#!/usr/bin/env python3
"""Daily consistent business snapshot; only pause admission after work is idle."""
import argparse, fcntl, json, os, shutil, sqlite3, subprocess, time, urllib.request, urllib.error
from pathlib import Path
BASE=Path('/opt/understand-book');ROOT=BASE/'data/production-20261031'
SERVICE='understand-book-multi.service';NGINX=Path('/etc/nginx/conf.d/understand-book-multi.conf')
DEST=BASE/'backups/business';OPS=BASE/'operations'
def run(*args):
    return subprocess.check_output([str(a) for a in args],text=True).strip()
def save(path,value):
    temp=path.with_suffix('.tmp');temp.write_text(json.dumps(value,indent=2));temp.chmod(0o600);temp.replace(path)
def idle():
    with sqlite3.connect('file:'+str(ROOT/'control.sqlite')+'?mode=ro',uri=True) as db:
        for table,where in [('run_admissions',"dispatch_state IN ('preparing','queued','claimed') OR unsaved!=0"),('model_call_charges',"state IN ('reserved','sent')"),('service_usage',"state IN ('reserved','sent')"),('web_usage',"state IN ('reserved','sent')")]:
            if db.execute('SELECT count(*) FROM '+table+' WHERE '+where).fetchone()[0]:return False
    for path in (ROOT/'users').glob('*/memory/learning.db'):
        with sqlite3.connect('file:'+str(path)+'?mode=ro',uri=True) as db:
            if db.execute("SELECT 1 FROM sqlite_master WHERE name='teaching_coach_jobs'").fetchone() and db.execute("SELECT count(*) FROM teaching_coach_jobs WHERE status IN ('pending','running')").fetchone()[0]:return False
    for path in (ROOT/'users').glob('*/memory/memory.json'):
        if any(j['status']=='running' for j in json.loads(path.read_text()).get('review_state',{}).get('review_jobs',[])):return False
    return True
def ready():
    for _ in range(100):
        try:urllib.request.urlopen(urllib.request.Request('http://127.0.0.1:8788/api/auth/me',headers={'Host':'understandbook.top','Origin':'https://understandbook.top'}),timeout=1)
        except urllib.error.HTTPError as e:
            if e.code==401:return
        except (urllib.error.URLError,TimeoutError):pass
        time.sleep(.15)
    raise RuntimeError('Service did not become ready')
def proxy(content):
    NGINX.write_bytes(content);run('nginx','-t');run('systemctl','reload','nginx')
def sync(source,target,previous=None,extra=()):
    target.mkdir(parents=True,exist_ok=True)
    args=['rsync','-a','--delete','--modify-window=-1',*extra]
    if previous and previous.exists():args+=['--link-dest='+str(previous)]
    run(*args,str(source)+'/',str(target)+'/')
def maintenance_notice(until):
    config=json.loads((OPS/'backup-destination.json').read_text())
    run('ssh','-i',OPS/'backup_ed25519','-o','BatchMode=yes','-o','StrictHostKeyChecking=yes',
        '-o','UserKnownHostsFile='+str(OPS/'backup_known_hosts'),config['host'],
        "mkdir -p /opt/understand-book/operations-external && printf '%s' "+str(until)+" > /opt/understand-book/operations-external/maintenance-until")

def restic_args():
    config=json.loads((OPS/'backup-destination.json').read_text())
    ssh='ssh -i '+str(OPS/'backup_ed25519')+' -o BatchMode=yes -o StrictHostKeyChecking=yes -o UserKnownHostsFile='+str(OPS/'backup_known_hosts')+' '+config['host']+' -s sftp'
    return ['restic','-r','sftp:'+config['host']+':'+config['directory'],'--password-file',str(OPS/'backup-password'),'-o','sftp.command='+ssh]
def offsite(snapshot):
    command=[*restic_args(),'backup','--json','--tag','business','--host','understandbook-production',str(snapshot),str(BASE/'tools'),str(BASE/'models-ubuntu')]
    summary=None
    with (OPS/'backup-transfer.log').open('w') as log:
        process=subprocess.Popen(command,stdout=subprocess.PIPE,stderr=log,text=True,env={**os.environ,'RESTIC_PROGRESS_FPS':'0.016666'})
        for line in process.stdout:
            log.write(line);log.flush()
            if line.startswith('{'):
                event=json.loads(line)
                if event.get('message_type')=='summary':summary=event
        if process.wait()!=0:raise RuntimeError('Off-host transfer failed; see operations/backup-transfer.log')
    if summary is None:raise RuntimeError('Off-host transfer returned no completed snapshot')
    save(snapshot/'transfer.json',summary)
    run(*restic_args(),'forget','--tag','business','--group-by','host,tags','--keep-last','7','--prune')
    return summary['snapshot_id']

def main():
    parser=argparse.ArgumentParser();parser.add_argument('--force',action='store_true');args=parser.parse_args()
    OPS.mkdir(exist_ok=True);DEST.mkdir(parents=True,exist_ok=True,mode=0o700)
    lock=(OPS/'backup.lock').open('a');fcntl.flock(lock,fcntl.LOCK_EX|fcntl.LOCK_NB)
    status_path=OPS/'backup-status.json';status=json.loads(status_path.read_text()) if status_path.exists() else {}
    # Retry a failed transfer before creating another snapshot.
    if status.get('local_snapshot') and not status.get('offsite_at'):
        status['snapshot_id']=offsite(Path(status['local_snapshot']));status['offsite_at']=int(time.time());save(status_path,status)
        temp=DEST/'latest.tmp';temp.unlink(missing_ok=True);temp.symlink_to(Path(status['local_snapshot']).name);temp.replace(DEST/'latest')
    if not args.force and int(time.time())-status.get('finished_at',0)<23*3600:return
    if not idle():print('Active work: backup deferred until the next hourly attempt');return
    previous=DEST/'latest';previous=previous.resolve() if previous.exists() else None
    snapshot=DEST/str(int(time.time()));snapshot.mkdir(mode=0o700)
    # Publications are immutable; the second short pass closes concurrent publication changes.
    sync(ROOT/'library',snapshot/'data/library',previous/'data/library' if previous else None)
    original=NGINX.read_bytes();maintenance=original.replace(b'    location /api/ {',b'    location /api/ {\n        return 503;').replace(b'    location = /mcp {',b'    location = /mcp {\n        return 503;')
    assert maintenance!=original
    stopped=False;start=time.monotonic()
    monitor_active=subprocess.run(['systemctl','is-active','--quiet','understand-book-operations.timer']).returncode==0
    maintenance_notice(int(time.time())+120)
    if monitor_active:run('systemctl','stop','understand-book-operations.timer','understand-book-operations.service')
    try:
        proxy(maintenance)
        if not idle():raise RuntimeError('Work started during backup admission pause; retry later')
        run('systemctl','stop',SERVICE);stopped=True
        sync(ROOT,snapshot/'data',previous/'data' if previous else None,('--exclude=shared-cache','--exclude=observability-spool','--exclude=publish.sock','--exclude=service.lock'))
        config=snapshot/'config';config.mkdir()
        for path in [BASE/'multi-reader.env',BASE/'adm10-admin-credentials.json',BASE/'multi-reader-credentials.json',Path('/etc/systemd/system/'+SERVICE)]:shutil.copy2(path,config/path.name)
        (config/'nginx.conf').write_bytes(original)
        sync(OPS,snapshot/'operations',previous/'operations' if previous else None,('--exclude=backup.lock','--exclude=backup-transfer.log','--exclude=__pycache__'))
        for source in Path('/etc/systemd/system').glob('understand-book-*'):
            if source.is_file():shutil.copy2(source,config/source.name)
        for source in [Path('/etc/nginx'),Path('/etc/letsencrypt')]:
            if source.exists():sync(source,config/source.name)
        # The snapshot must reopen normal admission when restored.
        (config/'nginx/conf.d'/NGINX.name).write_bytes(original)
        source=Path('/etc/polkit-1/rules.d/49-understand-book-render.rules')
        if source.exists():shutil.copy2(source,config/source.name)
        for source in [Path('/etc/sysctl.d/90-understand-book-backup-network.conf'),Path('/etc/modules-load.d/understand-book-bbr.conf')]:
            if source.exists():shutil.copy2(source,config/source.name)
        release=Path(run('systemctl','show',SERVICE,'-p','WorkingDirectory','--value'))
        save(snapshot/'restore.json',{'base':str(BASE),'data_root':str(ROOT),'release':str(release),'created_at':int(time.time())})
    finally:
        try:
            if stopped:run('systemctl','start',SERVICE);ready()
        finally:
            proxy(original)
            if monitor_active:run('systemctl','start','understand-book-operations.timer')
            maintenance_notice(0)
    pause=round(time.monotonic()-start,3)
    # Back up deployable binaries/web/scripts, excluding reproducible compiler caches.
    sync(release,snapshot/'release',previous/'release' if previous else None,('--exclude=.git','--exclude=node_modules','--exclude=.understand-book','--exclude=docs','--exclude=fixtures','--exclude=evals','--exclude=target/debug','--exclude=target/release/deps','--exclude=target/release/build','--exclude=target/release/incremental','--exclude=target/release/.fingerprint','--exclude=target/type-export'))
    with sqlite3.connect('file:'+str(snapshot/'data/control.sqlite')+'?mode=ro',uri=True) as db:
        assert db.execute('PRAGMA quick_check').fetchone()[0]=='ok'
    status={'finished_at':int(time.time()),'local_snapshot':str(snapshot),'offsite_at':None,'maintenance_seconds':pause}
    save(status_path,status)
    status['snapshot_id']=offsite(snapshot);status['offsite_at']=int(time.time());save(status_path,status)
    temp=DEST/'latest.tmp';temp.unlink(missing_ok=True);temp.symlink_to(snapshot.name);temp.replace(DEST/'latest')
    for old in sorted(p for p in DEST.iterdir() if p.is_dir() and p.name.isdigit())[:-2]:shutil.rmtree(old)
    print(json.dumps(status))
if __name__=='__main__':main()

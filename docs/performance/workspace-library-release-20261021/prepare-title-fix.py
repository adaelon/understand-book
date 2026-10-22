from pathlib import Path
import difflib,tarfile,json,sys
stage=Path(__file__).parent;fix=stage/'title-fix';fix.mkdir(exist_ok=True)
names=['crates/server/src/published_library.rs','crates/server/src/tests/video_publication_tests.rs']
patch=[]
for name in names:
    old=(stage/'source'/name).read_text('utf-8');new=Path(name).read_text('utf-8')
    diff=list(difflib.unified_diff(old.splitlines(True),new.splitlines(True),fromfile='a/'+name,tofile='b/'+name))
    assert sum(line.startswith('@@') for line in diff)==1, 'Unexpected unrelated changes: '+name
    patch.extend(diff)
    (stage/'source'/name).write_text(new,'utf-8',newline='\n')
(fix/'release.patch').write_text(''.join(patch),'utf-8',newline='\n')
with tarfile.open(fix/'source-overlay.tar.gz','w:gz') as archive:
    for name in names:archive.add(stage/'source'/name,arcname=name)
remote='/opt/understand-book/acceptance/workspace-library-title-20261021'
old='/opt/understand-book/releases/workspace-library-20261021';new='/opt/understand-book/releases/workspace-library-title-20261021'
shell=(stage/'build.sh').read_text('utf-8').replace('workspace-library-20261021','workspace-library-title-20261021')
shell=shell[:shell.index('cargo build')]+ 'cargo build --locked --release -j1 -p server --bin server\n'
(fix/'build.sh').write_text(shell,'utf-8',newline='\n')
deploy=(stage/'deploy.py').read_text('utf-8').replace("old=base/'releases/reader-latency-20261021';new=base/'releases/workspace-library-20261021'","old=base/'releases/workspace-library-20261021';new=base/'releases/workspace-library-title-20261021'")
deploy=deploy.replace("acceptance/workspace-library-20261021'","acceptance/workspace-library-title-20261021'").replace("backups/workspace-library-20261021'","backups/workspace-library-title-20261021'")
deploy=deploy.replace("assert title and title!=book['published_book_ref']['book_id']", "assert title and title!='external' and title!=book['published_book_ref']['book_id']")
(fix/'deploy.py').write_text(deploy,'utf-8',newline='\n')
(fix/'deploy.sh').write_text((stage/'deploy.sh').read_text('utf-8').replace('workspace-library-20261021','workspace-library-title-20261021'),'utf-8',newline='\n')
sys.path.insert(0,str(Path('tmp/mu12').resolve()))
from remote import connect
client=connect()
try:
    _,out,err=client.exec_command(f'mkdir -p {remote}')
    assert out.channel.recv_exit_status()==0,err.read().decode()
    with client.open_sftp() as sftp:
        for name in ['source-overlay.tar.gz','release.patch','build.sh']:sftp.put(str(fix/name),remote+'/'+name)
    _,out,err=client.exec_command(f'test ! -e {new} && cp -a --reflink=auto {old} {new} && tar -xzf {remote}/source-overlay.tar.gz -C {new} && nohup bash {remote}/build.sh >{remote}/build.log 2>&1 </dev/null &')
    assert out.channel.recv_exit_status()==0,err.read().decode()
    print(json.dumps({'title_build_started':True,'release':new}))
finally:client.close()

from pathlib import Path
import sys, json, tarfile, difflib
repo=Path.cwd(); stage=repo/'tmp/workspace-library-release-20261021'
sys.path.insert(0,str(repo/'tmp/mu12'))
from remote import connect
release='/opt/understand-book/releases/reader-latency-20261021'
remote_stage='/opt/understand-book/acceptance/workspace-library-20261021'
source=stage/'source'
client=connect()
try:
    _,out,err=client.exec_command("systemctl show understand-book-multi -p WorkingDirectory --value")
    assert out.read().decode().strip()==release
    command=f"mkdir -p {remote_stage} && tar --exclude='./target' --exclude='./node_modules' --exclude='*/node_modules' --exclude='./downloads' --exclude='./.git' --exclude='*/dist*' --exclude='./tmp' -czf {remote_stage}/baseline.tar.gz -C {release} ."
    _,out,err=client.exec_command(command); error=err.read().decode()
    assert out.channel.recv_exit_status()==0,error
    with client.open_sftp() as sftp:sftp.get(remote_stage+'/baseline.tar.gz',str(stage/'baseline.tar.gz'))
finally:client.close()
source.mkdir()
with tarfile.open(stage/'baseline.tar.gz') as archive:archive.extractall(source,filter='data')
files=[
'packages/web/src/NetworkApp.vue','packages/web/src/learning-library.css','packages/web/src/components/MaterialCover.vue',
'packages/web/src/components/SharedWorkspaceManifest.vue','packages/web/src/components/SharedWorkspaceCatalog.vue',
'packages/web/src/shared-workspaces.ts','packages/web/src/book-title.ts','packages/web/src/book-title.test.ts',
'packages/web/src/NetworkApp.learning-workspace.test.ts','packages/web/src/NetworkApp.resumption.test.ts',
'packages/web/src/components/CloudPlans.vue','packages/web/src/components/PersonalPreparations.vue','packages/web/src/personal-preparations.ts',
'crates/server/src/book_title.rs','crates/server/src/book_cover.rs','crates/server/src/published_library.rs',
'crates/server/src/authorization.rs','crates/server/src/multi_user_host.rs','crates/server/src/tests/video_publication_tests.rs',
'crates/server/src/cloud_storage.rs','crates/server/src/presentation_store.rs','crates/server/src/user_runtime.rs',
'crates/server/src/tests/cloud_storage_tests.rs','crates/server/src/tests/presentation_store_tests.rs']
patch=[];stats=[]
for name in files:
    before=(source/name).read_text('utf-8-sig') if (source/name).exists() else ''
    after=(repo/name).read_text('utf-8-sig')
    changes=list(difflib.unified_diff(before.splitlines(True),after.splitlines(True),fromfile='a/'+name,tofile='b/'+name))
    patch.extend(changes)
    stats.append({'file':name,'added':sum(s.startswith('+') and not s.startswith('+++') for s in changes),'removed':sum(s.startswith('-') and not s.startswith('---') for s in changes)})
(stage/'candidate.patch').write_text(''.join(patch),'utf-8')
(stage/'files.json').write_text(json.dumps(files,indent=2),'utf-8')
print(json.dumps(stats,indent=2))

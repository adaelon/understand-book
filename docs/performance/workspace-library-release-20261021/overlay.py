from pathlib import Path
import json,re,difflib,tarfile,shutil
repo=Path.cwd();stage=repo/'tmp/workspace-library-release-20261021';source=stage/'source'
patch=(stage/'candidate.patch').read_text('utf-8');parts=re.split(r'(?m)(?=^--- a/)',patch)
excluded={
 'packages/web/src/NetworkApp.vue':{27,196,248,336,372,397,428},
 'packages/web/src/NetworkApp.learning-workspace.test.ts':{22,84,135},
 'packages/web/src/personal-preparations.ts':{28},
}
only={
 'packages/web/src/NetworkApp.resumption.test.ts':{92},
 'crates/server/src/authorization.rs':{180},
 'crates/server/src/multi_user_host.rs':{815},
 'crates/server/src/tests/presentation_store_tests.rs':{18},
}
changes=[];release_patch=[]
for part in filter(None,parts):
    name=part.splitlines()[0][6:];path=source/name
    before=path.read_text('utf-8') if path.exists() else '';lines=before.splitlines(True)
    hunks=re.split(r'(?m)(?=^@@ )',part)[1:];out=[];cursor=0
    for hunk in hunks:
        header,*body=hunk.splitlines(True);old_start=int(re.match(r'@@ -(\d+)',header).group(1))
        if old_start in excluded.get(name,set()) or name in only and old_start not in only[name]:continue
        old=[line[1:] for line in body if line.startswith((' ','-'))]
        new=[line[1:] for line in body if line.startswith((' ','+'))]
        start=max(old_start-1,0);assert lines[start:start+len(old)]==old,(name,old_start)
        out.extend(lines[cursor:start]);out.extend(new);cursor=start+len(old)
    out.extend(lines[cursor:]);after=''.join(out)
    if name=='packages/web/src/NetworkApp.vue':
        current=(repo/name).read_text('utf-8')
        marker='function bookLabel(';end='const readerApp ='
        replacement=current[current.index(marker):current.index(end)]
        after=after[:after.index(marker)]+replacement+after[after.index(end):]
    if after!=before:
        path.parent.mkdir(parents=True,exist_ok=True);path.write_text(after,'utf-8',newline='\n')
        changes.append(name)
        release_patch.extend(difflib.unified_diff(before.splitlines(True),after.splitlines(True),fromfile='a/'+name,tofile='b/'+name))
with tarfile.open(stage/'source-overlay.tar.gz','w:gz') as archive:
    for name in changes:archive.add(source/name,arcname=name)
(stage/'release.patch').write_text(''.join(release_patch),'utf-8')
for directory in ['packages/web','packages/core']:
    shutil.copytree(source/directory,stage/'windows-source'/directory,dirs_exist_ok=True)
report={'release':'/opt/understand-book/releases/workspace-library-20261021','previous_release':'/opt/understand-book/releases/reader-latency-20261021',
 'files':changes,'windows_version':'0.3.4','excluded_unrelated_changes':['automatic reading resumption','additional presentation test modules','preparation-source formatting']}
(stage/'source.json').write_text(json.dumps(report,indent=2),'utf-8')
print(json.dumps(report,indent=2))

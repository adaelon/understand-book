from pathlib import Path
import shutil, subprocess, json, tarfile, difflib

repo=Path.cwd(); stage=repo/'tmp/reader-latency-release-20261021'; source=stage/'source'
changed=[]
for label in ['video-catalog-loading-20261021','question-submit-loading-20261021']:
    patch=(repo/'docs/performance'/label/'fix.patch').read_text('utf-8')
    if label=='video-catalog-loading-20261021':
        name='packages/web/src/components/SharedWorkspaces.test.ts'
        start=patch.index('--- a/'+name); end=patch.index('\n--- a/',start+1)
        block=patch[start:end]
        before=(source/name).read_text('utf-8')
        removed=next(line[1:] for line in block.splitlines() if line.startswith('-afterEach'))
        addition='\n'.join(line[1:] for line in block.splitlines() if line.startswith('+') and not line.startswith('+++'))
        assert before.count(removed)==1
        after=before.replace(removed,addition)
        replacement=''.join(difflib.unified_diff(before.splitlines(True),after.splitlines(True),fromfile='a/'+name,tofile='b/'+name))
        patch=patch[:start]+replacement+patch[end+1:]
    target=stage/(label+'.patch'); target.write_text(patch,encoding='utf-8',newline='\n')
    command=['git','apply','--unsafe-paths','--directory='+source.relative_to(repo).as_posix()]
    subprocess.run(command+['--check',str(target)],check=True)
    subprocess.run(command+[str(target)],check=True)
    changed.extend(line[6:] for line in patch.splitlines() if line.startswith('+++ b/'))
for name in ['packages/web/src/components/AgentPresentation.test.ts','packages/web/src/presentation-document.test.ts']:
    path=source/name
    path.write_text(path.read_text('utf-8').replace('animation_assets: {},','animation_assets: {}, action_contract: [],'),encoding='utf-8',newline='\n')
    changed.append(name)
with tarfile.open(stage/'source-overlay.tar.gz','w:gz') as archive:
    for name in sorted(set(changed)):archive.add(source/name,arcname=name)

# Native code comes from the already published 0.3.2 snapshot.
windows=stage/'windows-source'; old=repo/'tmp/windows-release-20261021/source'
shutil.copytree(old,windows,ignore=shutil.ignore_patterns('node_modules','local-reader','dist','target'))
for name in ['apps/desktop/package.json','apps/desktop/src-tauri/Cargo.toml','apps/desktop/src-tauri/tauri.conf.json','Cargo.lock']:
    path=windows/name; value=path.read_text('utf-8')
    if name=='Cargo.lock':value=value.replace('name = "understand-book-desktop"\nversion = "0.3.2"','name = "understand-book-desktop"\nversion = "0.3.3"')
    else:value=value.replace('"0.3.2"','"0.3.3"')
    path.write_text(value,encoding='utf-8',newline='\n')
# The embedded local library uses the released web source plus this release's fixes.
for directory in ['packages/web','packages/core']:
    shutil.copytree(source/directory,windows/directory,dirs_exist_ok=True)
report={'previous_release':'/opt/understand-book/releases/search-cache-20261021','release':'/opt/understand-book/releases/reader-latency-20261021','changed_files':sorted(set(changed)),'windows_version':'0.3.3','native_baseline':str(old),'patches':['video-catalog-loading-20261021','question-submit-loading-20261021']}
(stage/'source.json').write_text(json.dumps(report,ensure_ascii=False,indent=2),'utf-8')
print(json.dumps(report,ensure_ascii=False,indent=2))

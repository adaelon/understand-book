from pathlib import Path
import shutil,json,tarfile
stage=Path(__file__).parent
for name in ['src/components/CloudPlans.test.ts','src/personal-preparations.test.ts']:
    shutil.copy2(Path('packages/web')/name,stage/'source/packages/web'/name)
native=stage/'windows-source/apps/desktop/src-tauri'
main=(native/'src/main.rs').read_text('utf-8')
for module in ['cloud_auth','local_materials','reading_share']:
    main=main.replace(f'mod {module};',f'#[path = "../src/{module}.rs"] mod {module};')
main=main.replace('app.path().app_local_data_dir()?','std::path::PathBuf::from(std::env::var_os("UB_RELEASE_PROBE_DIR").expect("isolated probe directory"))')
main=main.replace('.title("Understand Book")','.visible(false).title("Understand Book")')
(native/'examples/desktop-release-probe.rs').write_text(main,'utf-8')
for name in ['probe.mjs','probe-published.mjs','verify-public.py']:
    content=(Path('tmp/reader-latency-release-20261021')/name).read_text('utf-8').replace('reader-latency-release-20261021','workspace-library-release-20261021').replace('dist-latency','dist-release').replace('v=0.3.3','v=0.3.4')
    (stage/name).write_text(content,'utf-8')
web=stage/'source/packages/web/dist-release'
files=[p.relative_to(web).as_posix() for p in web.rglob('*') if p.is_file()]
(stage/'static.json').write_text(json.dumps({'files':files,'bytes':sum((web/p).stat().st_size for p in files)},indent=2),'utf-8')
with tarfile.open(stage/'static.tar.gz','w:gz') as archive:
    for name in files:archive.add(web/name,arcname='packages/web/dist-multi/'+name)
    archive.add(stage/'windows-source/dist/UnderstandBookSetup.exe',arcname='downloads/UnderstandBookSetup.exe')
deploy=Path('tmp/reader-latency-release-20261021/deploy.py').read_text('utf-8')
deploy=deploy.replace('reader-latency-20261021','workspace-library-20261021').replace('search-cache-20261021','reader-latency-20261021')
(stage/'deploy.py').write_text(deploy,'utf-8')
shell=Path('tmp/reader-latency-release-20261021/deploy.sh').read_text('utf-8').replace('reader-latency-20261021','workspace-library-20261021')
(stage/'deploy.sh').write_text(shell,'utf-8',newline='\n')
print(json.dumps({'static_files':len(files),'archive_bytes':(stage/'static.tar.gz').stat().st_size}))

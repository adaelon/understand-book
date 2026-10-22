from pathlib import Path
import shutil, json

repo = Path.cwd()
root = repo/'tmp/windows-release-20261021'
dest = root/'source'
assert not dest.exists(), 'Candidate already exists; do not overwrite it.'
old = repo/'tmp/local-library-20261021/frontend'
shutil.copytree(old/'packages/web', dest/'packages/web', ignore=shutil.ignore_patterns('node_modules','dist','dist-multi'))
shutil.copytree(old/'packages/core', dest/'packages/core', ignore=shutil.ignore_patterns('node_modules','dist'))
for relative in ['package.json','tsconfig.json']:
    shutil.copy2(old/relative, dest/relative)
# These three desktop entry files were introduced with the published 0.3.1.
for relative in ['desktop.html','src/desktop-main.ts','src/components/DesktopLibrary.vue']:
    source = repo/'packages/web'/relative
    assert source.stat().st_mtime < (repo/'dist/UnderstandBookSetup.exe').stat().st_mtime
    shutil.copy2(source, dest/'packages/web'/relative)
native = repo/'apps/desktop/src-tauri'
for relative in ['src','icons','capabilities','permissions','windows','examples']:
    shutil.copytree(native/relative, dest/'apps/desktop/src-tauri'/relative)
for relative in ['Cargo.toml','build.rs','tauri.conf.json']:
    shutil.copy2(native/relative, dest/'apps/desktop/src-tauri'/relative)
for file in (dest/'apps/desktop/src-tauri').rglob('*'):
    if file.is_file() and 'permissions' not in file.parts:
        assert file.stat().st_mtime < (repo/'dist/UnderstandBookSetup.exe').stat().st_mtime, str(file)
(dest/'apps/desktop/scripts').mkdir(parents=True)
for relative in ['assert-release-config.mjs','build-local-reader.mjs','export-installer.mjs']:
    shutil.copy2(repo/'apps/desktop/scripts'/relative, dest/'apps/desktop/scripts'/relative)
shutil.copy2(repo/'apps/desktop/package.json', dest/'apps/desktop/package.json')
shutil.copy2(repo/'Cargo.lock', dest/'Cargo.lock')
(dest/'Cargo.toml').write_text('[workspace]\nresolver = "2"\nmembers = ["apps/desktop/src-tauri"]\n', encoding='utf-8')
for relative in ['apps/desktop/package.json','apps/desktop/src-tauri/Cargo.toml','apps/desktop/src-tauri/tauri.conf.json','Cargo.lock']:
    file = dest/relative
    value = file.read_text(encoding='utf-8')
    if relative == 'Cargo.lock':
        value = value.replace('name = "understand-book-desktop"\nversion = "0.3.1"', 'name = "understand-book-desktop"\nversion = "0.3.2"')
    else:
        value = value.replace('"0.3.1"', '"0.3.2"')
    file.write_text(value, encoding='utf-8')
shutil.copytree(repo/'apps/desktop/local-reader', root/'previous-local-reader')
shutil.copy2(repo/'dist/UnderstandBookSetup.exe', root/'UnderstandBookSetup-0.3.1.exe')
(root/'source.json').write_text(json.dumps({
    'version':'0.3.2', 'native_baseline':'0.3.1 native sources, all input mtimes precede the published 0.3.1 build',
    'web_baseline':str(old), 'desktop_entry_files':['desktop.html','src/desktop-main.ts','src/components/DesktopLibrary.vue'],
    'runtime_changes':'version only; cloud features use current production Reader',
    'cloud_origin':'https://understandbook.top', 'server_changes':False,
    'required_gate':'Rebuilt embedded Reader must match every 0.3.1 asset byte before packaging is accepted.'
}, ensure_ascii=False, indent=2), encoding='utf-8')
print(dest)

from pathlib import Path
import json,subprocess,tarfile
stage=Path('/opt/understand-book/acceptance/main-agent-planned-20261029');resources=json.loads((stage/'build-resources.json').read_text());release=Path(resources['release'])
run=lambda *args:subprocess.check_output(list(map(str,args)),text=True).strip()
assert (stage/'build.exit').read_text().strip()=='0'
assert run('systemctl','show',resources['unit'],'-p','ActiveState','--value')=='inactive'
with tarfile.open(stage/'source.tar.gz') as archive:
    for entry in archive.getmembers():assert (release/entry.name).read_bytes()==archive.extractfile(entry).read(),entry.name
swap=Path(resources['temporary_swap'])
if str(swap) in [line.split()[0] for line in Path('/proc/swaps').read_text().splitlines()[1:]]:run('swapoff',swap)
if swap.exists():swap.unlink()
run('sysctl','-w','vm.swappiness='+str(resources['original_swappiness']))
binary=release/'target/release/server'
report={'status':'built','release':str(release),'source_files_match':True,
 'binary_stat':{'size':binary.stat().st_size,'mtime_ns':binary.stat().st_mtime_ns},'temporary_swap_removed':True,'original_swappiness_restored':int(Path('/proc/sys/vm/swappiness').read_text())==resources['original_swappiness'],
 'last_build_line':(stage/'build.log').read_text(errors='replace').splitlines()[-1]}
(stage/'build-completion.json').write_text(json.dumps(report,indent=2));print(json.dumps(report))

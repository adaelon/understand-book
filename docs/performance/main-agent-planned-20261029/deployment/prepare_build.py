from pathlib import Path
import json,os,shutil,subprocess,tarfile
base=Path('/opt/understand-book');stage=base/'acceptance/main-agent-planned-20261029'
old=base/'releases/reader-outline-20261029';new=base/'releases/main-agent-planned-20261029'
run=lambda *args:subprocess.check_output(list(map(str,args)),text=True).strip()
assert run('systemctl','show','understand-book-multi','-p','WorkingDirectory','--value')==str(old)
assert not new.exists();assert shutil.disk_usage(base).free>5*1024**3
shutil.copytree(old,new,copy_function=os.link,symlinks=True)
binaries=['server','manage_reader','publish_book','reader_maintenance','presentation_worker','video_audio_review']
for name in binaries:
    p=new/'target/release'/name
    if p.exists():p.unlink()
for prefix in ['memory','runtime',*binaries]:
    for p in (new/'target/release/deps').glob('*'+prefix.replace('-','_')+'-*'):
        if p.is_file():p.unlink()
for directory in (new/'target/release/.fingerprint').iterdir():
    if directory.is_dir():
        for p in directory.iterdir():
            if p.is_file():data=p.read_bytes();p.unlink();p.write_bytes(data)
with tarfile.open(stage/'source.tar.gz') as archive:
    for entry in archive.getmembers():
        p=new/entry.name;assert p.is_relative_to(new) and entry.isfile()
        p.parent.mkdir(parents=True,exist_ok=True)
        if p.exists():p.unlink()
        p.write_bytes(archive.extractfile(entry).read())
for name in json.loads((stage/'release-source.json').read_text())['removed']:
    (new/name).unlink(missing_ok=True)
swap=stage/'build.swap';swappiness=int(Path('/proc/sys/vm/swappiness').read_text())
script=f'''#!/bin/bash
set -euo pipefail
trap 'printf "%s\\n" "$?" > {stage}/build.exit' EXIT
export CARGO_HOME=/opt/understand-book/tools/cargo
export RUSTUP_HOME=/opt/understand-book/tools/rustup
export PATH="$CARGO_HOME/bin:$PATH"
export TMPDIR={stage}/build-tmp
mkdir -p "$TMPDIR"
cd {new}
cargo --config 'profile.release.package.server.codegen-units=64' build --locked --release -j 1 -p server '''+' '.join('--bin '+b for b in binaries)+'\n'
(stage/'build.sh').write_text(script)
resources={'unit':'ub-main-agent-planned-20261029-build','original_swappiness':swappiness,'temporary_swap':str(swap),'release':str(new),'previous_release':str(old),'binaries':binaries,
 'cpu_quota':'150%','memory_high':'1100M','memory_max':'1400M'}
(stage/'build-resources.json').write_text(json.dumps(resources,indent=2))
run('fallocate','-l','2G',swap);swap.chmod(0o600);run('mkswap',swap);run('swapon',swap);run('sysctl','-w','vm.swappiness=60')
run('systemd-run','--unit='+resources['unit'],'--property=MemoryHigh=1100M','--property=MemoryMax=1400M','--property=MemorySwapMax=2G',
 '--property=CPUQuota=150%','--property=Nice=19','--property=IOWeight=10',
 '--property=StandardOutput=append:'+str(stage/'build.log'),'--property=StandardError=append:'+str(stage/'build.log'),'/bin/bash',stage/'build.sh')
print(json.dumps(resources))

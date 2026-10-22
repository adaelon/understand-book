from pathlib import Path
import difflib, json, sys

ROOT = Path(__file__).resolve().parents[3]
OUT = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT / 'tmp/mu12'))
from remote import connect

REMOTE = '/opt/understand-book/acceptance/video-catalog-loading-20261021'
changed = ['crates/base-schema/src/video.rs', 'crates/read-tools/src/video_graph.rs',
           'crates/read-tools/src/video_discourse.rs', 'crates/server/src/workspace_publication.rs']
patch = ''.join(''.join(difflib.unified_diff(
    (OUT / 'before' / rel).read_text(encoding='utf-8-sig').splitlines(True),
    (ROOT / rel).read_text(encoding='utf-8-sig').splitlines(True),
    fromfile='a/' + rel, tofile='b/' + rel)) for rel in changed)
(OUT / 'candidate.patch').write_text(patch, encoding='utf-8')
code = r'''
from pathlib import Path
import json, os, shutil, subprocess
out=Path('/opt/understand-book/acceptance/video-catalog-loading-20261021')
live=Path(subprocess.check_output(['systemctl','show','understand-book-multi','--property=WorkingDirectory','--value'],text=True).strip())
source=out/'source'
source.mkdir(exist_ok=True)
for name in ['read-tools','base-schema','book-tool-contracts']:
    shutil.copytree(live/'crates'/name,source/'crates'/name,dirs_exist_ok=True)
(source/'crates/server/src').mkdir(parents=True,exist_ok=True)
shutil.copy2(live/'crates/server/src/workspace_publication.rs',source/'crates/server/src/workspace_publication.rs')
(source/'Cargo.toml').write_text('[workspace]\nresolver="2"\nmembers=["crates/read-tools","crates/base-schema","crates/book-tool-contracts"]\n'.replace('\\n','\n'))
shutil.copy2(live/'Cargo.lock',source/'Cargo.lock')
with (source/'crates/read-tools/Cargo.toml').open('a') as f:
    f.write('\n[dev-dependencies]\nrusqlite={version="0.40.2",features=["bundled"]}\nserde_json={version="1",features=["arbitrary_precision"]}\n'.replace('\\n','\n'))
publication=(live/'crates/server/src/published_library.rs').read_text()
publication=publication[publication.index('#[derive(Debug, Clone, Serialize, Deserialize)]'):publication.index('pub struct PublishedBook {')]
intro='use base_schema::PublishedBookRef;\nuse serde::{Serialize,Deserialize};\nuse serde_json::{json,Value};\nuse std::{collections::{BTreeMap,BTreeSet},path::Path};\nuse rusqlite::{Connection,OptionalExtension,params};\nuse read_tools::ToolError;\n'.replace('\\n','\n')
intro+=publication+'\nstruct Material {published_book_ref:PublishedBookRef,required_capabilities:Vec<String>}\n'.replace('\\n','\n')
env=dict(os.environ,CARGO_HOME='/opt/understand-book/tools/cargo',RUSTUP_HOME='/opt/understand-book/tools/rustup')
env['PATH']=env['CARGO_HOME']+'/bin:'+env.get('PATH','')
course='/opt/understand-book/data/multi-reader-to-tb-20261012/library/published/19ac880d-a788-43f4-9725-6c56f4980189/01a15c89-da55-7d71-8329-a0ba9f25c86b'
summaries=[];expected=None
for phase in ['before','after']:
    if phase=='after':
        for rel in ['crates/base-schema/src/video.rs','crates/read-tools/src/video_graph.rs','crates/read-tools/src/video_discourse.rs','crates/server/src/workspace_publication.rs']:
            p=source/rel;p.write_text(p.read_text())
        subprocess.run(['patch','-p1','--fuzz=0','-i',str(out/'candidate.patch')],cwd=source,check=True)
    text=(source/'crates/server/src/workspace_publication.rs').read_text()
    body=text[text.index('pub(crate) fn material_readiness('):text.index('\nfn readiness(')]
    (source/'crates/read-tools/examples/catalog_probe.rs').write_text(intro+body+'\n'+(out/'linux-probe-main.rs').read_text())
    with (out/(phase+'-build.log')).open('w') as log:
        subprocess.run([env['CARGO_HOME']+'/bin/cargo','build','--offline','--release','-j','1','-p','read-tools','--example','catalog_probe'],cwd=source,env=env,stdout=log,stderr=subprocess.STDOUT,check=True)
    print(phase+' compiled',flush=True)
    for index in range(2):
        result=subprocess.run([str(source/'target/release/examples/catalog_probe'),course],capture_output=True,timeout=180,check=True)
        (out/(phase+'-'+str(index+1)+'.json')).write_bytes(result.stdout)
        data=json.loads(result.stdout)
        summary={k:data.pop(k) for k in ['catalog_ms','catalog_read_bytes','book_ms','book_read_bytes']}
        summary.update(phase=phase,run=index+1)
        if expected is None:expected=data
        assert data==expected,'readiness, graph, transcript, or search changed'
        summary['outputs_equal']=True;summaries.append(summary);print(json.dumps(summary),flush=True)
(out/'comparison.json').write_text(json.dumps(summaries,indent=2)+'\n')
'''
c = connect()
try:
    c.exec_command('mkdir -p ' + REMOTE)[1].channel.recv_exit_status()
    with c.open_sftp() as s:
        s.put(str(OUT / 'candidate.patch'), REMOTE + '/candidate.patch')
        s.put(str(OUT / 'linux-probe-main.rs'), REMOTE + '/linux-probe-main.rs')
    stdin, stdout, stderr = c.exec_command('python3 -u -', timeout=900)
    stdin.write(code); stdin.channel.shutdown_write()
    for line in stdout: print(line, end='', flush=True)
    print(stderr.read().decode(), end='', file=sys.stderr)
    result = stdout.channel.recv_exit_status()
    with c.open_sftp() as s:
        for name in ['before-build.log', 'after-build.log', 'comparison.json', 'before-1.json', 'after-1.json']:
            try: s.get(REMOTE + '/' + name, str(OUT / ('linux-' + name)))
            except FileNotFoundError: pass
    sys.exit(result)
finally:
    c.close()

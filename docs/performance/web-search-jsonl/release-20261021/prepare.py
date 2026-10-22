from pathlib import Path
import difflib, json, re, shutil, tarfile

stage=Path(__file__).parent
base=Path('tmp/cloud-release-20261021/source')
dest=stage/'source'
if not dest.exists():
    shutil.copytree(base,dest,ignore=shutil.ignore_patterns('node_modules','target','dist','dist-multi','.git'))
files=[]; excluded=[]; patches=[]
for entry in json.loads((stage/'scope.json').read_text()):
    name=entry['file']
    baseline=stage/'online-source'/name if (stage/'online-source'/name).exists() else base/name
    old=baseline.read_text(encoding='utf-8-sig') if baseline.exists() else ''
    current=Path(name).read_text(encoding='utf-8-sig')
    if name=='crates/server/src/tests/tutor_example_tests.rs':
        current=current.replace('        let request = example["move"]["intent_quote"].as_str().unwrap_or("Help me examine average speed.");\n        let result = json_post(&mut state, "/agent/chat", json!({"message":request}));', '        let result = json_post(&mut state, "/agent/chat", json!({"message":"Help me examine average speed."}));')
        excluded.append({'file':name,'reason':'Preserve the actual deployed fixture question; local baseline contained an additional test-only adjustment.'})
    if name=='crates/runtime/src/agent_prompt.rs':
        current=''.join(line for line in current.splitlines(True) if not line.startswith('- book.text may supply math_context:'))
        current=current.replace(', or math_context locations returned by a completed source read','')
        current=current.replace('EVIDENCE_ROUTING_REVISION: &str = "v6"','EVIDENCE_ROUTING_REVISION: &str = "v5"').replace('Some("v6")','Some("v5")')
        excluded.append({'file':name,'reason':'Preserve deployed evidence-routing v5; exclude mathematical-structure instructions.'})
    diff=list(difflib.unified_diff(old.splitlines(True),current.splitlines(True),fromfile='a/'+name,tofile='b/'+name))
    hunks=[]
    for line in diff[2:]:
        if line.startswith('@@'):hunks.append([])
        hunks[-1].append(line)
    selected=[]
    for hunk in hunks:
        additions=''.join(line for line in hunk if line.startswith('+'))
        keep=True
        if name in ['crates/runtime/src/lib.rs','crates/runtime/src/orchestrator.rs','crates/runtime/src/tool_result.rs','crates/runtime/src/tutor.rs']:
            keep=not any(marker in additions for marker in ['math_context','math_structure','scope_math_read_calls'])
        if name=='crates/server/src/workspace_registry.rs':keep='fn web_available' in additions
        if name=='crates/runtime/src/tool_registry.rs' and 'presentation.interact' in additions and 'Web' not in additions:keep=False
        if keep:selected.extend(hunk)
        else:excluded.append({'file':name,'hunk':hunk[0].strip(),'reason':'Unpublished mathematical-structure, reading-resumption, or unrelated test change.'})
    if not selected:continue
    original=old.splitlines(True); result=[]; cursor=0
    for line in selected:
        if line.startswith('@@'):
            start=max(0,int(re.match(r'@@ -(\d+)',line)[1])-1);result.extend(original[cursor:start]);cursor=start
        elif line.startswith(' '):result.append(line[1:]);cursor+=1
        elif line.startswith('-'):cursor+=1
        elif line.startswith('+'):result.append(line[1:])
    result.extend(original[cursor:])
    output=dest/name;output.parent.mkdir(parents=True,exist_ok=True);output.write_text(''.join(result),encoding='utf-8',newline='\n')
    files.append(name);patches.extend(diff[:2]+selected)

# The current live Reader also contains the already-published 0.3.1 web changes.
local_library=Path('tmp/local-library-20261021/frontend/packages/web/src')
published=json.loads(Path('docs/performance/windows-local-materials-20261021/source.json').read_text())['changed']
preserved=[]
for relative in published:
    name='packages/web/src/'+relative
    assert name not in files
    output=dest/name;output.parent.mkdir(parents=True,exist_ok=True)
    shutil.copyfile(local_library/relative,output);preserved.append(name)

(stage/'overlay.patch').write_text(''.join(patches),encoding='utf-8',newline='\n')
with tarfile.open(stage/'source-overlay.tar.gz','w:gz') as archive:
    for name in files+preserved:archive.add(dest/name,arcname=name)
manifest={'previous_release':'/opt/understand-book/releases/cloud-20261021','release':'/opt/understand-book/releases/search-cache-20261021',
 'files':files,'preserved_published_frontend_files':preserved,'excluded':excluded,
 'scope':'Search/Fetch, shared-budget service accounting and cost UI, stable history tool names, TurnFrozen context placement, stable Tutor references.',
 'schema_before':19,'schema_after':20,'desktop_installer':'Keep the existing published 0.3.1 bytes.'}
(stage/'source.json').write_text(json.dumps(manifest,ensure_ascii=False,indent=2),encoding='utf-8')
print(json.dumps({'files':len(files),'preserved_frontend':len(preserved),'excluded_hunks':len(excluded),'archive_bytes':(stage/'source-overlay.tar.gz').stat().st_size},ensure_ascii=False))

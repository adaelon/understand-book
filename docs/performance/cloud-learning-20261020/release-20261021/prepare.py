from pathlib import Path
import difflib,json,re,shutil,subprocess,tarfile
S=Path(__file__).parent
BASE=Path('tmp/om-release-20261020/verification-source')
N=S/'source'
shutil.copytree(BASE,N,dirs_exist_ok=True,ignore=shutil.ignore_patterns('node_modules','target','dist','.git'))
full=['Cargo.lock','apps/desktop/src-tauri/Cargo.toml','packages/web/src/home.ts']
full += ['crates/runtime/src/'+x for x in ['delivery_fallback.rs','goal.rs','workspace_goal.rs','workspace_goal_tests.rs','tutor_review_rules.md']]
full += ['crates/server/src/'+x+'.rs' for x in ['account_allowance','admin_api','admin_store','agent_run','allowance_admin','authorization','charge_admin','control_store','cost_summary','external_build_costs','learning_workspace','lib','model_spend_store','operator_builds','personal_preparations','presentation_store','published_library','run_admission','user_runtime','video_audio','workspace_publication','workspace_release_instances']]
full += [p.as_posix() for p in Path('crates/server/src').glob('*') if p.name.startswith(('cloud_','course_access.','desktop_auth.','exam_sprints.','service_usage.'))]
full += ['crates/server/src/agent_run_tests.rs']
full += [p.as_posix() for p in Path('crates/server/src/tests').glob('*.rs') if p.name.startswith(('cloud_','c4_','b2_','e1_','s1_','s2_','presentation_','teaching_','tutor_','w4_','w6_'))]
full += ['crates/server/src/tests/remove_schema19.sql']
full += [p.as_posix() for p in Path('apps/admin/src').rglob('*') if p.is_file()]
full += ['packages/web/src/'+x for x in ['App.vue','account-allowance.ts','personal-preparations.ts','shared-workspaces.ts','cloud-plans.ts','desktop-auth.ts','exam-sprints.ts','components/AccountAllowance.vue','components/CloudPlans.vue','components/DesktopAuthorization.vue','components/DesktopSignIn.vue','components/ExamSprintPanel.vue','components/PersonalPreparations.vue','components/SharedWorkspaceCatalog.vue','components/SharedWorkspaceManifest.vue','components/VideoReaderPane.vue','components/ProductHome.vue']]
mixed=['crates/runtime/src/orchestrator.rs','crates/runtime/src/library.rs','crates/server/src/auth.rs','crates/server/src/multi_user_host.rs','crates/server/src/workspace_registry.rs','packages/web/src/NetworkApp.vue']
files=[];skipped=[];patch=[]
for name in dict.fromkeys(full+mixed):
 old=(BASE/name).read_text(encoding='utf-8-sig') if (BASE/name).exists() else ''
 current=Path(name).read_text(encoding='utf-8-sig')
 if name.endswith('tutor_review_rules.md'):current=''.join(l for l in current.splitlines(True) if not l.startswith('若来源附有 math_context'))
 if name.endswith('/authorization.rs'):current=''.join(l for l in current.splitlines(True) if 'ReadingResumption' not in l)
 if name.endswith('/auth.rs'):
  current=current.replace('SESSION_SECONDS: i64 = 30 * 24 * 60 * 60','SESSION_SECONDS: i64 = 12 * 60 * 60')
  current=re.sub(r'    #\[test\]\n    fn reading_login_survives_an_overnight_break_and_expires_after_thirty_days\(\) \{.*?\n    \}\n\n','',current,flags=re.S)
 if name.endswith('/NetworkApp.vue'):
  current=current.replace('workspaceStorageKey, savedWorkspace, saveWorkspace, type','workspaceStorageKey, type')
 diff=list(difflib.unified_diff(old.splitlines(True),current.splitlines(True),fromfile='a/'+name,tofile='b/'+name))
 if name in mixed:
  hunks=[]
  for line in diff[2:]:
   if line.startswith('@@'):hunks.append([])
   hunks[-1].append(line)
  selected=[]
  for h in hunks:
   adds=''.join(l for l in h if l.startswith('+'))
   keep=True
   if name.endswith('/orchestrator.rs'):keep=not any(x in adds for x in ['math_context','source_range','math_structure_flow'])
   if name.endswith('/library.rs'):keep='WORKSPACE_CATALOG_POLICY' in adds
   if name.endswith('/multi_user_host.rs'):keep='Capability::ReadingResumption' not in adds
   if name.endswith('/workspace_registry.rs'):keep='review_audio_metered' in adds
   if name.endswith('/NetworkApp.vue'):keep=not any(x in adds for x in ['savedWorkspace','saveWorkspace','restoreLastVisit','restoreOnLoad'])
   if keep:selected.extend(h)
   else:skipped.append({'file':name,'hunk':h[0].strip()})
  diff=diff[:2]+selected if selected else []
  source=old.splitlines(True);result=[];cursor=0
  for l in diff[2:]:
   if l.startswith('@@'):
    start=int(re.match(r'@@ -(\d+)',l)[1])-1;result.extend(source[cursor:start]);cursor=start
   elif l.startswith(' '):result.append(l[1:]);cursor+=1
   elif l.startswith('-'):cursor+=1
   elif l.startswith('+'):result.append(l[1:])
  result.extend(source[cursor:]);current=''.join(result)
 if not diff:continue
 dest=N/name;dest.parent.mkdir(parents=True,exist_ok=True);dest.write_text(current,encoding='utf-8',newline='\n')
 files.append(name);patch.extend(diff)
(S/'overlay.patch').write_text(''.join(patch),encoding='utf-8',newline='\n')
with tarfile.open(S/'cloud-overlay.tar.gz','w:gz') as archive:
 for name in files:archive.add(N/name,arcname=name)
manifest={'previous_release':'/opt/understand-book/releases/om6-20261020','release':'/opt/understand-book/releases/cloud-20261021','base_commit':'33c19c7fa8aec9a4f1031122bc177743b234b9fd','files':files,'excluded_unrelated_hunks':skipped,'commercial_configuration':'unconfigured','courses':'unconfigured'}
(S/'source.json').write_text(json.dumps(manifest,indent=2))
print(json.dumps({'files':len(files),'bytes':(S/'cloud-overlay.tar.gz').stat().st_size,'excluded':skipped},indent=2))

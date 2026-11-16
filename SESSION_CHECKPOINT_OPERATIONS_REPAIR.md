# COMPLETED — 2026-11-09 14:12 HKT

All authorized operational remediation is completed. Material rebuilding remains explicitly deferred by user; do NOT start it or ask again without new instruction.

- Live release operations-remediation-r2-20261109, schema24. Video search->batch text read->source present actualpass; board preview/save andfreshloginreadbackpass. Currentserviceactive/noautorestarts, model/webready, timersactive/enabled, noactiveincidents ormailqueue.
- Exactly472historicalholds128.654186CNYreleasedviaauditedAPI; suppliercostunknownretained/platformowned. Webpricesunchanged, coverageuntil2026-12-09 08:00HKT. AIInfra completeversionalreadyauthorized,0grantschanged.
- LatestOFFSITE backup 1cd7750f94f92d9ce6419ccdecc774dd737bb238e8ffe40ff7615c38e971fb49 completed14:08:44HKT, local/opt/understand-book/backups/business/1794204109, capturesR2+latestdata+recoveryconfig. Two validbusinesssnapshots exist; seedremoved. Keep2local/7remote; dailywhenidle(hourlyattemptskip<23h). Externalmonitor everyminuteold115.190.121.150;oldservermustremainrunning.
- LATEST actualrestoredalluser/control/config+quickstart:1186files/320790331bytes,47DBs,60controltables,1095JSONfiles; exacthistory/latestboard/sourcebytes. Restoreisolatedoldacceptance/operations-restore-20261109/restored-1cd7750f94f9. NotafullOSrestore. OldPythonSQLite3.34can'tSTRICT; usedexistingNodeSQLite3.53.4. Latestrestore-verification.jsonpassed.
- Latestfinal-health.jsonpassed:primaryfree16.44GiB,oldfree3.98GiB;2local/2offsite snapshots;all3timersactive/enabled;pendingheld0;model/webready;noerrors/mailqueue.
- Allfunctionalitytestscomplete, no morerepeatsneeded. Whole-reposuiteNOT run. Thisroundthreeactualmodelprobes0.193792CNY,allsettled/adminvalidityrestored.
- Finaldocsupdated: docs/performance/operations-audit-20261109/{README.md,remediation.json}, docs/Linux多人阅读-现网运行说明.md. Private receipts tmp/operations-remediation-20261109/receipts;restore/backup/final-health filesadjacent. Recoverykeys E:/allwork/backups/understand-book-recovery (privateACL);NEVERprintsecrets.
- Localorchestratorsession55480completed(logfinish-backups-r2.log endsBACKUP_WORK_COMPLETE). Latestrestoreexec9675completedpass. No further backgroundmanualworkrequired;scheduledservicesintentionallyactive.
- No gitcommitrequested/made;unrelateddirtytreepreserved. Finaluserresponsepending: Chinese resultselfcontained,financialrelease,video/board/scopedalerts/rates/backuprestore,materialdefer,AIInfra correction,oldnodeoperationaldependency,linkdocs. Do notclaimfutureeverythingbugfree.

--- historical notes below ---

## NEWEST — backup first restore passed; final transfer nearing completion
- FIRST business snapshot f767f8e51ec7b9415a8cda27d377163f3fccf1e8b84f91a3cdec8b2fb7815c53 COMPLETED after45min, 59,392files/9,922,843,264rawbytes. First restored actualallusers/control/config/samplebook successfully. OldPython SQLite3.34.1 couldnotread STRICTschema; fixedchecker toexistingoldNode24.20(SQLite3.53.4), no DBdatachange. PASS:1179userfiles/319305807bytes,47SQLiteDBs,59controltables,1088JSONfiles,byteequalhistory+boardhead+sample source. Evidence tmp/.../restore-first-verification.json and restore-verification.json.
- First local orchestration session82824 FAILED ONLY dueoldSQLitechecker; replaced by ACTIVE session **55480**, script finish_backups.py, log **finish-backups-r2.log**. It has passedrestore, prunedbootstrap seed locally(oldoriginaldatauntouched), uploadedlatestbackupscript, started finalbackupPID209584.
- Final local snapshot **/opt/understand-book/backups/business/1794204109**, capturefinished1794204165, maintenance53.446sec (website restored). Finaltransfer log operations/backup-transfer.log (liveJSONminuteprogress) reached100%/59,441files/9,924,361,532bytes at180sec, stillflushingat240sec lastcheck. DO NOTstartduplicate. Orchestratorwillenableboth timers/checkexternalwhen offsitecomplete andwritebackup-final-verification.json.
- Afterorchestratorcompletes, run `python tmp/operations-remediation-20261109/restore_drill.py` ONCEagain toverifylatest schema24+newprobe records; script now uses per-snapshot target restored-{id12}, so won'treuse firstdata. Thissecondrestorecheapoldlocal~seconds and justifiednewdata. It writeslatestrestore-verification.json; firstproofpreservedrestore-first-verification.json. Primaryexpected uses smallnonemptyhistorysample +actualboardhead +sample sourcebytecompare; alluser+samplefilelistsizes and47DBs/JSONverify.
- Needfinalhealth/timers/retention readback; update docs remediation.json backup_restorepending withfinalbackup+latestrestore+monitorhealth, READMEbackupsection,runbookactualbackupinfo(+NodeSQLitecompat note). ThenfinalChineseuserreport. AllfunctionalfixesalreadydeployedR2andpassed. Allmaterialbuildsdeferred. No morepaidcalls.
- Lastcommentary: 最新快照的文件处理已全部完成，正在完成异地写入。我会等到仓库返回成功记录，再核对恢复结果和定时任务状态。

## Latest pending workflow (read first)
- Background LOCAL orchestrator `python -u tmp/operations-remediation-20261109/finish_backups.py > tmp/operations-remediation-20261109/finish-backups.log 2>&1` is ACTIVE exec session **82824**. It waits firstbusinessbackup, executes restore_drill.py, prunes seed by explicit queried seed snapshot IDs, uploads latestbackup script, starts final --force backup, waits completion, validates R2+normalnginx+keys, enables both timers and checks external actual status, writes tmp/.../backup-final-verification.json. Do NOT start duplicate! Original session8921 process24564 was stopped BEFOREfirstbackup to correct seedprune; current82824 has correct logic.
- Firstremote restic206371 stillactive, copied through /tools/render-runtime/browsers/google-chrome, rawphysicalread~7.7GB out~9.5GB sources, transfer~1GB, noerror aslastcheck. Slow4-5Mbps. No moremodelcalls/materialbuilds.
- Latest localbackup script differs frominstalled until orchestratoruploads it: streams restic JSON to operations/backup-transfer.log (outsidefrozen source), RESTIC_PROGRESS_FPS one/min; excludeslog fromsnapshot. Syntaxchecked. This enablesprogress forfinalbackup.
- Docs both updated: runbook currentR2/schema24 + backuprestore section, auditfinalpublication/proofs, sanitized remediation.json created. remediation.json backup_restore stillpending; need fill afterorchestratorcomplete. Threefullremediationmodelprobes TOTAL0.193792CNY (0.091892 firstvideo overview+0.052139 targetedvideo+0.049761board). Allcompleted/saved/settled; usefinal-live-verification correctedtest not initialvideoassertionfail. Targetedtests counts inremediation.json.
- collect_receipts.py session55876 COMPLETED; private SFTPdownload files in tmp/.../receipts; sanitized docs include noemails/credentials/content.
- Finalremaining: handle actualrestore/backupworkflow failures ifany; aftercomplete finalAPIhealth+primary/externaltimers/status+retention, updateevidence/docs/checkpoint, userfinal Chinese statingactivefixesdone, materialsdeferred, backupdaily/offsite7/local2 + actualrestorelimits, oldservermustremainon. No needrepeatpaidtests or broadrepo tests.
- Last usercommentary: “恢复所需的配置和凭证也已纳入最新备份脚本，凭证另外保存在你本机的私有目录。这样主机出故障时，仍有独立途径访问异地备份。当前首份传输还在继续。”

# CURRENT UPDATE — 2026-11-09 13:32 HKT (supersedes historical notes below)

- User approval unchanged: release 472 holds DONE, all material rebuilding DEFERRED. Continue remaining operational repairs, backup/restore/monitor. No subagents. No commit requested. Chinese communication, meaningful updates ~60sec, no further permission.
- LIVE release now /opt/understand-book/releases/operations-remediation-r2-20261109, schema24, PID207632. R1 published13:04, R2 published13:27:56HKT, maintenance31.883sec. Both deployments verified reader/admin catalog, history, boards, allowance, workspace + model/web ready. R2 backup backups/operations-remediation-r2-deploy-20261109. Records remote acceptance/operations-remediation-20261109/{code-deployment,r2-code-deployment}.json.
- R2 adds successful book.text_batch to video recovery scope; source-scoped library.read. Local targeted saved_text_does_not_publish_board_or_video_recovery_evidence test PASS in batch-scope-test.log. Previous12operations tests + locator + preview loop + watchdog9 + external1passed. preview classification log exists.
- Real HTTPS video targeted turn turn_01a1beb6-4392-77f2-9f72-e442ede2fea4 succeeds video.search -> book.text_batch LID184,193 from exact search text_reads -> source.present x2. Cost0.052139. Initial probe natural overview succeeded via book.search, did NOT exercise video; charges must include it in total. Missing video draft rejection cost0.
- Real board probe turn turn_01a1bebb-045f-77f1-934d-53b69c47e5c5 passes preview+save, cost0.049761. Fresh login readback exact document match PASS afterR2.
- final_live.py completed and wrote remote final-live-verification.json PASS. It parsed actual tool outputs with role='Tool' (case uppercase!), checks originalLIDs, invokes retry-save on already completed video to record recovery with originaltime1794201014; charges unchanged. No repeatmodelneeded. pendingheld0,webready,board/videosuccessscopespresent.
- Primary DIRECT SSH from local began EOF errors; use tunnel via oldhost. Wrapper `python tmp/operations-remediation-20261109/remote_tunnel.py <localremote-script.py>` connects SOURCE old, direct-tcpip toTARGET primary; pins knownkeys. NetworkpublicPython defaultUA gets403, custom monitoringUAworks.
- `collect_receipts.py` currently running exec session55876 downloads private reports to tmp/.../receipts and prints keys. Poll once. Need sanitize into docs evidence, no private emails/content/credentials.

## Backup CURRENT
- First frozen snapshot /opt/understand-book/backups/business/1794199474 (schema23/preR1 but after financial release;18.967secmaintenance) still transferring. ParentPID206370/restic206371 started13:12ish, log acceptance/.../backup-seeded.log; status operations/backup-status.json offsite_at null at13:28. Data transfer~400MB at4-5Mbps afterBBR; do NOT restart again needlessly. Primaryfree16.7GB, oldfree~5GB.
- Encrypted repo on old115.190.121.150 /opt/understand-book/backups/production-restic. Old retired business inactive. Oldsource publiclibrary/models/tools seeded locally with restic0.16.4 in97sec to avoid WANreupload; seed snapshot0f9eaa0d0a08bb9ac6be3d945cff6aff3ceb817cd43fd91e699f65c33d620006, hostretired-public-materials tagseed. Repo3.2GB. After firstBUSINESS status complete forgetseed+prune LOCALLY old, not whileprimaryprunebusy. Preservebusiness/userdata.
- Primary restic installed /usr/bin/restic. Old staticrestic /opt/understand-book/operations-external/restic. Restic password primary operations/backup-password, SSHkey backup_ed25519, pinnedknownhosts, backup-destination.json. Also private recoveryfiles E:/allwork/backups/understand-book-recovery ACLLenovo-only. NEVERprintsecrets.
- Backup production script latest local + installed primary operations + R2release: captures data/users/control/library, executable release, tools/models, full operations (exclude lock/cache), config/env/credentials/systemd units, nginx+letsencrypt+renderpolkit+BBRconfigs. Corrected nginxcopy overwritten with original BEFOREmaintenance so restoring will NOT preserve503. Actual finalforcedcapture still needed to verify this + currentR2/state.
- primaryBBR persisted /etc/sysctl.d/90-understand-book-backup-network.conf + modules-load understand-book-bbr.conf. Noqdiscchange.
- Backup timer installed NOTenabled, hourly attempts skips<23h, keeps2local/7remote, firstsyncimmutablelibrary then idlecheck, plannedadmissionpause+stop+rsync+restart, monitorpause+oldmarker120sec, captures config then release, offsite. Retry pendingtransfer first. --force makes currentnew capture afterprevious completes. Must run finalforcedbackup afterfirstupload; liveidle verified in script. This will capture R2 + newprobes + backupconfiguration.
- External monitoring installed old, NOTenabled: operations-external/external-watchdog.py, operations-watchdog.py, mail.env(allowlisted existingrecipientsettings). Unit/timer installed daemonreload. Sends via existingmailer only when meaningfulincident/recovery.
- Real oldhost externalcheck initially authpassedhomepage403 due defaultPythonUA. Fixed bothrequests same namedmonitorUA; real oldhost HTTPS auth401+homepage200 nowPASS (`update_external.py`). Updated oldinstalled and R2source. No syntheticemailtest.
- `restore_drill.py` PREPARED notrun, uses tunnel/status businesssnapshot gate, restores ALLusers/control/config and quickstartpublication to isolated old acceptance/operations-restore-20261109/restored, validates all user file counts/sizes, SQLitequickcheck, alltablecounts, JSON/JSONLreadability, exactbyte selectedhistory/board sample. Does NOT modifylive. Reports local tmp/.../restore-verification.json and old verification.json. Must execute only afterfirstoffsite_at. Oldrestic runs locally fast; no needpull309MBbackto primary. It samples largehistory and *board*.json; checksampleactualboardpathifneeded. NOfullmachine/serverrestoreclaim.
- Need afterfirstbusiness: run restoredrill; removeseedsnapshot locallyold (passwordpass ephemeralstdin/env, no disk), enableprimarybackup/oldexternal timers afterfinalbackup completion; actual status with no activeincidents/mailerrors. Externalrepositorymtime includesseed untilremoval, so don'tactivatebefore business.

## Docs started, pending final evidence
- docs/performance/operations-audit-20261109/README.md initialaudit kept as historical, appended authorizedfixes, AIInfra correction(0grantmissing), testsactual scope, materialdefer incl AI AgentEngineering plan currentlyrejects oldstagepolicy. Its paragraph says finalpublish/backup evidence pending; replace afterdone.
- Need update docs/Linux多人阅读-现网运行说明.md topcurrentrelease/schema24 and backup/restoremonitor runbook; it has stale '入口与版本' currentrelease/schema22, update that too. Historicaldated sections remain.
- Need sanitized remediation.json evidence incl release/deploypause, releasedcountamountunknownkept, realvideo/boardfreshreadback, allprobeactualcost, backupfinalsnapshot/restore/timers.
- Final Chinese concise but sufficient: allactiveoperationalrepairsdone whentrue, financial128.654186 released, realvalidation, dailybackup/offsite/externalminute, materialsdeferred, AIInfra correction, concrete dependency oldservermustremainon. No claiming entire repo reviewed/fullmachinerestore.
- Last commentary: “最后一处告警修正已构建完成，正在切换上线。异地监测的请求配置也已修正，并从另一台服务器成功检查了正式网站。上线后会用已经保存的视频回合确认恢复记录，不再产生模型用量。”

--- historical notes follow ---

# SESSION_CHECKPOINT_OPERATIONS_REPAIR — 2026-11-09

## Scope / authorization
- User asked fix all operational audit findings. Chinese; autonomous action, no extra approval.
- User explicitly approved release 472 pending holds 128.654186 CNY; unknown supplier cost platform-owned. DONE via API, audit rows preserved.
- User explicitly DEFERRED all material rebuilding after bounded 3-plan question: “暂缓材料补建，先完成其余修复”. Do NOT run builds or ask again.
- No subagents unless skill requests; none used. software-engineering skill read. Build skill read but builds now out of active scope.
- Workspace main very dirty with unrelated work. Preserve all. No git commit requested. No sandbox_permissions.

## Current production
- Host 184.195.172.171, /opt/understand-book, data/production-20261031, schema23.
- Live releases/continuous-source-20261108. Today's board browser env/startup guard already deployed + real probe verified previous turn.
- New candidate releases/operations-remediation-20261109 NOT deployed.
- SSH helper tmp/server-migration-20261031/remote.py; prepend tmp/as9-ssh-deps sys.path. connect(TARGET) pins PuTTY cached hostkey, password from FileZilla XML. Never print secrets.
- Run remote Python: python tmp/alert-diagnosis-20261109/remote_task.py <local-script.py> (600s timeout).
- All new one-off scripts/evidence tmp/operations-remediation-20261109; remote acceptance/operations-remediation-20261109.

## Done
- reconcile_remote.py released exactly472 holds via admin charge reconciliation API, provider_cost null/pending preserved, 472 append-only records. Remote reconciliation.json.
- web rates renewed via fix_configuration.py: keep original periods+5000 microCNY/request, append contiguous reviewed periods until1796774400 (2026-12-09UTC). Official https://api-docs.deepseek.com/zh-cn/guides/search/ verified previous turn. Config backup at backups/operations-remediation-20261109.
- AI Infra grant investigation CORRECTION: reader ALREADY sees old and new versions. Private audit catalog includes both. No grant missing (0 changed). Do not claim added grants or that newer version invisible. Newer complete publication01a100f5-e3c3-72f3-8bfb-eb8a08774ab5; old preparing expected historical version.
- Local fixes: read-tools video_search returns existing source_revision; runtime locator observer grants text_reads only matching bookid+source_revision, no evidence grant. Cross-source still scoped.
- Runtime halts after BOARD_PREVIEW_UNAVAILABLE after persisting receipts/effects; no repeated sampling. Presentation errors retain existing separate behavior.
- Schema24 operational_recovery.sql adds recovery_scope + operational_successes. operations.rs records matching successes (board requires board_delivery; video successful read + video attempt/book; presentation requires answer part; web search/fetch individually; model; save owner). Group failures by scope.
- web_usage shared rate parser+health contiguous intervals. operations includes web health and pending>24h counts. watchdog scoped recovery + web expiry + pending alerts.
- Local tests passed initially12 operations, 1 video locator; watchdog9. Preview actual loop test passed1 verifies 1sampling,1toolcall and receipt retained.
- Added final scope-persistence test after prior server test run; need execute latest operations tests again (session33123 may used pre-added test, inspect count).

## Builds currently running
- Initial remote build_candidate.py hardlinked live release, broke relevant sources/binaries/cache links, applied initial changes.patch. 6 release bins -j1; remote build.log / build.json / build-failure.json.
- Since then refined runtime to avoid changing presentation behavior, scoped admission + web/presentation successes, added unit tests.
- upload_refinement.py uploaded final-changes.patch and final-changed-files.json, started refine_candidate.py background. It stages patch against OLD source then WAITS initial build.json; only replaces candidate files that changed, recompiles6bins, writes final-build.json. Log refine-build.log. Do NOT deploy before final-build.json.
- Local baseline copies tmp/.../before; changed-files.json lists10+board_author_runtime_tests. operational_recovery.sql new. New backup scripts not part cargo patch.

## Backup setup and running first backup
- Old host115.190.121.150 reachable, retired business service inactive. Existing users/books/data retained. Cleared ONLY derived cargo caches in retired releases and node_modules, freed~5.9GB; capacity evidence backup-capacity.json + backup-node-capacity.json. ~8.9GB free BEFORE new backups.
- Installed apt restic0.16.4 on production; initialized encrypted SFTP repository on oldhost /opt/understand-book/backups/production-restic. Uses compression+dedup to avoid recopying full9GB every day.
- Prod /opt/understand-book/operations/backup_ed25519, backup_known_hosts, backup-password, backup-destination.json; root-only permissions. Key added to oldroot authorized_keys, hostkey pinned. Credentials ALSO downloaded E:/allwork/backups/understand-book-recovery with inherited ACL removed, Lenovo-only. Never print secrets.
- scripts/linux/backup-production.py uploaded to prod operations/backup-production.py. Daily snapshot only if idle, checks active/bkg runs; maintenance nginx + stop, rsync root includes all users/library/control/config, restart/readiness/finallyrestore; copies release excluding compiler caches/private build/docs etc; restic uploads frozen snapshot+tools+models; keep7remote,2local, retry transfer before new snapshot.
- First backup started PID203660 via start_backup.py. Remote log backup-first.log. Local capture finished1794199581, snapshot backups/business/1794199474; maintenance18.967sec, service restored; offsite was still uploading last check. Status operations/backup-status.json contains snapshot_id/offsite_at on complete. DO NOT deploy while backup capture occurs (capture now finished, upload may continue).
- New scripts/linux/external-watchdog.py checks HTTPS /api/auth/me401 and / page, oldhost repository snapshotmtime>36h, olddisk<1GiB; uses existing watchdog transitions/mail queue. External test passed1. Syntax py_compile passed. NOT installed/enabled yet.
- New systemd files scripts/linux/understand-book-backup.{service,timer}, understand-book-external.{service,timer}; NOT installed yet. PS wrote BOM; strip utf-8-sig before Linux upload.
- External module load of operations-watchdog.py is inside main (safe local tests). Oldhost destination /opt/understand-book/operations-external.
- Existing production mail environment: multi-reader.env + operations/alerts.env. Copy ONLY mail allowlist (API_KEY,FROM,ENDPOINT,ALERT_EMAIL) to oldhost private mail.env, not entire app env. Never output values. Oldhost python3.12 exists.

## Next actions
1. Poll remote build and backup compactly. Local cargo test -p server --lib operations after final test added; no repeated unrelated tests. Test restore actual offsite backup (control quick_check + JSONL/board exact bytes) into isolated location, preferably extracted full business data. Don't restore over live.
2. Install backup timer hourly (script skips <23h), external monitor minute after first backup complete; verify actual successfulcheck+timer. No unsolicited test email needed. Test notification logic done locally.
3. Adapt tmp/operations-observability-20261108/deploy.py to schema23->24, OLDcontinuous, Rremediation, final-build.json. Idle+maintenance, backup current DB, change service/env/nginx/worker paths, health reader/admin, no old backfill call. Exact preserved catalog baseline includes web renew. Rollback before exposure restore DB/config only; never overwrite live data after exposure.
4. Upload watchdog update to /opt/understand-book/operations/operations-watchdog.py with source snapshot; new API no UI changes. Post deploy actual video same failed question test + board regression sufficient using verify_board_remote.py patterns (restore admin allowance finally). Verify operations matching success scopes, webuntil, pending0. Do not fake success table through SQL.
5. Update audit/remediation docs + operational runbook, save sanitized evidence including cost/restore/timers/deploy, user final clearly material builds deferred and historicalversions remain.

## References
- Audit docs/performance/operations-audit-20261109/README.md, private tmp/alert-diagnosis-20261109/audit*.json.
- Today's board proof docs/performance/board-preview-path-20261109, helper verify_board_remote.py/final_remote.py.
- Production HTTP Client runpy live scripts/linux/smoke-mcp-release.py; admin login password /opt/understand-book/adm10-admin-credentials.json; always logout. Workspace IDs fresh perattach/login. Probe client_request_id ops-probe-* admin. Actual video book19ac880d-a788-43f4-9725-6c56f4980189 pub01a15c89-da55-7d71-8329-a0ba9f25c86b.
- Runbook docs/Linux多人阅读-现网运行说明.md. Previous deployment python has idle background logic/health and rollback.

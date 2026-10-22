from pathlib import Path
from datetime import datetime,timezone
import json,os,runpy,shutil,subprocess,urllib.request
B=Path('/opt/understand-book');S=B/'acceptance/cloud-20261021';NEW=B/'releases/cloud-20261021'
run=lambda *args:subprocess.check_output(list(map(str,args)),text=True).strip()
assert run('systemctl','show','understand-book-multi','-p','WorkingDirectory','--value')==str(NEW)
assert run('systemctl','is-active','understand-book-multi')=='active'
pid=int(run('systemctl','show','understand-book-multi','-p','MainPID','--value'));assert os.readlink('/proc/'+str(pid)+'/exe')==str(NEW/'target/release/server')
# Initial execution passed backup, schema, before/after account, public API and
# static-byte comparisons before reaching the download request in deploy.py.
failure=(S/'deploy.log').read_text();assert 'urllib.error.HTTPError: HTTP Error 403' in failure and 'urlopen' in failure
request=urllib.request.Request('https://understandbook.top/downloads/UnderstandBookSetup.exe',headers={'User-Agent':'UnderstandBook-MCP10-Verification'})
with urllib.request.urlopen(request,timeout=60) as response:
 installer=response.read();assert installer==(NEW/'downloads/UnderstandBookSetup.exe').read_bytes();assert response.headers.get_content_type()=='application/octet-stream'
report={'status':'passed','started_at_utc':'2026-10-21T01:21:58+00:00','public_reopened_at_utc':'2026-10-21T01:22:14+00:00','finished_at_utc':datetime.now(timezone.utc).isoformat(),
 'release':str(NEW),'previous_release':str(B/'releases/om6-20261020'),'base_commit':'33c19c7fa8aec9a4f1031122bc177743b234b9fd',
 'data_root':str(B/'data/multi-reader-to-tb-20261012'),'schema_before':18,'schema_after':19,
 'backup':str(B/'backups/cloud-20261021'),'configuration_backup':str(S/'previous-configuration'),
 'service_pid':pid,'maintenance_window_seconds_from_nginx_reload_log':15,
 'account_access_before_after':'passed, exact reader/admin authorized publication lists unchanged',
 'frontend':'passed, origin HTML and public entry assets compared byte-for-byte in initial deployment',
 'installer':{'version':'0.3.0','bytes':len(installer),'public_matches_candidate':True},
 'commercial_products':0,'model_calls':0,'free_bytes':shutil.disk_usage(B).free,
 'verification_adjustment':'Default Python download user agent received CDN 403; existing release verification user agent succeeded. No additional cutover or database restore.'}
(S/'deployment.json').write_text(json.dumps(report,indent=2));print(json.dumps(report,indent=2))

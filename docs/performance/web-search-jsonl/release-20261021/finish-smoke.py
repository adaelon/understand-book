from pathlib import Path
from datetime import datetime,timezone
import json,runpy,time
base=Path('/opt/understand-book');stage=base/'acceptance/search-cache-20261021';release=base/'releases/search-cache-20261021'
result=json.loads((stage/'live-smoke.json').read_text());assert result['status']=='passed'
Client=runpy.run_path(str(release/'scripts/linux/smoke-mcp-release.py'))['Client']
admin=Client({'origin':'https://understandbook.top'})
admin.login({'username':'admin','password':json.loads((base/'adm10-admin-credentials.json').read_text())['password']})
try:
    owner=result['user_id']
    response=admin.json('/api/admin/users/'+owner+'/status',{'operation_id':'search-release-disable-20261021','disabled':True})
    for attempt in range(30):
        records=admin.json('/api/admin/charges?user_id='+owner+'&limit=100')['items']
        if not any(r['state'] in ['reserved','sent'] for r in records):break
        time.sleep(1)
    assert all(r['state'] in ['settled','released'] and r.get('provider_cost_micro_cny') is not None for r in records)
    total=sum(r.get('provider_cost_micro_cny') or 0 for r in records)
    assert total<=result['allowance_micro_cny']
    final={'status':'passed','user_id':owner,'disabled':True,'records_preserved':len(records),'total_provider_cost_micro_cny':total,
      'same_cost_as_smoke':total==result['total_provider_cost_micro_cny'],'charges':records,
      'cost_summary':admin.json('/api/admin/cost-summary?user_id='+owner),'finished_at_utc':datetime.now(timezone.utc).isoformat()}
    (stage/'final-accounting.json').write_text(json.dumps(final,ensure_ascii=False,indent=2))
    print(json.dumps({k:v for k,v in final.items() if k not in ['charges','cost_summary']},indent=2))
finally:admin.json('/api/auth/logout',{})

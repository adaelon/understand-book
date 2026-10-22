from pathlib import Path
from concurrent.futures import ThreadPoolExecutor
from datetime import datetime,timezone
import json,re,urllib.request
stage=Path('tmp/reader-latency-release-20261021')
web=stage/'source/packages/web/dist-latency';origin='https://understandbook.top'
installer=(stage/'windows-source/dist/UnderstandBookSetup.exe').read_bytes()
index=(web/'index.html').read_bytes()
assets=json.loads((stage/'static.json').read_text('utf-8'))['files']
def check(relative):
    with urllib.request.urlopen(urllib.request.Request(origin+'/'+relative,headers={'User-Agent':'UnderstandBook-Release-Verification'}),timeout=60) as response:
        actual=response.read();status=response.status
    expected=installer if relative.startswith('downloads/') else (web/relative).read_bytes()
    if relative=='index.html':
        # The existing CDN injects its analytics beacon into HTML responses.
        normalized=re.sub(r'<script\b[^>]*src="https://static.cloudflareinsights.com/[^"]+"[^>]*></script>\s*','',actual.decode('utf-8')).encode('utf-8')
        assert normalized==expected,'Reader HTML differs beyond the existing CDN analytics beacon'
        return {'path':relative,'status':status,'bytes':len(actual),'candidate_html_matches_without_cdn_analytics':True}
    assert actual==expected,relative+' differs from the candidate'
    return {'path':relative,'status':status,'bytes':len(actual),'exact_match':True}
with ThreadPoolExecutor(max_workers=3) as pool:
    results=list(pool.map(check,assets+['downloads/UnderstandBookSetup.exe?v=0.3.3','downloads/UnderstandBookSetup.exe']))
report={'status':'passed','finished_at_utc':datetime.now(timezone.utc).isoformat(),'checks':results}
(stage/'public-verification.json').write_text(json.dumps(report,indent=2),'utf-8')
print(json.dumps(report,indent=2))

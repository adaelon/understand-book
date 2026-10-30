from pathlib import Path
from datetime import datetime,timezone
import json,re,urllib.request,urllib.error,runpy,subprocess
base=Path('/opt/understand-book');stage=base/'acceptance/board-source-window-20261028';release=base/'releases/board-source-window-20261028'
origin='https://understandbook.top';dist=release/'packages/web/dist-multi'
def get(path):
 with urllib.request.urlopen(urllib.request.Request(origin+path,headers={'User-Agent':'Mozilla/5.0'}),timeout=30) as response:return response.read()
assets=set()
for page in ['index.html','home.html','admin/index.html']:
 html=get('/'+page)
 clean=re.sub(rb'<script\b[^>]*src="https://static\.cloudflareinsights\.com/[^" ]+"[^>]*></script>\n?',b'',html)
 assert clean==(dist/page).read_bytes(),page
 assets.update(re.findall(r'(?:src|href)="(/(?:admin/)?assets/[^" ]+\.(?:js|css))"',html.decode()))
for path in sorted(assets):assert get(path)==(dist/path.lstrip('/')).read_bytes(),path
for path in ['board-preview/renderer.js','board-preview/renderer.css']:assert get('/'+path)==(dist/path).read_bytes(),path
reader=(dist/next(p.lstrip('/') for p in assets if '/reader-' in p and p.endswith('.js'))).read_text()
assert 'data-board-math' in reader and 'data-board-rich-text' in reader
assert 'board-note-link-label-bg' in reader
assert 'board-source-reader' in reader and 'board-canvas-fullscreen' in reader and 'source.read' in reader
health=runpy.run_path(str(stage/'health.py'))['ready']()
service=subprocess.check_output(['systemctl','show','understand-book-multi','-p','ActiveState','-p','NRestarts','-p','MainPID','-p','WorkingDirectory'],text=True).strip()
report={'status':'passed','verified_at_utc':datetime.now(timezone.utc).isoformat(),'public_pages_match_release':True,'referenced_assets_equal':len(assets),'board_preview_assets_equal':True,'formatted_board_text_present':True,'workspace_note_links_preserved':True,'reader_and_admin_health':health,'service':service}
(stage/'public-verification.json').write_text(json.dumps(report,indent=2));print(json.dumps(report))

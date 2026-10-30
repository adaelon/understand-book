from pathlib import Path
import json, runpy, time, urllib.error, urllib.parse
B=Path('/opt/understand-book');N=B/'releases/main-agent-planned-20261029'
smoke=runpy.run_path(str(N/'scripts/linux/smoke-mcp-release.py'))
def ready(transport=None):
    credentials=json.loads((B/'multi-reader-credentials.json').read_text())
    credentials['admin']=json.loads((B/'adm10-admin-credentials.json').read_text())['password']
    config={'origin':'https://understandbook.top'}
    if transport:config['transport_url']=transport
    report={}
    for username in ['reader','admin']:
        client=smoke['Client'](config)
        for attempt in range(35):
            try:client.request('/api/auth/me',expected=401);break
            except urllib.error.URLError:
                if attempt==34:raise
                time.sleep(1)
        client.login({'username':username,'password':credentials[username]})
        try:
            smoke['require_presentation_ready'](client.json('/api/auth/me')['capabilities']['presentation'])
            books=client.json('/api/library')['books'];assert books
            selected=next(b for b in books if b.get('is_default'))['published_book_ref']
            route='/api/books/'+selected['book_id']+'/publications/'+selected['publication_id']
            manifest=client.json(route+'/manifest');leaf=next(n for n in manifest['tree'] if not n.get('children'))
            assert client.json(route+'/text?'+urllib.parse.urlencode({'lid':leaf['lid']}))['text']
            client.json('/api/learning-workspaces')
            client.json('/api/me/chats')
            client.json('/api/account/allowance')
            plans=client.json('/api/account/plans');assert plans['catalog']['items']==[] and plans['current'] is None
            storage=client.json('/api/account/storage');assert storage['can_upload'] is False
            if username=='admin':assert client.json('/api/admin/cloud-plans')['items']==[]
            report[username]={'authorized_books':len(books),'presentation_ready':True,'manifest_and_body':True,'history_and_allowance_readable':True,
                'references':sorted((b['published_book_ref']['book_id'],b['published_book_ref']['publication_id']) for b in books)}
        finally:client.json('/api/auth/logout',{})
    return report


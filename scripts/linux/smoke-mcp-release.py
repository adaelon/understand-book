"""Exercise a deployed MCP endpoint with 15 explicitly gifted verification calls.

Private JSON config: origin, reader/admin {username,password}, operation_id,
book_id and query. Optional transport_url selects an isolated loopback server;
origin remains the trusted public origin. Credentials and source text are never
written to the report. Use a fresh operation_id for each complete run.
"""
import argparse
from concurrent.futures import ThreadPoolExecutor
from datetime import datetime, timezone
import http.cookies
import json
from pathlib import Path
import threading
import time
import urllib.error
import urllib.parse
import urllib.request

PROTOCOL = '2025-11-25'


class Client:
    def __init__(self, config):
        self.origin = config['origin'].rstrip('/')
        self.transport = config.get('transport_url', self.origin).rstrip('/')
        self.cookie = ''
        self.csrf = ''

    def request(self, path, data=None, expected=200, headers=None, method=None, web=True):
        request_headers = {'User-Agent': 'UnderstandBook-MCP10-Verification'}
        if web:
            request_headers.update({'Origin': self.origin, 'Cookie': self.cookie, 'X-CSRF-Token': self.csrf})
        if self.transport != self.origin:
            request_headers['Host'] = urllib.parse.urlsplit(self.origin).netloc
        if data is not None:
            request_headers['Content-Type'] = 'application/json'
        request_headers.update(headers or {})
        request = urllib.request.Request(self.transport + path, method=method,
            data=None if data is None else json.dumps(data).encode(), headers=request_headers)
        try:
            response = urllib.request.urlopen(request, timeout=60)
        except urllib.error.HTTPError as error:
            response = error
        with response:
            body = response.read()
            if response.status != expected:
                raise AssertionError(f'{path.split("?")[0]}: expected HTTP {expected}, got {response.status}')
            for value in response.headers.get_all('Set-Cookie', []):
                cookies = http.cookies.SimpleCookie(value)
                self.cookie = '; '.join(f'{key}={morsel.value}' for key, morsel in cookies.items())
            return body

    def json(self, path, data=None, **kwargs):
        return json.loads(self.request(path, data, **kwargs))

    def login(self, account):
        self.json('/api/auth/login', account)
        identity = self.json('/api/auth/me')
        self.csrf = identity['csrf_token']
        assert identity['user_id'] == account['username']


def verify(config):
    reader, admin, remote = [Client(config) for _ in range(3)]
    checks, measures = [], []
    credential = None
    report = {'status': 'running', 'origin': reader.origin, 'transport_url': reader.transport, 'checks': checks,
              'concurrency': measures, 'operation_id': config['operation_id'],
              'verified_at_utc': datetime.now(timezone.utc).isoformat()}
    reader.request('/api/auth/me', expected=401)
    reader.login(config['reader'])
    admin.login(config['admin'])
    token = ''

    def rpc(method, params=None, *, expected=200, extra=None, notification=False):
        payload = {'jsonrpc': '2.0', 'method': method}
        if not notification:
            payload['id'] = method
        if params is not None:
            payload['params'] = params
        headers = {'Authorization': 'Bearer ' + token, 'MCP-Protocol-Version': PROTOCOL,
                   'Accept': 'application/json, text/event-stream', **(extra or {})}
        started = time.perf_counter()
        body = remote.request('/mcp', payload, expected=expected, headers=headers, web=False)
        return (json.loads(body) if body else None), (time.perf_counter()-started)*1000, len(body)

    def tool(name, arguments):
        response, elapsed, size = rpc('tools/call', {'name': name, 'arguments': arguments})
        assert 'result' in response, 'JSON-RPC failed: ' + name
        return response['result'], elapsed, size

    def content(name, arguments):
        result, elapsed, size = tool(name, arguments)
        assert not result.get('isError'), 'Content tool failed: ' + result.get('structuredContent', {}).get('error_code', name)
        value = result['structuredContent']
        assert value['usage']['charged_units'] == 1
        assert value['usage']['charged_from']['kind'] == 'free_month'
        return value, elapsed, size

    try:
        library = reader.json('/api/library')['books']
        allowance = reader.json('/api/account/allowance')
        admin.json('/api/admin/usage')
        admin.json('/api/admin/users/' + config['reader']['username'] + '/receipts')
        checks.append('Existing reader/admin login, authorized catalog, model allowance and receipts are readable')
        state = reader.json('/api/account/mcp')
        assert state['mcp_url'] == reader.origin + '/mcp'
        before = state['usage']
        gift_path = '/api/admin/users/' + config['reader']['username'] + '/mcp/gifts'
        gift = {'operation_id': config['operation_id'], 'units': 15,
                'reason': 'MCP10 release verification: 15 successful reads; no payment or model request'}
        # A replay is not another test allowance: stop before making calls.
        admin.request('/api/admin/operations/' + config['operation_id'], expected=404)
        granted = admin.json(gift_path, gift)
        assert admin.json(gift_path, gift) == granted
        assert reader.json('/api/account/mcp')['usage']['free_remaining'] == before['free_remaining'] + 15
        credential = reader.json('/api/account/mcp/credentials', {'label': config['operation_id']})
        token = credential['token']
        initialized, _, _ = rpc('initialize', {'protocolVersion': PROTOCOL, 'capabilities': {},
                                      'clientInfo': {'name': 'mcp10-release-verifier', 'version': '1'}})
        assert initialized['result']['protocolVersion'] == PROTOCOL
        assert initialized['result']['instructions']
        assert rpc('notifications/initialized', notification=True, expected=202)[0] is None
        definitions, _, _ = rpc('tools/list', {})
        assert len(definitions['result']['tools']) == 11
        rpc('tools/list', {}, extra={'Origin': 'https://wrong-origin.example'}, expected=403)
        rpc('tools/list', {}, extra={'MCP-Protocol-Version': 'unsupported'}, expected=400)
        rpc('tools/list', {}, extra={'Authorization': ''}, expected=401)
        remote.request('/mcp', expected=405, headers={'Authorization': 'Bearer '+token}, web=False)
        checks.append('Protocol over ' + remote.transport + ': no-Origin Bearer, 11 tools, notification 202, GET 405, bad Origin/version and missing credential rejected')
        directory = tool('book_list', {'limit': 50})[0]['structuredContent']
        assert directory['usage']['charged_units'] == 0
        selected = next(book for book in directory['items'] if book['published_book_ref']['book_id'] == config['book_id'] and book['is_default'])
        reference = selected['published_book_ref']
        assert reference in [book['published_book_ref'] for book in library]
        book_path = '/api/books/' + urllib.parse.quote(reference['book_id']) + '/publications/' + urllib.parse.quote(reference['publication_id'])
        located = content('book_search_text', {'published_book_ref': reference, 'query': config['query']})[0]
        hit = located['data']['occurrences'][0]
        arguments = {'published_book_ref': reference, 'lid': hit['start_lid'], 'end_lid': hit['end_lid']}
        page = content('book_text', arguments)[0]
        source = page['sources'][0]
        assert source['published_book_ref'] == reference
        url = urllib.parse.urlsplit(source['source_url'])
        assert url.scheme + '://' + url.netloc == reader.origin
        query = urllib.parse.parse_qs(url.query)
        for name, expected in reference.items():
            assert query[name] == [expected]
        span = page['data']['source_range']
        assert int(query['start_utf16'][0]) == span['start']
        assert int(query['end_utf16'][0]) == span['end']
        manifest = reader.json(book_path + '/manifest')
        leaf = next(item for item in manifest['tree'] if item['lid'] == arguments['lid'])
        original = reader.json(book_path + '/text?' + urllib.parse.urlencode({'lid': arguments['lid'], 'end_lid': arguments['end_lid']}))
        raw = original['text'].encode('utf-16-le')
        extracted = raw[(span['start']-leaf['span']['start'])*2:(span['end']-leaf['span']['start'])*2].decode('utf-16-le')
        assert extracted == page['data']['text'], 'MCP source differs from Reader canonical text'
        reader.json(book_path + '/tutor_readiness')
        checks.append('Search/read/source URL identifies the exact publication and matches Reader text at UTF-16 coordinates')
        invalid = tool('book_text', {'published_book_ref': reference, 'lid': 'mcp10-nonexistent-lid'})[0]
        assert invalid['isError'] and invalid['structuredContent']['charged_units'] == 0
        for count in (1, 4, 8):
            barrier = threading.Barrier(count)
            def one(_):
                barrier.wait()
                result, elapsed, size = content('book_text', arguments)
                assert result['data']['text'] == page['data']['text']
                return elapsed, size
            with ThreadPoolExecutor(max_workers=count) as pool:
                values = list(pool.map(one, range(count)))
            measures.append({'concurrent_reads': count, 'successful': len(values),
                             'latency_ms': [round(v[0], 3) for v in values],
                             'response_bytes': [v[1] for v in values]})
        after = reader.json('/api/account/mcp')['usage']
        assert after['free_used'] - before['free_used'] == 15
        assert after['free_remaining'] == before['free_remaining']
        assert after['paid_remaining'] == before['paid_remaining']
        assert reader.json('/api/account/allowance') == allowance
        checks.append('Exactly 15 gifted free calls consumed; original free balance, paid balance and model allowance preserved')
        report.update(status='passed', publication_count=len(library), published_book_ref=reference,
                      source_range_utf16=span, canonical_text_matches=True,
                      gifted_calls=15, charged_calls=15, paid_calls=0, model_requests=0,
                      free_remaining_before=before['free_remaining'], free_remaining_after=after['free_remaining'])
    finally:
        if credential:
            reader.json('/api/account/mcp/credentials/' + credential['credential']['credential_id'] + '/revoke', {})
            rpc('tools/list', {}, expected=401)
            report['test_credential_revoked'] = True
        for client in (reader, admin):
            if client.csrf:
                client.json('/api/auth/logout', {})
        report['test_sessions_logged_out'] = True
    return report


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('config', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    result = verify(json.loads(args.config.read_text(encoding='utf-8')))
    args.output.write_text(json.dumps(result, indent=2), encoding='utf-8')
    print(json.dumps(result, indent=2))

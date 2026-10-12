"""Measure 1/4/8 simultaneous full MCP pages using 13 explicitly gifted calls.

Uses the same private JSON format as smoke-mcp-release.py. Select a publication
with at least 8000 characters; supply a fresh operation_id. No payment or model
request is made. Report contains timings and counts, never credentials or text.
"""
from pathlib import Path
from concurrent.futures import ThreadPoolExecutor
import argparse
import importlib.util
import json
import threading
import time
import urllib.parse

spec = importlib.util.spec_from_file_location('smoke', Path(__file__).with_name('smoke-mcp-release.py'))
smoke = importlib.util.module_from_spec(spec)
spec.loader.exec_module(smoke)


def measure(config):
    reader, admin, remote = [smoke.Client(config) for _ in range(3)]
    reader.login(config['reader'])
    admin.login(config['admin'])
    credential = None
    try:
        baseline = reader.json('/api/account/mcp')['usage']
        book = next(book for book in reader.json('/api/library')['books']
                    if book['published_book_ref']['book_id'] == config['book_id'] and book['is_default'])
        reference = book['published_book_ref']
        path = '/api/books/' + urllib.parse.quote(reference['book_id']) + '/publications/' + urllib.parse.quote(reference['publication_id'])
        tree = reader.json(path + '/manifest')['tree']
        first = min(tree, key=lambda node: node['span']['start'])
        last = max(tree, key=lambda node: node['span']['end'])
        arguments = {'published_book_ref': reference, 'lid': first['lid'], 'end_lid': last['lid']}
        admin.request('/api/admin/operations/' + config['operation_id'], expected=404)
        admin.json('/api/admin/users/' + config['reader']['username'] + '/mcp/gifts',
                   {'operation_id': config['operation_id'], 'units': 13,
                    'reason': 'MCP10 HTTPS capacity verification: 13 full 8000-character pages; no payment or model request'})
        credential = reader.json('/api/account/mcp/credentials', {'label': config['operation_id']})
        headers = {'Authorization': 'Bearer ' + credential['token'], 'MCP-Protocol-Version': smoke.PROTOCOL,
                   'Accept': 'application/json, text/event-stream'}
        remote.json('/mcp', {'jsonrpc': '2.0', 'id': 1, 'method': 'initialize', 'params':
            {'protocolVersion': smoke.PROTOCOL, 'capabilities': {}, 'clientInfo': {'name': 'mcp10-page-capacity', 'version': '1'}}},
            headers=headers, web=False)
        measurements, first_page = [], None
        for count in (1, 4, 8):
            barrier = threading.Barrier(count)

            def read(_):
                barrier.wait()
                started = time.perf_counter()
                raw = remote.request('/mcp', {'jsonrpc': '2.0', 'id': 1, 'method': 'tools/call',
                    'params': {'name': 'book_text', 'arguments': arguments}}, headers=headers, web=False)
                elapsed = (time.perf_counter() - started) * 1000
                result = json.loads(raw)['result']
                assert not result.get('isError')
                value = result['structuredContent']
                assert len(value['data']['text']) == 8000
                assert not value['complete'] and value['next_calls']
                assert value['usage']['charged_units'] == 1 and value['usage']['charged_from']['kind'] == 'free_month'
                return {'latency_ms': round(elapsed, 3), 'response_bytes': len(raw)}, value

            with ThreadPoolExecutor(max_workers=count) as pool:
                responses = list(pool.map(read, range(count)))
            if first_page is None:
                first_page = responses[0][1]
            assert all(value['data']['text'] == first_page['data']['text'] for _, value in responses)
            measurements.append({'concurrent_reads': count, 'successful': len(responses),
                                 'reads': [value for value, _ in responses]})
        after = reader.json('/api/account/mcp')['usage']
        assert after['free_used'] - baseline['free_used'] == 13
        assert after['free_remaining'] == baseline['free_remaining']
        assert after['paid_remaining'] == baseline['paid_remaining']
        report = {'status': 'passed', 'origin': config['origin'], 'transport_url': remote.transport, 'published_book_ref': reference,
                  'characters_per_page': 8000, 'source_range_utf16': first_page['data']['source_range'],
                  'concurrency': measurements, 'gifted_calls': 13, 'charged_calls': 13, 'paid_calls': 0, 'model_requests': 0,
                  'free_remaining_before': baseline['free_remaining'], 'free_remaining_after': after['free_remaining']}
    finally:
        if credential:
            reader.json('/api/account/mcp/credentials/' + credential['credential']['credential_id'] + '/revoke', {})
        reader.json('/api/auth/logout', {})
        admin.json('/api/auth/logout', {})
    report.update(test_credential_revoked=True, test_sessions_logged_out=True)
    return report


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('config', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    result = measure(json.loads(args.config.read_text(encoding='utf-8')))
    args.output.write_text(json.dumps(result, indent=2), encoding='utf-8')
    print(json.dumps(result, indent=2))

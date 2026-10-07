"""Report observed run/accounting facts; never infer quality from delivery."""
import collections
import json
import sys
from pathlib import Path

root = Path(sys.argv[1])
def read(path):
    return json.loads(path.read_text(encoding='utf-8-sig'))

frozen = read(root / 'frozen.json')
rows = []
run_ids = list(frozen['run_order'])
for name in frozen['run_order']:
    run_ids.extend(f'{name}/{p.name}' for p in sorted((root / 'runs' / name).glob('revision-*')) if p.is_dir())
run_ids.extend(f'diagnostics/{p.name}' for p in sorted((root / 'diagnostics').glob('*')) if p.is_dir())
for name in run_ids:
    run = root / name if name.startswith('diagnostics/') else root / 'runs' / name
    if not run.exists():
        rows.append({'id': name, 'status': 'not_started'})
        continue
    responses = [read(p) for p in sorted(run.glob('response-*.json'))]
    calls = [call for response in responses for call in response.get('tool_calls', [])]
    author_calls = []
    invalid_author_arguments = []
    for response_index, response in enumerate(responses):
        for call in response.get('tool_calls', []):
            if call['name'] != 'presentation.author':
                continue
            try:
                author_calls.append(json.loads(call['arguments']))
            except json.JSONDecodeError as error:
                invalid_author_arguments.append({'response_index': response_index, 'error': str(error)})
    summary = read(run / 'summary.json') if (run / 'summary.json').exists() else {}
    outcome = read(run / 'outcome.json') if (run / 'outcome.json').exists() else {}
    completions = [read(p) for p in sorted(run.glob('completion-*-response.json'))]
    completion_tokens = sum((c.get('usage') or {}).get('total_tokens') or 0 for c in completions)
    identity = 'diagnostic_continuation' if name.startswith('diagnostics/') else 'agent_revision' if '/' in name else 'plain_control' if name.startswith('plain-') else 'independent_generation'
    row = {'id':name, 'identity':identity, 'status':'returned' if summary else 'running',
           'method':summary.get('method'), 'model':summary.get('model'),
           'requests':len(list(run.glob('request-*.json'))), 'responses':len(responses),
           'chat_reported_tokens':sum(r.get('usage') or 0 for r in responses),
           'completion_reported_tokens':completion_tokens,
           'reported_tokens':sum(r.get('usage') or 0 for r in responses) + completion_tokens,
           'structured_completion_count':summary.get('structured_completions'),
           'completion_usage_available':summary.get('structured_completions') == len(completions) and all((c.get('usage') or {}).get('total_tokens') is not None for c in completions),
           'all_chat_usage_available':all(r.get('usage') is not None for r in responses) and len(responses)==len(list(run.glob('request-*.json'))),
           'elapsed_ms':summary.get('elapsed_ms'), 'incomplete':outcome.get('incomplete'),
           'warning':outcome.get('warning'), 'error':outcome.get('error_code'),
           'author_operations':dict(collections.Counter(call.get('operation', 'missing_operation') for call in author_calls)),
           'invalid_author_arguments':invalid_author_arguments,
           'delivered':(run / 'content.json').exists()}
    if (run / 'history.json').exists():
        history = read(run / 'history.json')
        row['goals'] = [{'status':g['status'],'verification':[r['verification'] for r in g['requirements']]} for s in history['sessions'] for g in s.get('goals', [])]
        activities = [a for s in history['sessions'] for t in s['turns'] for a in (t.get('run_summary') or {}).get('activities', [])]
        row['tool_durations_ms'] = [{'name':a['name'],'status':a['status'],'duration_ms':a['duration_ms']} for a in activities if a['kind']=='tool']
    rows.append(row)
result = {'batch':root.name,'method':frozen['method'],'runs':rows,'total_reported_tokens':sum(r.get('reported_tokens',0) for r in rows)}
(root / 'cost-summary.json').write_text(json.dumps(result,ensure_ascii=False,indent=2),encoding='utf-8')
print(json.dumps({'batch':root.name,'tokens':result['total_reported_tokens'],'runs':[{k:v for k,v in r.items() if k not in ['tool_durations_ms','goals']} for r in rows]},ensure_ascii=False))

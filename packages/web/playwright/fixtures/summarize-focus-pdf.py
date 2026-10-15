"""Analyze saved FP6 browser traces. Compare decoded PDF reader pixels directly.

Headless browser frame evidence is not physical panel presentation evidence.
Run from repo root; no app or browser process is needed.
"""
import base64
import gzip
import io
import json
import math
import re
import sys
from pathlib import Path
from PIL import Image, ImageChops

ROOT = Path(sys.argv[sys.argv.index('--root') + 1]) if '--root' in sys.argv else Path('docs/performance/focus-reading-fp6-20261015/results')
WORK_KEYS = ['preparations', 'canvases', 'textLayers', 'textChunks', 'decorations']

def dist(values):
    values = sorted(values)
    return dict(count=len(values), **{key: values[max(0, math.ceil(len(values) * q) - 1)] if values else None
                for key, q in [('p50', .5), ('p95', .95), ('p99', .99), ('max', 1)]})

def analyze_trace(file):
    events = json.loads(gzip.decompress(file.read_bytes()))['traceEvents']
    result = json.loads(file.with_name(file.name.replace('.trace.json.gz', '.json')).read_text(encoding='utf-8'))
    marks = sorted([e for e in events if re.match(r'^(fp4-\d+|fp1-input-\d+)-input$', e['name'])], key=lambda e: e['ts'])
    ends = {e['name']: e['ts'] for e in events if e['name'].endswith('-end')}
    shots = sorted([e for e in events if e['name'] == 'Screenshot' and e.get('args', {}).get('snapshot')], key=lambda e: e['ts'])
    # The pre-trace image is only a pixel reference. Its sentinel timestamp
    # never contributes to latency; changed captures use actual trace times.
    if result.get('trace', {}).get('initialScreenshot'):
        shots.insert(0, dict(ts=0, args=dict(snapshot=result['trace']['initialScreenshot'])))
    decoded = {}
    def crop(shot):
        ts = shot['ts']
        if ts not in decoded:
            im = Image.open(io.BytesIO(base64.b64decode(shot['args']['snapshot']))).convert('RGB')
            w, h = im.size
            # Preserve the entering page edge (especially a reverse drag).
            # Side controls occupy the outer padding; menus sit above/below.
            decoded[ts] = im.crop((math.ceil(w * .045), math.ceil(h * .06), math.floor(w * .955), math.floor(h * .94)))
        return decoded[ts]
    windows, captures = [], []
    for mark in marks:
        end = ends.get(mark['name'].removesuffix('-input') + '-end')
        if end is None: raise ValueError(f'Missing animation end: {mark["name"]}')
        previous = [s for s in shots if s['ts'] < mark['ts']]
        changed = None
        if previous:
            before = crop(previous[-1])
            for shot in shots:
                if not mark['ts'] <= shot['ts'] <= end: continue
                current = crop(shot)
                reference = before if before.size == current.size else before.resize(current.size, Image.Resampling.LANCZOS)
                difference = ImageChops.difference(reference, current).convert('L')
                pixels = difference.histogram()
                # Ignore JPEG noise; require >0.5% of pixels to change by >8/255.
                if sum(pixels[9:]) > current.width * current.height * .005:
                    changed = shot['ts']; captures.append((changed - mark['ts']) / 1000); break
        drawings = [e for e in events if e['name'] == 'DrawFrame' and mark['ts'] <= e['ts'] <= end]
        drops = [e for e in events if e['name'] == 'DroppedFrame' and mark['ts'] <= e['ts'] <= end]
        ids = sorted(set(e['args']['frameSeqId'] for e in drawings))
        missing_gaps = sum(max(0, b - a - 1) for a, b in zip(ids, ids[1:]))
        dropped_ids = set(e['args']['frameSeqId'] for e in drops)
        # Chromium's DroppedFrame reports a missed smoothness update. Sequence
        # gaps alone also include NO_UPDATE_DESIRED while the pointer is still;
        # retain those gaps as diagnostics rather than inventing missing frames.
        missing = len(dropped_ids)
        layouts = [e for e in events if e['name'] == 'Layout' and e.get('dur') and mark['ts'] <= e['ts'] <= end]
        pointer_down = [e for e in events if e['name'] == 'EventDispatch' and e.get('args', {}).get('data', {}).get('type') == 'pointerdown' and e['ts'] < mark['ts']]
        recognition = (mark['ts'] - pointer_down[-1]['ts']) / 1000 if 'touch' in file.name and pointer_down else None
        windows.append(dict(name=mark['name'], inputTs=mark['ts'], endTs=end, firstChangedCaptureTs=changed,
                            pointerDispatchToDragInputMs=recognition,
                            drawnFrames=len(ids), droppedFrames=len(dropped_ids), missingSequenceFrames=missing_gaps,
                            missingFrames=missing, expectedFrames=len(set(ids).union(dropped_ids)),
                            layoutCount=len(layouts), layoutMs=sum(e['dur'] for e in layouts) / 1000))
    beginnings = sorted(set(e['ts'] for e in events if e['name'] == 'BeginFrame'))
    periods = [(b - a) / 1000 for a, b in zip(beginnings, beginnings[1:]) if 4000 < b - a < 25000]
    drawn = sum(w['drawnFrames'] for w in windows); missing = sum(w['missingFrames'] for w in windows)
    expected = sum(w['expectedFrames'] for w in windows)
    samples = result['samples']
    start = samples[0]['input']; finish = samples[-1]['end']
    work = {k: max(s['after'][k] - s['before'][k] for s in samples if s.get('after'))
            for k in WORK_KEYS}
    return dict(file=file.name, windows=windows, inputToChangedReaderCaptureMs=dist(captures),
                peak=result['final']['peak'], maxPages=result['final']['current']['geometry']['columns'] * 3, misses=result['final']['metrics']['misses'],
                inputToObservedTransformMs=dist([s['firstChange'] - s['input'] for s in samples if s['firstChange'] is not None]),
                browserFrames=dict(drawn=drawn, expected=expected, missing=missing, missingRatio=missing / expected if expected else None,
                                   observedRefreshPeriodMs=dist(periods)),
                longTasksDuringReading=[t for t in result['longTasks'] if start <= t['start'] <= finish],
                longTasksBeforeReading=[t for t in result['longTasks'] if t['start'] < start],
                maxWorkDuringInteraction=work,
                mathFontRequestsDuringReading=[e['args']['data']['url'] for e in events if e['name'] == 'ResourceSendRequest'
                    and 'KaTeX_' in e.get('args', {}).get('data', {}).get('url', '') and marks[0]['ts'] <= e['ts'] <= windows[-1]['endTs']],
                physicalDisplayLatency=None)

def analyze_normal(file):
    r = json.loads(file.read_text(encoding='utf-8')); samples = r['samples']
    peak = {k: max(s['after']['peak'][k] for s in r['states']) for k in r['states'][0]['after']['peak']}
    work = {k: max(s['after'][k] - s['before'][k] for s in samples if s.get('after'))
            for k in WORK_KEYS}
    start = samples[0]['input']; finish = samples[-1]['end']
    return dict(file=file.name, browser=r['browser'], width=r['width'], count=len(samples), elapsedMs=r['elapsedMs'],
                effectiveTurns=sum(s['before']['current']['id'] != s['after']['current']['id'] for s in r['states']),
                misses=max(s['after']['metrics']['misses'] for s in r['states']), peak=peak,
                inputToObservedTransformMs=dist([s['firstChange'] - s['input'] for s in samples if s['firstChange'] is not None]),
                totalAnimationMs=dist([s['end'] - s['input'] for s in samples if s.get('end')]), maxWorkDuringAnimation=work,
                longTasksDuringReading=[t for t in r['longTasks'] if start <= t['start'] <= finish],
                longTasksBeforeReading=[t for t in r['longTasks'] if t['start'] < start])


traces = [analyze_trace(p) for p in sorted(ROOT.glob('render-*.trace.json.gz'))]
normal = [analyze_normal(p) for p in sorted(ROOT.glob('normal-*.json')) if 'progress' not in p.name]
summary = dict(traces=traces, normal=normal, physicalPresentation='unmeasured; browser-rendered evidence only')
failures = []
for t in traces:
    p1 = t['inputToChangedReaderCaptureMs']
    if p1['count'] != 20 or p1['p95'] is None or p1['p95'] > 50: failures.append(t['file'] + ': P1')
    if t['browserFrames']['missingRatio'] is None or t['browserFrames']['missingRatio'] > .01: failures.append(t['file'] + ': P2')
    if t['longTasksDuringReading']: failures.append(t['file'] + ': reading long task')
    if any(t['maxWorkDuringInteraction'].values()): failures.append(t['file'] + ': P6')
    if t['misses']: failures.append(t['file'] + ': unready normal turn')
    if t['peak']['pages'] > t['maxPages'] or t['peak']['backingPixels'] > 12_000_000 or t['peak']['preparationTasks'] > 1: failures.append(t['file'] + ': resource budget')
for n in normal:
    if n['count'] != 200 or n['effectiveTurns'] != 200 or n['misses']: failures.append(n['file'] + ': P3')
    if any(n['maxWorkDuringAnimation'].values()): failures.append(n['file'] + ': P6')
    if n['longTasksDuringReading']: failures.append(n['file'] + ': reading long task')
    if n['peak']['pages'] > (3 if n['width'] == 390 else 6) or n['peak']['backingPixels'] > 12_000_000 or n['peak']['preparationTasks'] > 1: failures.append(n['file'] + ': resource budget')
display = []
for file in sorted(ROOT.glob('display-*.json')):
    r = json.loads(file.read_text(encoding='utf-8')); groups, ready_groups = {}, {}
    events = json.loads(gzip.decompress(file.with_suffix('.trace.json.gz').read_bytes()))['traceEvents']
    marks = {e['name']: e['ts'] for e in events if e['name'].startswith('fp6-display-')}
    shots = sorted(e['ts'] for e in events if e['name'] == 'Screenshot' and e.get('args', {}).get('snapshot'))
    presented = []
    for sample in r['samples']:
        kind = sample['name'].split('-')[2]
        if sample['end'] is None: failures.append(file.name + ': unfinished display change'); continue
        ready_groups.setdefault(kind, []).append(sample['end'] - sample['input'])
        begin, ready = marks[sample['name'] + '-input'], marks[sample['name'] + '-ready']
        # A ready DOM is not yet a presented page. Use the first browser
        # filmstrip image after the ready frame; keep DOM timing diagnostic.
        next_input = min((ts for name, ts in marks.items() if name.endswith('-input') and ts > begin), default=math.inf)
        capture = next((ts for ts in shots if ready <= ts < next_input), None)
        if capture is None: failures.append(file.name + ': missing readable capture'); continue
        elapsed = (capture - begin) / 1000
        groups.setdefault(kind, []).append(elapsed)
        presented.append(dict(name=sample['name'], inputTs=begin, readyTs=ready, firstReadableCaptureTs=capture, elapsedMs=elapsed))
        if not sample['oldStillVisible']: failures.append(file.name + ': P4 blank')
        if sample['before']['pageIndex'] != sample['after']['pageIndex']: failures.append(file.name + ': P4 page anchor')
    timing = {k: dist(v) for k, v in groups.items()}
    if len(r['samples']) != 60 or any(v['count'] != 20 or v['p95'] > 100 for v in timing.values()): failures.append(file.name + ': P4')
    display.append(dict(file=file.name, width=r['width'], inputToReadableScreenMs=timing,
                        inputToReadyDomMs={k: dist(v) for k, v in ready_groups.items()}, captures=presented))
summary['displayChanges'] = display
long_file = ROOT / 'long-reading.json'
if long_file.exists():
    r = json.loads(long_file.read_text(encoding='utf-8')); samples = r['interactions']
    peak = r['final']['peak']
    work = {k: max(s['after'][k] - s['before'][k] for s in samples if s.get('after')) for k in WORK_KEYS}
    start, finish = samples[0]['input'], samples[-1]['end']
    long = dict(elapsedMs=r['elapsedMs'], turns=r['turns'],
        effectiveTurns=sum(s['before']['current']['id'] != s['after']['current']['id'] for s in r['samples']),
        misses=max(s['after']['metrics']['misses'] for s in r['samples']), peak=peak,
        maxWorkDuringInteraction=work, initialResources=r['initial']['resources'], returnedResources=r['final']['resources'],
        longTasksDuringReading=[t for t in r['longTasks'] if start <= t['start'] <= finish])
    summary['longReading'] = long
    if r['elapsedMs'] < 1_200_000 or long['effectiveTurns'] < 500: failures.append('P5 duration/turns')
    if peak['pages'] > 6 or peak['backingPixels'] > 12_000_000 or peak['preparationTasks'] > 1: failures.append('P5 resource budget')
    if r['final']['resources']['domNodes'] > r['initial']['resources']['domNodes']: failures.append('P5 retained DOM on return')
    if r['final']['current']['start'] != r['initial']['current']['start']: failures.append('P5 return anchor')
    if long['misses'] or any(work.values()): failures.append('P5 readiness/interaction work')
    if long['longTasksDuringReading']: failures.append('P5 reading long task')
if '--check' in sys.argv:
    if len(traces) != 5: failures.append('Need four network/width traces and touch trace')
    if len(normal) != 4: failures.append('Need four normal reading groups')
    if len(display) != 2: failures.append('Need desktop and mobile display changes')
    if not long_file.exists(): failures.append('Need 20-minute long reading')
summary['acceptance'] = dict(passed=not failures, failures=failures)
(ROOT / 'summary.json').write_text(json.dumps(summary, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
print(json.dumps({**summary, 'traces': [{k: v for k, v in t.items() if k != 'windows'} for t in traces]}, ensure_ascii=False, indent=2))
if '--check' in sys.argv and failures: raise SystemExit('\n'.join(failures))


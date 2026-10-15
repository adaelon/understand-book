"""FP7 joint acceptance for the recorded production-entry platform matrix.

Run from the repository root after all browsers have stopped. Frame/pixel
analysis reuses FP4/FP6; this file checks the exact FP7 coverage and budgets.
"""
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path('docs/performance/focus-reading-fp7-20261015/results')
HERE = Path(__file__).parent
failures = []


def read(file):
    return json.loads(file.read_text(encoding='utf-8'))


def require(condition, message):
    if not condition:
        failures.append(message)


def work_keys(suite):
    return ['preparations', 'decorations'] + (['alignments', 'geometryProbes'] if suite == 'prose' else ['canvases', 'textLayers', 'textChunks'])


def work(samples, suite):
    return {k: max(s['after'][k] - s['before'][k] for s in samples if s.get('after')) for k in work_keys(suite)}


def limits(suite, width):
    if suite == 'pdf':
        return dict(pages=3 if width <= 600 else 6, backingPixels=12_000_000, preparationTasks=1)
    return dict(screens=4, pages=4 if width <= 600 else 8, sourceBytes=524_288,
                parsedBytes=2_097_152, domNodes=12_000, decodedImagePixels=8_000_000, preparationTasks=1)


def check_budget(peak, suite, width, label):
    for key, maximum in limits(suite, width).items():
        require(peak[key] <= maximum, f'{label}: {key} budget')


def long_tasks(raw, samples, supported):
    if not supported:
        return None
    return [t for t in raw['longTasks'] if samples[0]['input'] <= t['start'] <= samples[-1]['end']]


summary = dict(platforms={}, physicalPresentation=None,
               unmeasured=['Physical display latency and physical dropped frames',
                           'Physical phones and operating-system selection handles',
                           'WebKit timestamped filmstrip/compositor frames and Long Tasks API'])
first_pdf = read(ROOT / 'chromium/pdf-first-normal/normal-1440-local.json')
summary['knownObservations'] = [dict(
    evidence='chromium/pdf-first-normal/normal-1440-local.json',
    readingLongTasks=long_tasks(first_pdf, first_pdf['samples'], True),
    diagnostic='chromium/pdf-frames/diagnostic-analysis.json',
    attribution=None,
    status='Three first-run tasks did not recur in the 200-turn diagnostic trace; original evidence retained. Final acceptance uses the diagnostic repeat for this group.')]

for engine, widths in [('chromium', [1440, 390]), ('webview2', [1440]), ('webkit', [390])]:
    platform = summary['platforms'][engine] = {}
    for suite in ['prose', 'pdf']:
        root = ROOT / engine / suite
        label = f'{engine}/{suite}'
        evidence = platform[suite] = dict(normal=[], frames=None, displayChanges=None)
        if engine != 'webkit':
            script = HERE / ('summarize-focus-turn.py' if suite == 'prose' else 'summarize-focus-pdf.py')
            # These older per-slice reports have a different full-matrix check;
            # FP7 validates its explicit platform coverage below.
            run = subprocess.run([sys.executable, str(script), '--root', str(root)], capture_output=True)
            if run.returncode:
                raise RuntimeError(run.stderr.decode('utf-8', errors='replace'))
            analyzed = read(root / 'summary.json')
            traces = analyzed['traces']
            require(len(traces) == (5 if engine == 'chromium' else 2), label + ': frame groups')
            evidence['frames'] = [{k: v for k, v in t.items() if k != 'windows'} for t in traces]
            for trace in traces:
                name = label + '/' + trace['file']
                p1 = trace['inputToChangedReaderCaptureMs']
                require(p1['count'] == 20 and p1['p95'] is not None and p1['p95'] <= 50, name + ': P1')
                ratio = trace['browserFrames']['missingRatio']
                require(ratio is not None and ratio <= .01, name + ': P2')
                require(not trace['longTasksDuringReading'], name + ': long task')
                # Read the source counters as the earlier prose summarizer's
                # default mode predates annotations.
                raw = read(root / trace['file'].replace('.trace.json.gz', '.json'))
                require(not any(work(raw['samples'], suite).values()), name + ': P6')
                trace_width = 390 if 'touch' in trace['file'] or '-390-' in trace['file'] else 1440
                check_budget(raw['final']['peak'], suite, trace_width, name)
            if suite == 'prose':
                display = []
                for width in widths:
                    raw = read(root / f'p4-{width}.json')
                    require(len(raw['samples']) == 80, label + ': P4 samples')
                    require(len(raw['timing']) == 4 and all(v['count'] == 20 and v['p95'] <= 100 for v in raw['timing'].values()), label + ': P4 timing')
                    require(all(s['oldStillVisible'] for s in raw['samples']), label + ': P4 old frame')
                    check_budget(raw['final']['peak'], suite, width, label + ': P4')
                    display.append(dict(width=width, inputToReadableScreenMs=raw['timing'],
                                        screencastUpperBoundSamples=sum(s.get('captureSource') == 'Page.screencastFrame' for s in raw['samples'])))
                evidence['displayChanges'] = display
            else:
                evidence['displayChanges'] = analyzed['displayChanges']
                require(len(evidence['displayChanges']) == len(widths), label + ': P4 groups')
                for item in evidence['displayChanges']:
                    timing = item['inputToReadableScreenMs']
                    require(len(item['captures']) == 60 and len(timing) == 3 and all(v['count'] == 20 and v['p95'] <= 100 for v in timing.values()), label + ': P4 timing')
                require(not any('P4' in f or 'readable capture' in f for f in analyzed['acceptance']['failures']), label + ': P4 continuity')
        for width in widths:
            for network in ['local', 'weak']:
                file = root / f'normal-{width}-{network}.json'
                raw = read(file)
                samples = raw['samples']
                turns = sum(s['before']['current']['id'] != s['after']['current']['id'] for s in raw['states'])
                misses = max(s['after']['metrics']['misses'] for s in raw['states'])
                peak = {k: max(s['after']['peak'][k] for s in raw['states']) for k in raw['states'][0]['after']['peak']}
                changes = work(samples, suite)
                tasks = long_tasks(raw, samples, engine != 'webkit')
                intervals = sorted(b['input'] - a['input'] for a, b in zip(samples, samples[1:]))
                item = dict(file=file.name, browser=raw['browser'], width=width, network=network, turns=turns,
                            environment=raw.get('environment'), elapsedMs=raw['elapsedMs'], misses=misses, peak=peak,
                            actualInputIntervalMs=dict(min=min(intervals), p50=intervals[len(intervals)//2], max=max(intervals)),
                            maxWorkDuringInteraction=changes, longTasksDuringReading=tasks)
                evidence['normal'].append(item)
                name = label + '/' + file.name
                require(len(samples) == 200 and turns == 200 and misses == 0 and raw['elapsedMs'] >= 199_000, name + ': P3')
                require(min(intervals) >= 900, name + ': P3 catch-up inputs')
                require(intervals[len(intervals)//2] <= 1050, name + ': P3 automation overhead')
                require(not any(changes.values()), name + ': P6')
                require(not tasks, name + ': long task')
                check_budget(peak, suite, width, name)

for engine, suite, width in [('chromium', 'prose', 1440), ('webview2', 'pdf', 1440), ('webkit', 'prose', 390)]:
    raw = read(ROOT / engine / suite / 'long-reading.json')
    label = f'{engine}/{suite}/long-reading'
    peak = raw['final']['peak']
    changes = work(raw['interactions'], suite)
    tasks = long_tasks(raw, raw['interactions'], engine != 'webkit')
    item = dict(elapsedMs=raw['elapsedMs'], turns=raw['turns'], width=width, peak=peak,
                initialResources=raw['initial']['resources'], returnedResources=raw['final']['resources'],
                maxWorkDuringInteraction=changes, longTasksDuringReading=tasks,
                misses=max(s['after']['metrics']['misses'] for s in raw['samples']))
    summary['platforms'][engine][suite]['longReading'] = item
    require(item['turns'] >= 500 and item['elapsedMs'] >= 1_200_000, label + ': P5 duration/turns')
    require(len(raw['interactions']) >= raw['turns'], label + ': P5 interaction samples')
    if suite == 'pdf':
        require(sum(s['before']['current']['id'] != s['after']['current']['id'] for s in raw['samples']) == raw['turns'], label + ': P5 effective turns')
    else:
        require(raw.get('initialFontSize') is not None and raw.get('returnedFontSize') == raw['initialFontSize'], label + ': P5 same typography')
    require(raw['final']['current']['start'] == raw['initial']['current']['start'], label + ': P5 source return')
    require(item['returnedResources']['domNodes'] <= item['initialResources']['domNodes'], label + ': P5 retained DOM')
    require(not any(changes.values()) and not item['misses'], label + ': P5 readiness/work')
    require(not tasks, label + ': P5 long task')
    check_budget(peak, suite, width, label)

summary['acceptance'] = dict(passed=not failures, failures=failures)
(ROOT / 'summary.json').write_text(json.dumps(summary, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
print(json.dumps(summary['acceptance'], ensure_ascii=False, indent=2))
if failures:
    raise SystemExit(1)

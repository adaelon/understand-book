"""Plot the measured FP7 resident resources against the frozen budgets."""
import json
from pathlib import Path
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt

ROOT = Path('docs/performance/focus-reading-fp7-20261015/results')
fig, axes = plt.subplots(3, 1, figsize=(10, 10), constrained_layout=True)
groups = [('chromium', 'prose', 8), ('webview2', 'pdf', 6), ('webkit', 'prose', 4)]
for axis, (engine, suite, page_limit) in zip(axes, groups):
    raw = json.loads((ROOT / engine / suite / 'long-reading.json').read_text(encoding='utf-8'))
    if suite == 'prose':
        points = [(0, raw['initial']['resources'])] + [(s['turns'], s['after']['resources']) for s in raw['samples']]
        bounds = dict(pages=page_limit, domNodes=12_000, sourceBytes=524_288, parsedBytes=2_097_152, decodedImagePixels=8_000_000)
    else:
        points = [(0, raw['initial']['resources'])] + [(i + 1, s['after']['resources']) for i, s in enumerate(raw['samples'])]
        bounds = dict(pages=page_limit, backingPixels=12_000_000)
    points.append((raw['turns'] + 20, raw['final']['resources']))
    for key, limit in bounds.items():
        axis.plot([p[0] for p in points], [p[1][key] / limit * 100 for p in points], label=key, linewidth=1.5)
    axis.axhline(100, color='#a33a32', linestyle='--', linewidth=1, label='Frozen limit')
    axis.axvline(raw['turns'], color='#888', linestyle=':', linewidth=1)
    axis.set_title(f'{engine} / {suite} — {raw["turns"]} turns, {raw["elapsedMs"] / 60000:.2f} minutes', loc='left')
    axis.set_ylabel('% of resource budget')
    axis.set_ylim(0, 110)
    axis.set_xticks([0, 100, 300, 500, raw['turns'] + 20], ['Start', '100', '300', '500', 'Return'])
    axis.grid(axis='y', alpha=.2)
    axis.legend(loc='upper left', bbox_to_anchor=(1.01, 1), fontsize=8)
axes[-1].set_xlabel('Completed reading turns; Return is the original source anchor')
fig.suptitle('FP7: Resident resources during production-entry reading', fontsize=15)
fig.savefig(ROOT / 'resource-curves.png', dpi=160)
fig.savefig(ROOT / 'resource-curves.svg')

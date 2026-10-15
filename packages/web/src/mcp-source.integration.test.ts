import { readFileSync } from 'node:fs';
import { expect, it } from 'vitest';
import { readMcpSource, resolveMcpSource } from './mcp-source';
import type { Manifest } from './api';

// Exported by the real HTTP/SQLite test mcp9_two_accounts_purchase_recovery_and_source_evidence.
const evidence = JSON.parse(readFileSync(new URL('../../../docs/performance/beta-mcp/source-evidence.json', import.meta.url), 'utf8'));
it('opens actual HTTP source URLs and reconstructs every UTF-16 page in Reader', () => {
  const manifest: Manifest = { tree: evidence.tree, stats_by_lid: {} };
  let combined = '';
  let end = 0;
  for (const page of evidence.pages) {
    const source = readMcpSource(new URL(page.sources[0].source_url).search)!;
    expect(source.published_book_ref).toEqual(evidence.published_book_ref);
    expect(source.start_utf16).toBe(end);
    const target = resolveMcpSource(source, manifest);
    const text = [...target.ranges].map(([lid, range]) => {
      const node = manifest.tree.find(node => node.lid === lid)!;
      return evidence.source_text.slice(node.span.start, node.span.end).slice(...range);
    }).join('');
    expect(text).toBe(page.data.text);
    expect(source.end_utf16 - source.start_utf16).toBe(text.length);
    combined += text;
    end = source.end_utf16;
  }
  expect(combined).toBe(evidence.source_text);
  expect(evidence.pages).toHaveLength(2);
});

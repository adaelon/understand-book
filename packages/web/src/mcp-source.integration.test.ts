import { readFileSync } from 'node:fs';
import { expect, it } from 'vitest';
import { readMcpSource, resolveMcpSource } from './mcp-source';
import type { Manifest } from './api';

interface SourceEvidence {
  tree: Manifest['tree'];
  published_book_ref: { book_id: string; publication_id: string };
  source_text: string;
  pages: Array<{ sources: Array<{ source_url: string }>; data: { text: string } }>;
}

function expectPagesReconstructSource(evidence: SourceEvidence) {
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
}

it('reconstructs a two-page source URL across leaves with UTF-16 offsets', () => {
  // Original public fixture; the second page starts inside the first leaf.
  expectPagesReconstructSource({
    published_book_ref: { book_id: 'public-example', publication_id: 'original' },
    source_text: '原文😀甲乙续页丙丁',
    tree: [
      { lid: '1', kind: 'chapter', children: ['1.1', '1.2'], span: { start: 0, end: 10 } },
      { lid: '1.1', kind: 'paragraph', children: [], span: { start: 0, end: 6 } },
      { lid: '1.2', kind: 'paragraph', children: [], span: { start: 6, end: 10 } },
    ],
    pages: [
      { sources: [{ source_url: 'http://localhost/?book_id=public-example&publication_id=original&start_lid=1&end_lid=1&start_utf16=0&end_utf16=4' }], data: { text: '原文😀' } },
      { sources: [{ source_url: 'http://localhost/?book_id=public-example&publication_id=original&start_lid=1&end_lid=1&start_utf16=4&end_utf16=10' }], data: { text: '甲乙续页丙丁' } },
    ],
  });
});

// Optional evidence exported by mcp9_two_accounts_purchase_recovery_and_source_evidence.
// Explicit input keeps private HTTP/SQLite runs out of the public source distribution.
const evidencePath = process.env.MCP_SOURCE_EVIDENCE;
it.skipIf(!evidencePath)('opens actual HTTP source URLs and reconstructs every UTF-16 page in Reader', () => {
  expectPagesReconstructSource(JSON.parse(readFileSync(evidencePath!, 'utf8')));
});

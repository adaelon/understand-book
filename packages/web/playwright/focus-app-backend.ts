import type { Page } from '@playwright/test';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { observeProductionEntry } from './focus-production-entry';

export async function backend(page: Page, material = 'complex', chunk?: string, supplied?: any[]) {
  await observeProductionEntry(page);
  const chunks = chunk ? [chunk] : Array.from({ length: 10 }, (_, i) => String(i));
  const sources = supplied ?? (await Promise.all(chunks.map(c => readFile(path.resolve(`../../tmp/focus-reading-fp1/public/fp1/${material}/${c}.json`), 'utf8').then(JSON.parse)))).flat();
  let offset = 0;
  const tree = sources.map((s: any) => { const start = offset; offset += s.text.length + 2; return { lid: s.lid, kind: s.kind, children: [], span: { start, end: offset - 2 } }; });
  const parents = [...new Set(sources.map((s: any) => s.parent).filter(Boolean))].map(lid => {
    const children = tree.filter((n: any) => n.lid.startsWith(`${lid}.`));
    return { lid, kind: 'chapter', title: `Chapter ${lid}`, children: children.map((n: any) => n.lid), span: { start: children[0].span.start, end: children.at(-1)!.span.end } };
  });
  const manifest = [...parents, ...tree].sort((a, b) => a.span.start - b.span.start || b.span.end - a.span.end);
  const source = sources.map((s: any) => s.text).join('\n\n');
  const calls: { method: string; path: string; at: number; body?: any }[] = [];
  const control = { offline: false, delay: 0, downloadBytesPerMs: 0, formulaDelay: 0, positionDelay: 0, revision: 'fp3-source-1', position: null as any, annotations: [] as any[] };
  await page.route('**/api/**', async route => {
    const request = route.request(), url = new URL(request.url()), p = url.pathname.slice(4);
    calls.push({ method: request.method(), path: p + url.search, at: Date.now(), body: request.postData() ? request.postDataJSON() : undefined });
    const answer = (value: unknown, status = 200) => route.fulfill({ status, contentType: 'application/json', body: JSON.stringify(value) }).catch(() => {});
    if (p === '/book/text') {
      if (control.offline) return route.abort();
      if (control.delay) await new Promise(r => setTimeout(r, control.delay));
      const lid = url.searchParams.get('lid'), end = url.searchParams.get('end') ?? lid;
      const a = tree.find((n: any) => n.lid === lid), b = tree.find((n: any) => n.lid === end);
      if (!a || !b) return answer({ error_code: 'NOT_FOUND' }, 404);
      const text = source.slice(a.span.start, b.span.end);
      if (control.downloadBytesPerMs) await new Promise(r => setTimeout(r, Buffer.byteLength(text) / control.downloadBytesPerMs));
      return answer({ lid, text });
    }
    if (p === '/book/formula_semantics') { if (control.formulaDelay) await new Promise(r => setTimeout(r, control.formulaDelay)); return answer({ error_code: 'NOT_FOUND' }, 404); }
    if (p === '/reader/position') {
      if (control.positionDelay) await new Promise(r => setTimeout(r, control.positionDelay));
      control.position = request.postDataJSON();
      const lids = [...new Set(control.position.ranges.map((r: any) => r.lid))];
      return answer({ ok: true, viewport: { anchor_lid: control.position.lid, top_lid: control.position.lid, bottom_lid: lids.at(-1), width: 8, visible_lids: lids } });
    }
    if (p === '/reader/highlight') {
      const body = request.postDataJSON(), text = sources.find((s: any) => s.lid === body.lid).text;
      const id = `fp5-hl-${control.annotations.length}`;
      control.annotations.push({ mem_id: id, type: 'highlight', layer: 'long_term', book_id: material, anchor: { lid: body.lid }, range: body.range, content: text.slice(body.range.start, body.range.end), source_session_id: body.source_session_id });
      return answer({ ok: true, highlight_id: id });
    }
    if (p === '/memory/save') {
      const body = request.postDataJSON(), id = `fp5-note-${control.annotations.length}`;
      control.annotations.push({ ...body, anchor: { lid: body.selection_context?.ranges[0]?.lid }, note: undefined, mem_id: id, book_id: material }); return answer({ status: 'CREATED', mem_id: id });
    }
    if (p === '/agent/runs') return answer({ answer: '已解释当前原文。', answer_view: { parts: [] }, effects: [] });
    if (p === '/reader/goto') {
      const lid = request.postDataJSON().lid, index = tree.findIndex((n: any) => n.lid === lid || n.lid.startsWith(`${lid}.`));
      const visible = tree.slice(Math.max(0, index), Math.max(0, index) + 8).map((n: any) => n.lid);
      return answer({ viewport: { anchor_lid: lid, top_lid: lid, bottom_lid: visible.at(-1), width: visible.length, visible_lids: visible } });
    }
    const first = tree[0].lid;
    const data: Record<string, unknown> = {
      '/desktop/status': { desktop_host: false, reader_only: true, active_book: true },
      '/book/build_workbench': { book_id: material, readiness: { route: 'reader', reasons: [], stages: {} }, input: { manifest: null }, source_review: { unresolved: [] } },
      '/book/manifest': { tree: manifest, stats_by_lid: {} },
      '/book/asset_manifest': { images: sources.map((s: any) => s.imageAsset).filter(Boolean) },
      '/book/source_fingerprint': { book_id: material, source_fingerprint: control.revision },
      '/book/source_manifest': null,
      '/reader/state': { book_id: material, revision: 0, visible_position: control.position, viewport: control.position ? { anchor_lid: control.position.lid, top_lid: control.position.lid, bottom_lid: control.position.ranges.at(-1).lid, width: 8, visible_lids: [...new Set(control.position.ranges.map((r: any) => r.lid))] } : { anchor_lid: first, top_lid: first, bottom_lid: tree[7]?.lid ?? first, width: 8, visible_lids: tree.slice(0, 8).map((n: any) => n.lid) }, profile: { profile_id: 'technical_learning', profile_version: '1' }, layout: { rev: 0, open_slots: [] } },
      '/profile/manifest': { profile_id: 'technical_learning', slots: [] },
      '/memory/recall': control.annotations, '/memory/profile': {}, '/profile/memory': {}, '/profile/backfill': { jobs: [], sessions: [] },
      '/agent/history': { active_session_id: 'fp3-chat', sessions: [], current: { id: 'fp3-chat', book_id: material, turns: [], goals: [] } },
      '/build_intent/artifacts': { overlay: null }, '/build_intent/usage.event': {},
      '/tutor/state': { control: { enabled: false, revision: 0, current_tutor_session_id: null }, sessions: {} },
      '/tutor/readiness': { status: 'ready', limitations: [], reason: 'ready', teaching_assets: { status: 'ready', limitations: [] } },
    };
    return p in data ? answer(data[p]) : answer({ error_code: 'NOT_FOUND', message: p }, 404);
  });
  // Real imported external image URLs can be unavailable. No external network
  // dependency in acceptance; the product must keep a stable missing-image box.
  await page.route(/^https?:\/\/(?!127\.0\.0\.1)/, route => route.abort());
  return { control, calls, sources };
}


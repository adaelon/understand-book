import { expect, test, type Page, type Route } from "@playwright/test";
import { readFileSync } from 'node:fs';

type ResolveStatus = "resolved" | "partial" | "unresolved";

function pdfFixture(content = "BT /F1 20 Tf 72 700 Td (Selectable PDF fixture text for explicit actions.) Tj ET"): Buffer {
  const objects = [
    "<< /Type /Catalog /Pages 2 0 R >>",
    "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
    "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>",
    "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
    `<< /Length ${Buffer.byteLength(content, "ascii")} >>\nstream\n${content}\nendstream`,
  ];
  let body = "%PDF-1.4\n";
  const offsets = [0];
  objects.forEach((object, index) => {
    offsets.push(Buffer.byteLength(body, "ascii"));
    body += `${index + 1} 0 obj\n${object}\nendobj\n`;
  });
  const xrefOffset = Buffer.byteLength(body, "ascii");
  body += `xref\n0 ${objects.length + 1}\n`;
  body += "0000000000 65535 f \n";
  body += offsets.slice(1).map((offset) => `${String(offset).padStart(10, "0")} 00000 n \n`).join("");
  body += `trailer\n<< /Size ${objects.length + 1} /Root 1 0 R >>\nstartxref\n${xrefOffset}\n%%EOF\n`;
  return Buffer.from(body, "ascii");
}

function boundaryPdfFixture(): Buffer {
  return pdfFixture([
    "BT /F1 16 Tf",
    "72 740 Td (Selectable PDF fixture text for explicit actions.) Tj",
    "0 -24 Td (Boundary paragraph target.) Tj",
    "0 -48 Td (Boundary line target.) Tj",
    "0 -24 Td (Same paragraph continuation.) Tj",
    "0 -24 Td (Boundary filler line.) Tj",
    "0 -48 Td (Following paragraph.) Tj",
    "ET",
  ].join("\n"));
}

const profile = {
  profile_id: "technical_learning",
  profile_version: "fixture-v1",
  ui_slots: [],
  layout_presets: [],
  allowed_layout_actions: [],
  agent_tools: [],
};

const readerState = {
  viewport: {
    anchor_lid: "1.1",
    top_lid: "1.1",
    bottom_lid: "1.1",
    width: 1,
    visible_lids: ["1.1"],
  },
  open_panels: [],
  selection: null,
  layout: {
    rev: 0,
    active_preset: null,
    open_slots: [],
    focused_slot: null,
    pinned_evidence: [],
    panel_sizes: {},
    slot_order: {},
  },
  profile,
};

const sourceMap = {
  version: "pdf_source_map.v1",
  book_id: "pdf-selection-actions",
  coordinate_system: {
    space: "pdf_user_space",
    origin: "bottom_left",
    unit: "pt",
    rotation_applied: false,
  },
  pages: [{ pageIndex: 0, page_label: "1", width: 612, height: 792, rotate: 0, view: [0, 0, 612, 792] }],
  entries: [],
  excluded_regions: [],
  page_region_index: {},
  page_excluded_index: {},
  config_hash: "fixture-v1",
};

function json(route: Route, value: unknown, status = 200) {
  return route.fulfill({
    status,
    contentType: "application/json",
    body: JSON.stringify(value),
  });
}

async function installApiFixture(
  page: Page,
  resolveStatus: ResolveStatus,
  resolveDelayMs = 0,
  pdf = pdfFixture(),
  resolutionBasis?: "exact" | "recovered",
) {
  const calls = {
    highlights: [] as Record<string, unknown>[],
    notes: [] as Record<string, unknown>[],
    agent: [] as Record<string, unknown>[],
    translations: [] as Record<string, unknown>[],
    replacements: [] as Record<string, unknown>[],
    opens: [] as Record<string, unknown>[],
    resolves: 0,
    resolveRequests: [] as Record<string, unknown>[],
  };
  const records: Record<string, unknown>[] = [];
  let nextId = 1;
  const history = {
    active_session_id: "chat-fixture",
    sessions: [{
      id: "chat-fixture",
      title: "Fixture",
      created_at: "2026-07-13T00:00:00Z",
      updated_at: "2026-07-13T00:00:00Z",
      turn_count: 0,
      turns: [],
    }],
    current: {
      id: "chat-fixture",
      book_id: "pdf-selection-actions",
      title: "Fixture",
      created_at: "2026-07-13T00:00:00Z",
      updated_at: "2026-07-13T00:00:00Z",
      turns: [],
    },
  };

  await page.route("**/api/**", async (route) => {
    const request = route.request();
    const path = new URL(request.url()).pathname;
    const body = request.postData() ? request.postDataJSON() as Record<string, unknown> : null;
    if (path === "/api/book/pdf/original") {
      return route.fulfill({ status: 200, contentType: "application/pdf", body: pdf });
    }
    if (path === "/api/desktop/status") {
      return json(route, {
        desktop_host: false,
        active_book: true,
        book_dir: null,
        library_root: "",
        library_root_available: true,
      });
    }
    if (path === "/api/book/build_workbench") {
      return json(route, {
        version: "build_workbench_snapshot.v1",
        book_id: "pdf-selection-actions",
        readiness: { route: "reader", status: "trusted_book", reasons: [], stages: {} },
        input: { manifest: null, fingerprint: null, ready: true },
        jobs: [],
        source_review: {
          report: null,
          unresolved: [],
          review_draft_markdown: null,
          decisions: null,
          ready_for_rerun: false,
        },
        operations: { warnings: [], permission_audit: [] },
      });
    }
    if (path === "/api/book/manifest") {
      return json(route, {
        tree: [{
          lid: "1.1",
          children: [],
          span: { start: 0, end: 52 },
          kind: "paragraph",
        }],
        stats_by_lid: {},
      });
    }
    if (path === "/api/book/asset_manifest") {
      return json(route, { version: "asset_manifest.v1", book_id: "pdf-selection-actions", images: [] });
    }
    if (path === "/api/book/source_fingerprint") {
      return json(route, { book_id: "pdf-selection-actions", source_fingerprint: "source" });
    }
    if (path === "/api/book/source_manifest") {
      return json(route, {
        version: "source_manifest.v2",
        book_id: "PDF selection action fixture",
        canonical_source: {
          kind: "reconciled_markdown",
          path: "source.txt",
          citation_anchor: "lid",
          sha256: "source",
        },
        original_pdf: { path: "fixture.pdf", sha256: "pdf", citation_anchor: false },
        capabilities: {
          view_pdf: { status: "available" },
          project_lid_to_pdf: { status: "degraded", reason: "fixture quality gate" },
          resolve_pdf_selection: { status: "available" },
          project_ranges_to_pdf: { status: "degraded", reason: "fixture quality gate" },
        },
        alignment_quality: {
          policy_version: "hybrid_quality_policy.v1",
          tier: "degraded",
          unit_location_ratio: 0.8465608466,
          exact_text_span_ratio: 0.6627206241,
          exact_formula_ratio: 0.3168202765,
          heading_location_ratio: 0.976744186,
          report_path: "alignment_report.json",
        },
      });
    }
    if (path === "/api/book/pdf_source_map") return json(route, sourceMap);
    if (path === "/api/profile/manifest") {
      return json(route, {
        ...profile,
        projections: [],
        guided_reading_policy: {},
        defaults: {},
      });
    }
    if (path === "/api/book/text") {
      return json(route, { lid: new URL(request.url()).searchParams.get("lid") ?? "1.1", text: "Selectable PDF fixture text for explicit actions." });
    }
    if (path === "/api/reader/state") return json(route, readerState);
    if (path === '/api/memory/delete') {
      const index = records.findIndex(record => record.mem_id === body?.mem_id);
      if (index >= 0) records.splice(index, 1);
      return json(route, { ok: true });
    }
    if (path === "/api/memory/recall") return json(route, records);
    if (path === "/api/reader/pdf_selection.resolve") {
      calls.resolves += 1;
      calls.resolveRequests.push(body ?? {});
      if (resolveDelayMs > 0) {
        await new Promise((resolve) => setTimeout(resolve, resolveDelayMs));
      }
      if (resolveStatus === "unresolved") {
        return json(route, { status: "unresolved", ranges: [], quote_markdown: "" });
      }
      return json(route, {
        status: resolveStatus,
        ...(resolveStatus === "partial" ? { diagnostic: "material_or_ambiguous" } : {}),
        ...(resolutionBasis ? {
          resolution_basis: resolutionBasis,
          ...(resolutionBasis === "recovered" ? {
            recovery_policy_version: "pdf_selection_recovery.v1",
            recovered_differences: ["layout_whitespace", "hyphen_representation"],
          } : {}),
        } : {}),
        ranges: [{
          lid: "1.1",
          range: { start: 0, end: 10 },
          source_span: { start: 0, end: 10 },
          quote_markdown: "Selectable",
        }],
        quote_markdown: "Selectable",
      });
    }
    if (path === "/api/reader/highlight") {
      calls.highlights.push(body ?? {});
      const memId = `highlight-${nextId++}`;
      records.push({
        mem_id: memId,
        type: "highlight",
        layer: "long_term",
        book_id: "pdf-selection-actions",
        anchor: { lid: body?.lid, concept: null },
        content: "Selectable",
        range: body?.range,
        source_session_id: body?.source_session_id ?? null,
      });
      return json(route, { ok: true, highlight_id: memId });
    }
    if (path === '/api/book/library') return json(route, { root: '', books: [] });
    if (path === '/api/book/open') { calls.opens.push(body ?? {}); return json(route, { ok: true }); }
    if (path === '/api/memory/replace') {
      calls.replacements.push(body ?? {});
      const index = records.findIndex(record => record.mem_id === body?.mem_id);
      records[index] = { ...records[index], mem_id: `edited-${nextId++}`, content: body?.content };
      return json(route, records[index]);
    }
    if (path === "/api/memory/save") {
      calls.notes.push(body ?? {});
      const memId = `note-${nextId++}`;
      records.push({
        mem_id: memId,
        type: "note",
        layer: body?.layer ?? "long_term",
        book_id: "pdf-selection-actions",
        anchor: { lid: body?.anchor_lid ?? (body?.selection_context as any)?.ranges[0]?.lid, concept: null },
        content: body?.content,
        selection_context: body?.selection_context,
        note: body?.note ? { material: { book_id: 'pdf-selection-actions', publication_id: null },
          association: (body.note as any).association,
          retained_excerpt: { kind: (body.note as any).association.kind === 'selection' ? 'original' : 'assistant', text: (body.note as any).retained_excerpt }, source_bindings: [] } : undefined,
      });
      return json(route, records.at(-1));
    }
    if (path === "/api/reader/pdf_ranges.project") {
      const ranges = (body?.ranges ?? []) as Array<{ lid: string; range: { start: number; end: number } }>;
      return json(route, {
        projections: ranges.map((selected) => ({
          ...selected,
          status: "exact",
          ...(resolutionBasis ? {
            resolution_basis: resolutionBasis,
            ...(resolutionBasis === "recovered" ? {
              recovery_policy_version: "pdf_selection_recovery.v1",
              recovered_differences: ["layout_whitespace", "hyphen_representation"],
            } : {}),
          } : {}),
          rects: [{
            pageIndex: 0,
            bbox: [72, 696, 164, 716],
            source_span: selected.range,
          }],
          covered_range: selected.range,
          terminal_rect: {
            pageIndex: 0,
            bbox: [156, 696, 164, 716],
            source_span: { start: selected.range.end - 1, end: selected.range.end },
          },
        })),
      });
    }
    if (path === "/api/reader/selection.translate") {
      calls.translations.push(body ?? {});
      return json(route, {
        translation_markdown: "前馈网络",
        target_locale: "zh-CN",
      });
    }
    if (path === "/api/agent/history") return json(route, history);
    if (path === "/api/agent/runs") {
      calls.agent.push(body ?? {});
      return json(route, {
        answer: "fixture answer",
        incomplete: false,
        warning: null,
        turns: 1,
        tokens_spent: 1,
        effects: [],
        trace: [],
      });
    }
    return json(route, {
      error_code: "UNMOCKED",
      category: "internal",
      message: `Unmocked fixture route: ${path}`,
    }, 500);
  });
  return Object.assign(calls, { records, history });
}

for (const width of [390, 1440]) test(`RS1 previews and downloads selected notes without changing reading state at ${width}px`, async ({ page }, info) => {
  const calls = await installApiFixture(page, 'resolved');
  await page.route('**/api/book/paper_metadata', route => json(route, { available: true, title: { value: '学习与理解' } }));
  const base = { type: 'note', layer: 'long_term', book_id: 'pdf-selection-actions', anchor: {} };
  calls.records.push(
    { ...base, mem_id: 'rs1-legacy', content: '记录一个问题，给思考留出空间。' },
    { ...base, mem_id: 'rs1-original', anchor: { lid: '1.1' }, content: '先写下自己的理解，再回到原文核对。', note: {
      material: { book_id: 'pdf-selection-actions', publication_id: null }, association: { kind: 'selection' },
      retained_excerpt: { kind: 'original', text: '阅读，是把文字和已有经验连接起来。' }, source_bindings: [] } },
    { ...base, mem_id: 'rs1-presentation', content: '改变一个条件，观察结果如何变化。', note: {
      material: { book_id: 'pdf-selection-actions', publication_id: null }, association: { kind: 'presentation', title: '斜率与变化',
        receipt: { session_id: 'private-chat', turn_id: 'private-turn', reference: { presentation_id: 'private-presentation', revision: 2 }, state_revision: 1, saved_state_ref: 'private-state' } },
      retained_excerpt: null, source_bindings: [] } },
  );
  const originalRecords = JSON.stringify(calls.records);
  await page.addInitScript(() => {
    const draw = CanvasRenderingContext2D.prototype.drawImage;
    CanvasRenderingContext2D.prototype.drawImage = function(...args: any[]) {
      const image = args[0];
      if (this.canvas.width === 1080 && image instanceof HTMLImageElement && image.src.startsWith('data:image/svg+xml')) {
        const xml = new TextDecoder().decode(Uint8Array.from(atob(image.src.split(',')[1]), ch => ch.charCodeAt(0)));
        const doc = new DOMParser().parseFromString(xml, 'image/svg+xml');
        doc.querySelectorAll('style').forEach(el => el.remove());
        (window as any).sharePainted.push(doc.documentElement.textContent);
      }
      return (draw as any).apply(this, args);
    };
    const original = CanvasRenderingContext2D.prototype.fillText;
    (window as any).sharePainted = [];
    CanvasRenderingContext2D.prototype.fillText = function(text, x, y, maxWidth) {
      if (this.canvas.width === 1080 && this.canvas.height === 1440) (window as any).sharePainted.push(text);
      return original.call(this, text, x, y, maxWidth);
    };
  });
  await page.setViewportSize({ width, height: 900 }); await page.goto('/');
  await expect(page.getByRole('button', { name: '适合栏宽', exact: true })).toBeVisible();
  if (width < 1024) await page.locator('.workspace-mobile-nav').getByRole('button', { name: '笔记', exact: true }).click();
  else { await page.getByRole('button', { name: '阅读工具', exact: true }).click(); await page.getByRole('tab', { name: '笔记', exact: true }).click(); }
  for (const id of ['rs1-original', 'rs1-legacy', 'rs1-presentation']) {
    const card = page.locator(`#reader-panel-notes [data-mem-id="${id}"]`);
    await card.locator('summary').click();
    const open = card.getByRole('button', { name: '生成分享图', exact: true });
    await open.scrollIntoViewIfNeeded();
    const before = await page.locator('.pdf-page-list').evaluate(el => el.scrollTop);
    await open.click();
    const panel = page.getByRole('dialog', { name: '生成分享图', exact: true });
    await expect(panel).toBeVisible();
    await panel.getByLabel('标题', { exact: true }).fill('把阅读变成思考');
    await panel.getByLabel('我的感想', { exact: true }).fill('今天多理解了一点。');
    if (id === 'rs1-original') {
      await panel.getByLabel('排版').selectOption('excerpt');
      await panel.getByLabel('配色').selectOption('blue');
      await panel.getByRole('checkbox', { name: '我的笔记' }).uncheck();
      await expect(panel).toContainText('《学习与理解》');
    }
    await page.evaluate(() => { (window as any).sharePainted = []; });
    if (id === 'rs1-original' && width === 1440) {
      await page.evaluate(() => {
        const original = HTMLCanvasElement.prototype.toBlob;
        HTMLCanvasElement.prototype.toBlob = function(callback, ...args) {
          HTMLCanvasElement.prototype.toBlob = original;
          callback(null);
        };
      });
      await panel.getByRole('button', { name: '预览图片', exact: true }).click();
      await expect(panel.getByRole('alert')).toContainText('图片生成失败');
      await expect(panel.getByLabel('我的感想', { exact: true })).toHaveValue('今天多理解了一点。');
    }
    await panel.getByRole('button', { name: '预览图片', exact: true }).click();
    const preview = panel.getByRole('img'); await expect(preview).toBeVisible();
    const painted = await page.evaluate(() => (window as any).sharePainted.join(''));
    expect(painted).toContain('把阅读变成思考'); expect(painted).toContain('今天多理解了一点。'); expect(painted).not.toContain('private-');
    if (id === 'rs1-original') { expect(painted).toContain('阅读，是把文字和已有经验连接起来。'); expect(painted).not.toContain('先写下自己的理解'); }
    if (id === 'rs1-legacy') { expect(painted).toContain('阅读笔记'); expect(painted).toContain('无已记录出处'); expect(painted).not.toContain('学习与理解'); }
    if (id === 'rs1-presentation') expect(painted).toContain('记录于演示「斜率与变化」');
    expect(await page.evaluate(() => document.fonts.check('400 44px "Noto Serif SC Variable"', '阅读理解'))).toBe(true);
    const bytes = await preview.evaluate(async img => Array.from(new Uint8Array(await (await fetch((img as HTMLImageElement).src)).arrayBuffer())));
    const downloadPromise = page.waitForEvent('download');
    await panel.getByRole('button', { name: '下载 PNG', exact: true }).click();
    const download = await downloadPromise; const path = info.outputPath(`${id}-${width}.png`); await download.saveAs(path);
    expect(readFileSync(path).equals(Buffer.from(bytes))).toBe(true);
    const decoded = await page.evaluate(async base64 => {
      const bitmap = await createImageBitmap(await (await fetch(`data:image/png;base64,${base64}`)).blob());
      const canvas = document.createElement('canvas'); canvas.width = bitmap.width; canvas.height = bitmap.height;
      const ctx = canvas.getContext('2d')!; ctx.drawImage(bitmap, 0, 0);
      const pixels = ctx.getImageData(0, 0, canvas.width, canvas.height).data;
      let dark = 0; for (let i = 0; i < pixels.length; i += 4) if (.2126 * pixels[i] + .7152 * pixels[i + 1] + .0722 * pixels[i + 2] < 150) dark++;
      return { width: bitmap.width, height: bitmap.height, dark };
    }, readFileSync(path).toString('base64'));
    expect(decoded.width).toBe(1080); expect(decoded.height).toBe(1440); expect(decoded.dark).toBeGreaterThan(3000);
    await page.screenshot({ path: info.outputPath(`rs1-panel-${width}.png`) });
    await panel.getByRole('button', { name: '关闭分享' }).click();
    await expect(open).toBeFocused();
    expect(await page.locator('.pdf-page-list').evaluate(el => el.scrollTop)).toBe(before);
  }
  expect(JSON.stringify(calls.records)).toBe(originalRecords); expect(calls.notes).toEqual([]); expect(calls.replacements).toEqual([]); expect(calls.agent).toEqual([]);
});

async function selectFixtureText(page: Page, start: number, end: number) {
  const span = page.locator(".pdf-text-layer span").first();
  await expect(span).toHaveText(/Selectable PDF fixture/);
  await span.evaluate((element, offsets) => {
    const text = element.firstChild!;
    const range = document.createRange();
    range.setStart(text, offsets.start);
    range.setEnd(text, offsets.end);
    const selection = window.getSelection()!;
    selection.removeAllRanges();
    selection.addRange(range);
    element.closest(".pdf-page-list")!.dispatchEvent(new MouseEvent("mouseup", { bubbles: true }));
  }, { start, end });
}

test('RE6 zoom, fit and container changes keep real PDF text, canvas and highlights aligned', async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  const calls = await installApiFixture(page, 'resolved');
  await page.goto('/');
  const list = page.locator('.pdf-page-list');
  const text = page.locator('.pdf-text-layer span').first();
  await expect(text).toHaveText(/Selectable PDF fixture/);
  for (const factor of ['1.5', '2', '1']) {
    await page.getByLabel('PDF 缩放倍率（相对适宽）').selectOption(factor);
    await expect(list).toHaveAttribute('aria-busy', 'false');
    await expect(text).toHaveText(/Selectable PDF fixture/);
    await text.scrollIntoViewIfNeeded();
    await dragBetweenTextOffsets(page, 'Selectable PDF fixture text for explicit actions.', 0, 10, true);
    await expect(page.locator('.pdf-selection-toolbar')).toBeVisible();
    expect(await page.evaluate(() => getSelection()?.toString())).toBe('Selectable');
    await page.locator('.pdf-selection-toolbar').getByTitle('高亮').click();
    await expect.poll(() => calls.highlights.length).toBeGreaterThan(0);
    await expect(page.locator('.pdf-user-highlight').first()).toBeVisible();
    const geometry = await page.locator('.pdf-page-shell').first().evaluate(el => {
      const canvas = el.querySelector('canvas')!.getBoundingClientRect();
      const layer = el.querySelector('.pdf-text-layer')!.getBoundingClientRect();
      return { width: Math.abs(canvas.width - layer.width), height: Math.abs(canvas.height - layer.height), left: Math.abs(canvas.left - layer.left), top: Math.abs(canvas.top - layer.top) };
    });
    for (const delta of Object.values(geometry)) expect(delta).toBeLessThanOrEqual(1);
  }
  await page.getByRole('button', { name: '适合栏宽', exact: true }).click();
  await expect(page.getByLabel('PDF 缩放倍率（相对适宽）')).toHaveValue('1');
  const before = await page.locator('.pdf-user-highlight').first().getAttribute('style');
  await page.setViewportSize({ width: 390, height: 844 });
  await expect(list).toHaveAttribute('aria-busy', 'false');
  await expect(text).toHaveText(/Selectable PDF fixture/);
  expect(await page.locator('.pdf-user-highlight').first().getAttribute('style')).toBe(before);
  await selectFixtureText(page, 0, 10);
  await expect(page.locator('.pdf-selection-toolbar')).toBeVisible();
  await page.screenshot({ path: '../../docs/performance/reader-re5-re6/pdf-selection-mobile.png' });
});

test('RE6 licensed two-column formula PDF keeps original typography and column geometry', async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  const pdf = readFileSync('../core/test/fixtures/hybrid-foundation-goldset/v1/licensed-two-column-formula/paper.pdf');
  await installApiFixture(page, 'resolved', 0, pdf);
  await page.goto('/');
  const left = page.locator('.pdf-text-layer span').filter({ hasText: /^Left Formula$/ });
  const right = page.locator('.pdf-text-layer span').filter({ hasText: /^Right Formula$/ });
  await expect(left).toBeVisible(); await expect(right).toBeVisible();
  const positions = () => page.locator('.pdf-page-shell').first().evaluate(el => {
    const box = el.getBoundingClientRect();
    return [...el.querySelectorAll('.pdf-text-layer span')].filter(node => /^(Left Formula|Right Formula|Alpha beta|Lambda mu)/.test(node.textContent ?? '')).map(node => {
      const rect = node.getBoundingClientRect();
      return { text: node.textContent, x: (rect.left - box.left) / box.width, y: (rect.top - box.top) / box.height, width: rect.width / box.width, height: rect.height / box.height };
    });
  });
  const before = await positions(); expect(before).toHaveLength(4);
  await page.getByRole('button', { name: '阅读设置', exact: true }).click();
  await expect(page.locator('.reader-typography-panel')).toContainText('PDF 原版保留原始排版');
  await page.getByLabel('字号', { exact: true }).fill('22');
  await page.getByLabel('字体', { exact: true }).selectOption('sans');
  await page.getByRole('button', { name: '关闭阅读设置', exact: true }).click();
  expect(await positions()).toEqual(before);
  await page.getByLabel('PDF 缩放倍率（相对适宽）').selectOption('1.5');
  await expect(page.locator('.pdf-page-list')).toHaveAttribute('aria-busy', 'false');
  const after = await positions(); expect(after).toHaveLength(before.length);
  for (let i = 0; i < before.length; i++) {
    expect(after[i].text).toBe(before[i].text);
    for (const key of ['x', 'y', 'width', 'height'] as const) expect(Math.abs(after[i][key] - before[i][key])).toBeLessThan(0.003);
  }
  await page.getByRole('button', { name: '适合栏宽', exact: true }).click();
  await expect(page.locator('.pdf-page-list')).toHaveAttribute('aria-busy', 'false');
  await page.screenshot({ path: '../../docs/performance/reader-re5-re6/pdf-two-column.png' });
});

async function dragPastTextEnd(page: Page, text: string) {
  const span = page.locator(".pdf-text-layer span").filter({ hasText: text }).first();
  await expect(span).toHaveText(text);
  await span.scrollIntoViewIfNeeded();
  const box = await span.boundingBox();
  if (!box) throw new Error(`PDF text span has no box: ${text}`);
  const y = box.y + box.height / 2;
  await page.mouse.move(box.x + 2, y);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width + 2, y, { steps: 12 });
  await page.mouse.up();
}

async function dragBetweenTextOffsets(page: Page, text: string, start: number, end: number, alignStart = false) {
  const span = page.locator(".pdf-text-layer span").filter({ hasText: text }).first();
  await expect(span).toHaveText(text);
  await span.scrollIntoViewIfNeeded();
  if (alignStart) await span.evaluate(el => { el.closest('.pdf-page-list')!.scrollLeft = 0; });
  const points = await span.evaluate((element, offsets) => {
    const node = element.firstChild;
    if (!(node instanceof Text)) throw new Error("PDF text span has no text node");
    const characterRect = (offset: number) => {
      const range = document.createRange();
      range.setStart(node, offset);
      range.setEnd(node, offset + 1);
      return range.getBoundingClientRect();
    };
    const first = characterRect(offsets.start);
    const last = characterRect(offsets.end - 1);
    return {
      start: { x: first.left + 1, y: first.top + first.height / 2 },
      end: { x: last.right - 1, y: last.top + last.height / 2 },
    };
  }, { start, end });
  await page.mouse.move(points.start.x, points.start.y);
  await page.mouse.down();
  await page.mouse.move(points.end.x, points.end.y, { steps: 12 });
  await page.mouse.up();
}

test("resolved real PDF selection performs three explicit actions and sends structured AskQuote", async ({ page }, testInfo) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  const calls = await installApiFixture(page, "resolved");
  await page.goto("/");
  await expect(page.locator(".pdf-text-layer span").first()).toHaveText(/Selectable PDF fixture/);
  const quality = page.locator(".alignment-quality-bar");
  await quality.locator("summary").click();
  await expect(quality).toContainText("66.3%");
  await page.screenshot({ path: testInfo.outputPath("hf2-reader-degraded-desktop.png"), fullPage: true });
  await quality.locator("summary").click();

  await selectFixtureText(page, 0, 10);
  await page.locator(".pdf-selection-toolbar").getByTitle("高亮").click();
  await expect.poll(() => calls.highlights.length).toBe(1);
  expect(calls.highlights[0]).toMatchObject({ lid: "1.1", range: { start: 0, end: 10 } });
  await expect(page.locator(".pdf-user-highlight")).toHaveCount(1);

  await selectFixtureText(page, 0, 10);
  await page.locator(".pdf-selection-toolbar").getByTitle("笔记").click();
  await expect(page.getByRole("region", { name: "笔记编辑" })).toBeVisible();
  await page.getByRole("button", { name: "保存笔记", exact: true }).click();
  await expect.poll(() => calls.notes.length).toBe(1);
  expect(calls.notes[0]).toMatchObject({
    selection_context: {
      status: "resolved",
      ranges: [{ lid: "1.1", range: { start: 0, end: 10 } }],
    },
  });
  expect(calls.notes[0]).not.toHaveProperty("anchor_lid");
  await expect(page.locator(".pdf-note-marker")).toHaveCount(1);

  await selectFixtureText(page, 0, 10);
  await page.locator(".pdf-selection-toolbar").getByTitle("问 AI").click();
  await expect(page.locator(".ask-draft")).toContainText("Selectable");
  expect(calls.agent).toHaveLength(0);
  await page.locator(".agent-input textarea").fill("What does this mean?");
  await page.locator(".agent-compose-row > button").click();
  await expect.poll(() => calls.agent.length).toBe(1);
  expect(calls.agent[0]).toMatchObject({
    message: "What does this mean?",
    display_user: "What does this mean?",
    question_anchor_lid: "1.1",
    question_quote: {
      lid: "1.1",
      status: "resolved",
      raw_quote: "Selectable",
      resolved_quote: "Selectable",
      ranges: [{ lid: "1.1", range: { start: 0, end: 10 } }],
    },
  });
});

for (const resolutionBasis of ["exact", "recovered"] as const) {
  for (const viewport of [
    { name: "desktop", width: 1440, height: 900 },
    { name: "mobile", width: 390, height: 844 },
  ] as const) {
    test(`${resolutionBasis} real PDF selection keeps every action and refreshes as located on ${viewport.name}`, async ({ page }) => {
      await page.setViewportSize({ width: viewport.width, height: viewport.height });
      const calls = await installApiFixture(page, "resolved", 0, pdfFixture(), resolutionBasis);
      await page.goto("/");

      await selectFixtureText(page, 0, 10);
      let toolbar = page.locator(".pdf-selection-toolbar");
      await expect(toolbar.locator(".pdf-selection-status")).toHaveCount(0);
      await expect(toolbar.locator("button")).toHaveCount(5);
      await toolbar.locator("button").nth(0).click();
      await expect.poll(() => calls.highlights.length).toBe(1);
      await expect(page.locator(".pdf-user-highlight")).toHaveCount(1);

      await selectFixtureText(page, 0, 10);
      toolbar = page.locator(".pdf-selection-toolbar");
      await toolbar.locator("button").nth(1).click();
      await expect(page.getByRole("region", { name: "笔记编辑" })).toBeVisible();
      await page.getByRole("button", { name: "保存笔记", exact: true }).click();
      await expect.poll(() => calls.notes.length).toBe(1);
      expect(calls.notes[0]).toMatchObject({
        selection_context: {
          status: "resolved",
          resolution_basis: resolutionBasis,
        },
      });
      await expect(page.locator(".pdf-note-marker")).toHaveCount(1);

      await page.reload();
      await expect(page.locator(".pdf-text-layer span").first()).toHaveText(/Selectable PDF fixture/);
      await expect(page.locator(".pdf-user-highlight")).toHaveCount(1);
      await expect(page.locator(".pdf-note-marker")).toHaveCount(1);

      await selectFixtureText(page, 0, 10);
      toolbar = page.locator(".pdf-selection-toolbar");
      await toolbar.locator("button").nth(3).click();
      await expect.poll(() => calls.translations.length).toBe(1);
      await expect(page.locator(".pdf-translation-surface")).toContainText("前馈网络");
      await page.locator(".pdf-translation-head button").click();

      await selectFixtureText(page, 0, 10);
      toolbar = page.locator(".pdf-selection-toolbar");
      await toolbar.locator("button").nth(2).click();
      await page.locator(".agent-input textarea").fill(`Explain ${resolutionBasis} selection`);
      await page.locator(".agent-compose-row > button").click();
      await expect.poll(() => calls.agent.length).toBe(1);
      expect(calls.agent[0]).toMatchObject({
        question_quote: {
          status: "resolved",
          resolution_basis: resolutionBasis,
        },
      });
    });
  }
}

test("partial mobile selection hides persistence actions but keeps exact-subrange Ask explicit", async ({ page }, testInfo) => {
  await page.setViewportSize({ width: 390, height: 844 });
  const calls = await installApiFixture(page, "partial");
  await page.goto("/");
  const quality = page.locator(".alignment-quality-bar");
  await expect(quality.getByText("PDF 对齐：部分可用")).toBeVisible();
  await quality.locator("summary").click();
  await expect(quality).toContainText("66.3%");
  await selectFixtureText(page, 0, 10);
  const toolbar = page.locator(".pdf-selection-toolbar");
  await expect(toolbar).toContainText("存在缺字或歧义");
  await expect(toolbar.getByTitle("高亮")).toHaveCount(0);
  await expect(toolbar.getByTitle("笔记")).toHaveCount(0);
  await expect(toolbar.getByTitle("问 AI")).toBeEnabled();
  const toolbarBox = await toolbar.boundingBox();
  const statusBox = await toolbar.locator(".pdf-selection-status").boundingBox();
  expect(toolbarBox?.x).toBeGreaterThanOrEqual(8);
  expect((toolbarBox?.x ?? 0) + (toolbarBox?.width ?? 0)).toBeLessThanOrEqual(382);
  expect(statusBox?.x).toBeGreaterThanOrEqual(8);
  expect((statusBox?.x ?? 0) + (statusBox?.width ?? 0)).toBeLessThanOrEqual(382);
  await page.screenshot({ path: testInfo.outputPath("hf2-reader-partial-mobile.png"), fullPage: true });
  await toolbar.getByTitle("问 AI").click();
  await expect(page.locator(".ask-draft")).toContainText("部分定位");
  expect(calls.highlights).toHaveLength(0);
  expect(calls.notes).toHaveLength(0);
  expect(calls.agent).toHaveLength(0);
});

test("pending and unresolved PDF selection never renders an action toolbar", async ({ page }) => {
  const calls = await installApiFixture(page, "unresolved", 1_000);
  await page.goto("/");
  await page.waitForTimeout(500);
  await selectFixtureText(page, 0, 10);

  await expect.poll(() => calls.resolves).toBe(1);
  await page.waitForTimeout(200);
  expect(await page.locator(".pdf-selection-toolbar").count()).toBe(0);
  expect(await page.evaluate(() => window.getSelection()?.toString())).toBe("Selectable");

  await page.waitForTimeout(900);
  await expect(page.locator(".pdf-selection-toolbar")).toHaveCount(0);
  expect(await page.evaluate(() => window.getSelection()?.toString())).toBe("Selectable");
});

test("unresolved real PDF selection remains native-copy only", async ({ page }) => {
  const calls = await installApiFixture(page, "unresolved");
  await page.goto("/");
  await selectFixtureText(page, 0, 10);
  await expect.poll(() => calls.resolves).toBe(1);
  await expect(page.locator(".pdf-selection-toolbar")).toHaveCount(0);
  expect(await page.evaluate(() => window.getSelection()?.toString())).toBe("Selectable");
  expect(calls.highlights).toHaveLength(0);
  expect(calls.notes).toHaveLength(0);
  expect(calls.agent).toHaveLength(0);
});

test("physical selection can start in the middle of a PDF line", async ({ page }) => {
  const text = "Selectable PDF fixture text for explicit actions.";
  const calls = await installApiFixture(page, "resolved");
  await page.goto("/");
  await dragBetweenTextOffsets(page, text, 15, 27);
  expect(await page.evaluate(() => window.getSelection()?.toString())).toBe("fixture text");
  await expect.poll(() => calls.resolveRequests.length).toBe(1);
  expect(calls.resolveRequests[0]).toMatchObject({ raw_quote: "fixture text" });
});

for (const [boundary, text] of [
  ["line", "Boundary line target."],
  ["paragraph", "Boundary paragraph target."],
] as const) {
  test(`physical trailing whitespace keeps ${boundary} selection exact`, async ({ page }) => {
    const calls = await installApiFixture(page, "resolved", 0, boundaryPdfFixture());
    await page.goto("/");
    await page.evaluate(() => window.getSelection()?.removeAllRanges());
    await dragPastTextEnd(page, text);
    await expect.poll(() => calls.resolveRequests.length).toBe(1);
    expect(await page.evaluate(() => window.getSelection()?.toString())).toBe(text);
    expect(calls.resolveRequests[0]).toMatchObject({ raw_quote: text });
    expect((calls.resolveRequests[0].rects as unknown[])).toHaveLength(1);
  });
}


test('RN3 PDF draft survives collapse, failure and a reduced keyboard viewport, then edits through the annotation', async ({ page }, info) => {
  const calls = await installApiFixture(page, 'resolved');
  await page.setViewportSize({ width: 390, height: 844 }); await page.goto('/');
  await selectFixtureText(page, 0, 10);
  await page.locator('.pdf-selection-toolbar').getByTitle('笔记').click();
  const editor = page.getByRole('region', { name: '笔记编辑' });
  await expect(editor.getByRole('textbox')).toHaveValue('');
  await expect(editor).toContainText('原文摘录');
  await editor.getByRole('textbox').fill('自己的 PDF 想法');
  await editor.getByRole('button', { name: '收起', exact: true }).click();
  await page.getByRole('button', { name: '继续编辑笔记' }).click();
  await expect(editor.getByRole('textbox')).toHaveValue('自己的 PDF 想法');
  await page.setViewportSize({ width: 390, height: 370 });
  await editor.getByRole('textbox').focus();
  await editor.getByRole('textbox').dispatchEvent('keydown', { key: 'Enter', ctrlKey: true, isComposing: true });
  expect(calls.notes).toHaveLength(0);
  const save = editor.getByRole('button', { name: '保存笔记', exact: true });
  const rect = await save.boundingBox(); expect(rect!.y + rect!.height).toBeLessThanOrEqual(370);
  await page.route('**/api/memory/save', route => json(route, { error_code: 'WRITE_FAILED', category: 'internal', message: '暂时写入失败' }, 500), { times: 1 });
  await save.click(); await expect(editor.getByRole('alert')).toContainText('暂时写入失败');
  await expect(editor.getByRole('textbox')).toHaveValue('自己的 PDF 想法');
  await page.screenshot({ path: info.outputPath('rn3-keyboard-viewport.png') });
  await save.click(); await expect(editor).not.toBeVisible();
  expect(calls.notes[0]).toMatchObject({ content: '自己的 PDF 想法', note: { association: { kind: 'selection' }, retained_excerpt: 'Selectable' } });
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.locator('.pdf-note-marker').click();
  const detail = page.getByRole('region', { name: '笔记详情' }).filter({ hasText: '自己的 PDF 想法' });
  await expect(detail).toContainText('原文摘录');
  await page.locator('.pdf-annotation-note-item').getByRole('button', { name: '编辑', exact: true }).click();
  await editor.getByRole('textbox').fill('修订想法'); await save.click();
  await expect.poll(() => calls.replacements.length).toBe(1);
  expect(calls.replacements[0]).toEqual({ mem_id: 'note-1', content: '修订想法' });
  await expect(editor).not.toBeVisible();
  await page.locator('.pdf-note-marker').click();
  await page.locator('.pdf-annotation-note-item').getByRole('button', { name: '在笔记中查看', exact: true }).click();
  const savedCard = page.locator('#reader-panel-notes .memory-card');
  await expect(savedCard).toHaveAttribute('data-mem-id', /edited-/);
  await expect(savedCard.locator('.note-detail')).toContainText('修订想法');
});

test('RN3 Markdown and delivered answer entries share the editor, preserving old mixed notes and material-switch choices', async ({ page }) => {
  const calls = await installApiFixture(page, 'resolved');
  calls.history.current.turns.push({ turn_id: 'answer-1', user: '解释一下', status: 'completed', effect_labels: [], outcome: {
    answer: '__助手粗体__ 与原文不同。', answer_view: { parts: [{ kind: 'markdown', text: '__助手粗体__ 与原文不同。' }], sources: [] }, effects: [], trace: [], memory_updates: [], turns: 1, tokens_spent: 0, profile_usage: { snapshot_revision: 0, injected_fact_ids: [], claimed_used_fact_ids: [], influences: [] },
  } } as never);
  calls.records.push({ mem_id: 'legacy', type: 'note', layer: 'long_term', book_id: 'pdf-selection-actions', anchor: { lid: '1.1' }, content: '> 未知作者的旧摘录\n\n混排正文' });
  await page.setViewportSize({ width: 1440, height: 900 }); await page.goto('/');
  await page.getByRole('button', { name: 'Markdown', exact: true }).click();
  await expect(page.locator('.answer-markdown strong')).toBeVisible();
  const original = page.locator('.prose [data-lid="1.1"]').first();
  await original.evaluate(el => {
    const walker = document.createTreeWalker(el, NodeFilter.SHOW_TEXT); const text = walker.nextNode()!;
    const range = document.createRange(); range.setStart(text, 0); range.setEnd(text, 10);
    const selection = getSelection()!; selection.removeAllRanges(); selection.addRange(range);
    document.dispatchEvent(new Event('selectionchange'));
    el.dispatchEvent(new PointerEvent('pointerup', { bubbles: true }));
  });
  await page.locator('.hl-popover').getByRole('button', { name: '笔记', exact: true }).click();
  const editor = page.getByRole('region', { name: '笔记编辑' });
  await editor.getByRole('textbox').fill('Markdown 想法');
  await editor.getByRole('button', { name: '保存笔记', exact: true }).click();
  await expect.poll(() => calls.notes.length).toBe(1);
  expect((calls.notes[0].note as any).retained_excerpt).toBe((calls.notes[0].selection_context as any).raw_quote);
  const answer = page.locator('.answer-markdown strong');
  await answer.evaluate(el => {
    const range = document.createRange(); range.selectNode(el);
    const selection = getSelection()!; selection.removeAllRanges(); selection.addRange(range);
    el.dispatchEvent(new MouseEvent('mouseup', { bubbles: true }));
  });
  await page.getByRole('button', { name: '记笔记', exact: true }).click();
  await expect(editor).toContainText('助手摘录');
  await expect(editor.getByRole('textbox')).toHaveValue('');
  await editor.getByRole('textbox').fill('我的回答笔记');
  await page.getByRole('button', { name: '打开书', exact: true }).click();
  const picker = page.getByRole('dialog', { name: '书库', exact: true });
  await picker.getByPlaceholder('.understand-book/book-id').fill('another-book');
  await picker.getByRole('button', { name: '打开', exact: true }).click();
  const choice = page.getByRole('dialog', { name: '处理未保存笔记' });
  await choice.getByRole('button', { name: '继续编辑', exact: true }).click();
  expect(calls.opens).toHaveLength(0);
  await picker.getByRole('button', { name: '取消', exact: true }).click();
  await page.getByRole('button', { name: '继续编辑笔记' }).click();
  await editor.getByRole('button', { name: '保存笔记', exact: true }).click();
  await expect.poll(() => calls.notes.length).toBe(2);
  expect(calls.notes[1]).toMatchObject({ content: '我的回答笔记', note: { association: { kind: 'answer', session_id: 'chat-fixture', turn_id: 'answer-1' }, retained_excerpt: '__助手粗体__' } });
  await page.getByRole('button', { name: '阅读工具', exact: true }).click();
  await page.getByRole('tab', { name: '笔记', exact: true }).click();
  const card = page.locator('[data-mem-id="legacy"]').first(); await card.locator('summary').click();
  await expect(card.getByRole('region', { name: '笔记详情' })).toContainText('阅读笔记');
  await card.getByRole('button', { name: '编辑', exact: true }).click();
  await expect(editor.getByRole('textbox')).toHaveValue('> 未知作者的旧摘录\n\n混排正文');
  await editor.getByRole('textbox').fill('旧笔记的修订正文');
  await page.getByRole('button', { name: '打开书', exact: true }).click();
  await picker.getByPlaceholder('.understand-book/book-id').fill('another-book');
  await picker.getByRole('button', { name: '打开', exact: true }).click();
  await choice.getByRole('button', { name: '保存并继续', exact: true }).click();
  await expect.poll(() => calls.opens.length).toBe(1);
  expect(calls.replacements.at(-1)).toEqual({ mem_id: 'legacy', content: '旧笔记的修订正文' });
});


for (const width of [320, 390, 768, 1440]) test(`RN4 retrieves mixed notes and preserves edit/delete/answer continuity at ${width}px`, async ({ page }, info) => {
  const calls = await installApiFixture(page, 'resolved');
  const text = '长文段落与自己的理解。\n\n'.repeat(30) + '$$x^2 + y^2 = z^2$$\n\n```js\nconst result = "完整代码结尾";\n```';
  const base = { type: 'note', layer: 'long_term', book_id: 'pdf-selection-actions', anchor: {} };
  calls.records.push(
    { ...base, mem_id: 'legacy-rn4', anchor: { lid: '1.1' }, content: '> 旧混排正文，不猜作者' },
    { ...base, mem_id: 'answer-rn4', generated_at: '1700000000', content: text,
      note: { material: { book_id: 'pdf-selection-actions', publication_id: null }, association: { kind: 'answer', session_id: 'chat-fixture', turn_id: 'answer-rn4' }, retained_excerpt: { kind: 'assistant', text: '独有的助手摘录' }, source_bindings: [] } },
    { ...base, mem_id: 'highlight-rn4', type: 'highlight', generated_at: '2025-01-01T00:00:00Z', anchor: { lid: '1.1' }, content: '可搜索的高亮原文' },
  );
  calls.history.current.turns.push({ turn_id: 'answer-rn4', user: '原问题', status: 'completed', effect_labels: [], outcome: {
    answer: '原回答全文', answer_view: { parts: [{ kind: 'markdown', text: '原回答全文' }], sources: [] }, effects: [], trace: [], memory_updates: [], turns: 1, tokens_spent: 0,
    profile_usage: { snapshot_revision: 0, injected_fact_ids: [], claimed_used_fact_ids: [], influences: [] },
  } } as never);
  await page.setViewportSize({ width, height: 900 }); await page.goto('/');
  await expect(page.getByRole('button', { name: '适合栏宽', exact: true })).toBeVisible();
  async function openNotes() {
    if (await page.locator('.workspace-mobile-nav').isVisible()) await page.locator('.workspace-mobile-nav').getByRole('button', { name: '笔记', exact: true }).click();
    else {
      await page.getByRole('button', { name: '阅读工具', exact: true }).click();
      await page.getByRole('tab', { name: '笔记', exact: true }).click();
    }
  }
  await openNotes();
  const panel = page.locator('#reader-panel-notes');
  await expect(panel.locator('.memory-card').first()).toHaveAttribute('data-mem-id', 'highlight-rn4');
  await expect(panel.locator('.memory-card[data-mem-id="legacy-rn4"]')).toContainText('保存时间未知');
  await panel.getByRole('searchbox', { name: '搜索笔记' }).fill('独有的助手');
  await expect(panel.locator('.memory-card')).toHaveCount(1);
  const card = panel.locator('.memory-card').first();
  expect(await card.locator('.note-preview').evaluate(el => getComputedStyle(el).overflowY)).toBe('hidden');
  await card.locator('summary').click();
  await expect(card.locator('.katex-display')).toBeVisible();
  await expect(card.locator('pre')).toContainText('完整代码结尾');
  const edit = card.getByRole('button', { name: '编辑', exact: true });
  await edit.scrollIntoViewIfNeeded();
  const rect = await edit.boundingBox(); expect(rect!.x).toBeGreaterThanOrEqual(0); expect(rect!.x + rect!.width).toBeLessThanOrEqual(width);
  await page.screenshot({ path: info.outputPath(`rn4-${width}.png`) });
  await card.getByRole('button', { name: '返回原回答' }).click();
  await expect(page.locator('[data-turn-id="answer-rn4"] .answer-markdown')).toContainText('原回答全文');
  await openNotes(); await edit.click();
  const editor = page.getByRole('region', { name: '笔记编辑' });
  await editor.getByRole('textbox').fill('修改后自己的文字');
  await editor.getByRole('button', { name: '保存笔记', exact: true }).click();
  await expect(editor).not.toBeVisible();
  const updated = panel.locator('.memory-card');
  await expect(updated).toHaveCount(1); await expect(updated.locator('.note-detail')).toContainText('修改后自己的文字');
  await expect(updated).toHaveAttribute('data-mem-id', /edited-/);
  page.once('dialog', d => d.accept()); await updated.getByRole('button', { name: '删除', exact: true }).click();
  await expect(panel).toContainText('没有匹配的记录'); await expect(page.locator('.banner')).toContainText('笔记已删除');
  await panel.getByRole('searchbox', { name: '搜索笔记' }).fill('');
  await panel.getByRole('combobox', { name: '笔记类型' }).selectOption('highlight'); await expect(panel.locator('.memory-card')).toHaveCount(1);
  await panel.getByRole('combobox', { name: '笔记类型' }).selectOption('all');
  await panel.getByRole('combobox', { name: '笔记排序' }).selectOption('original');
  await expect(panel.locator('.memory-card')).toHaveCount(2);
});

for (const width of [390, 1440]) test(`RS2 rich content paginates completely and saves the exact preview at ${width}px`, async ({ page }, info) => {
  test.setTimeout(120_000);
  const calls = await installApiFixture(page, 'resolved');
  await page.setViewportSize({ width, height: 900 }); await page.goto('/');
  await expect(page.getByRole('button', { name: '适合栏宽', exact: true })).toBeVisible();
  const result = await page.evaluate(async () => {
    const { paginateShare, renderShare } = await import('/src/reading-share.ts');
    const long = '中文与 English，保留空格和标点。'.repeat(30);
    const markdown = '# 阅读中的证据\n\n**重点**与 `inline code`。\n\n- 第一项\n- 第二项\n\n> 引用原话\n\n' + long + '\n\n```js\nconst value = 42;\nconsole.log(value);\n```\n\n$$\n\\frac{a^2+b^2}{c}\n$$\n\n|名称|值|\n|---|---|\n' + Array.from({length:14}, (_,i) => `|第${i}项|${i}|`).join('\n');
    const draft = { source: { parts: [{ id: 'body', label: '我的笔记', text: markdown }], sources: ['《学习与理解》', '第一章'], association: '' }, selected: ['body'], title: '完整分享', reflection: '我的补充', layout: 'understanding', palette: 'paper' };
    const layout = await paginateShare(draft as any);
    const contents = layout.pages.map(page => {
      const content = page.querySelector('.content')!.cloneNode(true) as HTMLElement;
      content.querySelectorAll('.identity,.caption,thead').forEach(el => el.remove());
      return content.textContent ?? '';
    });
    const sources = layout.pages.map(page => page.querySelector('footer')!.textContent);
    layout.dispose();
    const blobs = await renderShare(draft as any);
    let firstHeader: Uint8ClampedArray | undefined;
    const files = await Promise.all(blobs.map(async blob => {
      const bitmap = await createImageBitmap(blob); const canvas = document.createElement('canvas'); canvas.width = bitmap.width; canvas.height = bitmap.height;
      const ctx = canvas.getContext('2d')!; ctx.drawImage(bitmap,0,0);
      const pixels = ctx.getImageData(0,0,canvas.width,canvas.height).data;
      let dark = 0; for (let i=0;i<pixels.length;i+=4) if (.2126*pixels[i] + .7152*pixels[i+1] + .0722*pixels[i+2] < 150) dark++;
      // The title now follows the paper; only the fixed brand is shared across page headers.
      const header = ctx.getImageData(88, 54, 904, 64).data;
      const headerMatches = !firstHeader || header.every((value,index) => value === firstHeader![index]); firstHeader ??= header;
      return { headerMatches, width:bitmap.width, height:bitmap.height, dark, bytes:Array.from(new Uint8Array(await blob.arrayBuffer())) };
    }));
    return { long, contents, sources, files };
  });
  expect(result.contents.length).toBeGreaterThan(2);
  const joined = result.contents.join('');
  expect(joined).toContain(result.long); expect(joined).toContain('const value = 42;'); expect(joined).toContain('console.log(value);');
  for (let i=0;i<14;i++) expect(joined).toContain(`第${i}项`);
  expect(joined).toContain('第一项'); expect(joined).toContain('第二项'); expect(joined).toContain('我的补充');
  for (const [index,file] of result.files.entries()) { const {writeFileSync}=await import('node:fs');writeFileSync(info.outputPath(`rs2-page-${index+1}.png`),Buffer.from(file.bytes)); }
  for (const [index,file] of result.files.entries()) {
    expect(file.headerMatches, `page ${index+1}`).toBe(true); expect(file.width).toBe(1080); expect(file.height).toBe(1440); expect(file.dark).toBeGreaterThan(3000);
    expect(result.sources[index]).toContain('《学习与理解》');
  }
  expect(calls.notes).toEqual([]); expect(calls.agent).toEqual([]);
});

test('RS2 PDF exact selection and saved highlight use original text and source, partial selection cannot share', async ({ page }) => {
  const calls = await installApiFixture(page, 'resolved');
  await page.goto('/'); await selectFixtureText(page, 0, 10);
  await page.locator('.pdf-selection-toolbar').getByTitle('生成分享图').click();
  const panel = page.getByRole('dialog', { name: '生成分享图', exact: true });
  await expect(panel.locator('fieldset pre')).toHaveText('Selectable');
  await expect(panel).toContainText('第 1 页');
  await panel.getByRole('button', { name: '关闭分享' }).click();
  await selectFixtureText(page, 0, 10);
  await page.locator('.pdf-selection-toolbar').getByTitle('高亮').click();
  await expect.poll(() => calls.highlights.length).toBe(1);
  const highlightBox = await page.locator('.pdf-user-highlight').first().boundingBox();
  await page.mouse.click(highlightBox!.x + highlightBox!.width / 2, highlightBox!.y + highlightBox!.height / 2);
  await page.locator('.pdf-highlight-actions').getByRole('button', { name: '生成分享图' }).click();
  await expect(panel.locator('fieldset pre')).toHaveText('Selectable');
  await panel.getByRole('button', { name: '关闭分享' }).click();
  expect(calls.notes).toEqual([]); expect(calls.agent).toEqual([]);
  await installApiFixture(page, 'partial'); await page.reload(); await selectFixtureText(page, 0, 10);
  await expect(page.locator('.pdf-selection-toolbar').getByTitle('生成分享图')).toHaveCount(0);
});

test('RS2 Markdown cross-paragraph selection preserves source text and downloads a later page', async ({ page }, info) => {
  test.setTimeout(90_000);
  const calls = await installApiFixture(page, 'resolved');
  const first = '先阅读原文，再形成自己的理解。'; const second = '接着核对证据，保留完整的上下文。';
  await page.route('**/api/book/manifest', route => json(route, { tree:[
    {lid:'1',kind:'chapter',title:'# 阅读方法',children:['1.1','1.2'],span:{start:0,end:100}},
    {lid:'1.1',kind:'paragraph',children:[],span:{start:0,end:first.length}},
    {lid:'1.2',kind:'paragraph',children:[],span:{start:first.length,end:first.length+second.length}},
  ],stats_by_lid:{} }));
  await page.route('**/api/book/text?*', route => json(route, {lid:new URL(route.request().url()).searchParams.get('lid'),text:new URL(route.request().url()).searchParams.get('lid')==='1.1'?first:second}));
  await page.route('**/api/book/paper_metadata', route => json(route, {title:{value:'原书名'}}));
  await page.setViewportSize({width:1440,height:900}); await page.goto('/');
  await page.getByRole('button',{name:'Markdown',exact:true}).click();
  await expect(page.locator('.prose [data-lid="1.2"]').first()).toContainText(second);
  await page.locator('.prose').first().evaluate(el => {
    const a=el.querySelector('[data-lid="1.1"]')!, b=el.querySelector('[data-lid="1.2"]')!;
    const aText=document.createTreeWalker(a,NodeFilter.SHOW_TEXT).nextNode()!;
    const bText=document.createTreeWalker(b,NodeFilter.SHOW_TEXT).nextNode()!;
    const range=document.createRange();range.setStart(aText,0);range.setEnd(bText,bText.textContent!.length);
    getSelection()!.removeAllRanges();getSelection()!.addRange(range);
    document.dispatchEvent(new Event('selectionchange'));b.dispatchEvent(new PointerEvent('pointerup',{bubbles:true}));
  });
  await page.locator('.hl-popover').getByRole('button',{name:'生成分享图'}).click();
  const panel=page.getByRole('dialog',{name:'生成分享图',exact:true});
  await expect(panel.locator('fieldset pre')).toHaveText(first+'\n\n'+second);
  await expect(panel).toContainText('《原书名》'); await expect(panel).toContainText('阅读方法');
  await panel.getByLabel('我的感想',{exact:true}).fill('对照原文，逐步理解。'.repeat(40));
  await panel.getByRole('button',{name:'预览图片',exact:true}).click();
  await expect(panel.getByRole('img')).toBeVisible({timeout:30000});
  await panel.getByRole('button',{name:'下一页'}).click();
  const preview=await panel.getByRole('img').evaluate(async img => Array.from(new Uint8Array(await (await fetch((img as HTMLImageElement).src)).arrayBuffer())));
  const downloading=page.waitForEvent('download'); await panel.getByRole('button',{name:'下载 PNG'}).click();
  const download=await downloading; expect(download.suggestedFilename()).toBe('阅读随记-2.png');
  const path=info.outputPath('rs2-second-page.png');await download.saveAs(path);expect(readFileSync(path).equals(Buffer.from(preview))).toBe(true);
  await panel.getByRole('button',{name:'关闭分享'}).click();
  expect(calls.notes).toEqual([]);expect(calls.agent).toEqual([]);
});

test('RS3 visual cards group the original excerpt, reflection and source in both layouts and palettes', async ({ page }, info) => {
  test.setTimeout(120_000);
  await installApiFixture(page, 'resolved'); await page.goto('/');
  await expect(page.getByRole('button', { name: '适合栏宽', exact: true })).toBeVisible();
  const cards = await page.evaluate(async () => {
    const { paginateShare, renderShare } = await import('/src/reading-share.ts');
    const text = '如果任务进一步要求保存笔记，系统就需要执行一次真实的写入。笔记属于读者，会话记录需要保存本次交互，阅读器可能因为导航而改变位置。这些变化拥有各自的生命周期：视口随操作变化，聊天随追问增长，笔记在聊天结束后仍可使用。';
    const results = [];
    for (const layout of ['excerpt', 'understanding']) for (const palette of ['blue', 'paper']) {
      const draft = { source: { parts: [{ id: 'excerpt', label: '原文摘录', text, format: 'plain' }], sources: ['深入 Understand Book：从知识构建到阅读 Agent 的架构与实现', '一次阅读任务包含哪些工作'], association: '' },
        selected: ['excerpt'], title: '阅读随记', reflection: '好哎', layout, palette } as any;
      const output = await paginateShare(draft);
      const sheet = output.pages[0];
      const body = sheet.querySelector('.share-section')!;
      const reflection = sheet.querySelector('.reflection')!;
      const footer = sheet.querySelector('footer')!;
      const count = output.pages.length;
      const content = body.textContent;
      const sourceGap = footer.getBoundingClientRect().top - reflection.getBoundingClientRect().bottom;
      const reflectionGap = reflection.getBoundingClientRect().top - body.getBoundingClientRect().bottom;
      output.dispose();
      const blobs = await renderShare(draft);
      results.push({ layout, palette, count, content, text, sourceGap, reflectionGap,
        bytes: Array.from(new Uint8Array(await blobs[0].arrayBuffer())) });
    }
    return results;
  });
  for (const card of cards) {
    expect(card.count).toBe(1); expect(card.content).toContain(card.text);
    expect(card.reflectionGap).toBeGreaterThan(0); expect(card.sourceGap).toBeGreaterThan(0); expect(card.sourceGap).toBeLessThan(60);
    const { writeFileSync } = await import('node:fs');
    writeFileSync(info.outputPath(`rs3-${card.layout}-${card.palette}.png`), Buffer.from(card.bytes));
  }
});

for (const width of [320, 390, 768, 1440]) test(`RS3 all pages save and close preserves reading and chat at ${width}px`, async ({ page }, info) => {
  test.setTimeout(120_000);
  page.setDefaultTimeout(10000);
  const calls = await installApiFixture(page, 'resolved');
  const content = '**阅读与理解**\n\n' + '中文与 English，保留空格和标点。'.repeat(35) + '\n\n```js\nconst value = 42;\n```\n\n$$\\frac{a^2+b^2}{c}$$';
  calls.records.push({ mem_id: 'rs3-note', type: 'note', layer: 'long_term', book_id: 'pdf-selection-actions', anchor: {}, content });
  await page.setViewportSize({ width, height: 720 }); await page.goto('/');
  await expect(page.getByRole('button', { name: '适合栏宽', exact: true })).toBeVisible();
  const mobileNav = page.locator('.workspace-mobile-nav');
  if (width < 1024) await mobileNav.getByRole('button', { name: '问答', exact: true }).click();
  const composer = page.getByPlaceholder('从当前阅读位置提问...');
  await composer.fill('尚未发送的阅读问题');
  if (width < 1024) await mobileNav.getByRole('button', { name: '笔记', exact: true }).click();
  else { await page.getByRole('button', { name: '阅读工具', exact: true }).click(); await page.getByRole('tab', { name: '笔记', exact: true }).click(); }
  const card = page.locator('#reader-panel-notes [data-mem-id="rs3-note"]');
  await card.locator('summary').click();
  const open = card.getByRole('button', { name: '生成分享图', exact: true });
  await open.scrollIntoViewIfNeeded();
  const reader = page.locator('.pdf-page-list');
  const before = await reader.evaluate(el => { el.scrollTop = 180; return el.scrollTop; });
  await open.click();
  const panel = page.getByRole('dialog', { name: '生成分享图', exact: true });
  await panel.getByLabel('标题', { exact: true }).fill('连续阅读');
  await panel.getByLabel('我的感想', { exact: true }).fill('保存全部页面后回到原来的问题。');
  // A reduced viewport exercises scroll reachability; it is not a physical keyboard/device claim.
  if (width < 1024) await page.setViewportSize({ width, height: 420 });
  for (const label of ['标题', '我的感想', '排版', '配色']) {
    const control = panel.getByRole(label === '排版' || label === '配色' ? 'combobox' : 'textbox', { name: label, exact: true });
    await control.scrollIntoViewIfNeeded(); await expect(control).toBeInViewport();
  }
  await panel.getByRole('button', { name: '预览图片', exact: true }).click();
  await expect(panel.getByRole('img')).toBeVisible({ timeout: 45000 });
  expect(await panel.evaluate(el => el.scrollWidth <= el.clientWidth + 1)).toBe(true);
  const nav = panel.getByRole('navigation', { name: '分享图分页' });
  const count = Number((await nav.locator('span').innerText()).split('/')[1]);
  expect(count).toBeGreaterThan(1);
  for (let index = 1; index <= count; index++) {
    await expect(nav.locator('span')).toHaveText(`${index} / ${count}`);
    const image = panel.getByRole('img');
    const bytes = await image.evaluate(async img => Array.from(new Uint8Array(await (await fetch((img as HTMLImageElement).src)).arrayBuffer())));
    const downloading = page.waitForEvent('download');
    await panel.getByRole('button', { name: '下载 PNG', exact: true }).click();
    const download = await downloading; expect(download.suggestedFilename()).toBe(`阅读随记-${index}.png`);
    const path = info.outputPath(`rs3-${width}-page-${index}.png`); await download.saveAs(path);
    expect(readFileSync(path).equals(Buffer.from(bytes))).toBe(true);
    if (index < count) await nav.getByRole('button', { name: '下一页' }).click();
  }
  const openImage = panel.getByRole('link', { name: '打开图片', exact: true });
  await openImage.scrollIntoViewIfNeeded(); await expect(openImage).toBeInViewport();
  const popupPromise = page.waitForEvent('popup'); await openImage.click();
  const popup = await popupPromise; await popup.waitForLoadState();
  expect(popup.url()).toBe(await panel.getByRole('img').getAttribute('src')); await popup.close();
  await page.setViewportSize({ width, height: 720 });
  await nav.scrollIntoViewIfNeeded();
  await page.screenshot({ path: info.outputPath(`rs3-${width}.png`) });
  await page.keyboard.press('Escape'); await expect(panel).toHaveCount(0); await expect(open).toBeFocused();
  expect(await reader.evaluate(el => el.scrollTop)).toBe(before);
  if (width < 1024) await mobileNav.getByRole('button', { name: '问答', exact: true }).click();
  else await page.getByRole('button', { name: '阅读工具', exact: true }).click();
  await expect(composer).toHaveValue('尚未发送的阅读问题');
  if (width < 1024) await mobileNav.getByRole('button', { name: '阅读', exact: true }).click();
  await selectFixtureText(page, 0, 10);
  await expect(page.locator('.pdf-selection-toolbar').getByTitle('生成分享图')).toBeVisible();
  expect(calls.records[0].content).toBe(content);
  expect(calls.notes).toEqual([]); expect(calls.replacements).toEqual([]); expect(calls.agent).toEqual([]);
});

test('RS2 overwide tables and formulas fail before PNG export; long ordered lists and code preserve every item', async ({page}) => {
  await installApiFixture(page,'resolved');await page.goto('/');
  await expect(page.getByRole('button',{name:'适合栏宽',exact:true})).toBeVisible();
  const result=await page.evaluate(async () => {
    const {paginateShare}=await import('/src/reading-share.ts');
    const draft=(text:string)=>({source:{parts:[{id:'body',label:'我的笔记',text}],sources:['原材料'],association:''},selected:['body'],title:'完整内容',reflection:'',layout:'understanding',palette:'paper'} as any);
    const failures=[];
    for (const text of ['|列|\n|---|\n|'+'W'.repeat(100)+'|', '$$'+'x+'.repeat(100)+'y$$']) {
      try {const output=await paginateShare(draft(text));output.dispose();failures.push('unexpected success');} catch(error){failures.push(String(error));}
    }
    const list=Array.from({length:30},(_,i)=>`${i+1}. 项目${i+1} 原始文字`).join('\n');
    const code=Array.from({length:45},(_,i)=>`const item${i} = ${i};`).join('\n');
    const output=await paginateShare(draft(list+'\n\n```js\n'+code+'\n```'));
    const items=output.pages.flatMap(page=>[...page.querySelectorAll('ol')].map(list=>({start:list.getAttribute('start')||'1',text:list.textContent})));
    const codeText=output.pages.flatMap(page=>[...page.querySelectorAll('pre')].map(block=>block.textContent)).join('');
    const headers=output.pages.map(page=>({title:page.querySelector('.caption h1')!.textContent,top:page.querySelector('.content')!.getBoundingClientRect().top-page.querySelector('header')!.getBoundingClientRect().bottom}));
    output.dispose(); return {failures,items,code,codeText,headers};
  });
  result.failures.forEach(message=>expect(message).toMatch(/过宽|无法完整放入/));
  expect(result.items.map(item=>Number(item.start))).toEqual(Array.from({length:30},(_,i)=>i+1));
  expect(result.codeText).toBe(result.code+'\n');
  result.headers.forEach(header=>{expect(header.title).toContain('完整内容');expect(header.top).toBeGreaterThanOrEqual(20);});
});

for (const width of [320, 390, 768, 1440]) test(`RS6 delivered answer selection and identical PNG at ${width}px`, async ({ page }, info) => {
  test.setTimeout(90000);
  const calls = await installApiFixture(page, 'resolved');
  const text = '仅当样本独立时，这个结论才成立。\n\n先检查假设，再使用结论。';
  const outcome = { answer: text, incomplete: false, effects: [], trace: [], memory_updates: [],
    profile_usage: { snapshot_revision: 0, injected_fact_ids: [], claimed_used_fact_ids: [], influences: [] },
    answer_view: { parts: [{ kind: 'markdown', text }, { kind: 'sources', source_ref_ids: ['rs6-a', 'rs6-b'] }],
      sources: [{ source_ref_id: 'rs6-a', label: '正文 · 独立性' }, { source_ref_id: 'rs6-b', label: '另一本材料 · 第 9 页' }] } };
  (calls.history.current.turns as any[]).push({ turn_id: 'rs6-old-turn', user_turn_ordinal: 1, user: '什么时候能使用这个结论？', status: 'completed', outcome, effect_labels: [] });
  (calls.history.current.turns as any[]).push({ turn_id: 'rs6-incomplete', user_turn_ordinal: 2, user: '不应导出的未完成回合', status: 'completed', outcome: { ...outcome, incomplete: true, answer_view: null, answer: '未完成回答' }, effect_labels: [] });
  const sourceRequests: any[] = [];
  await page.route('**/api/agent/source.resolve', route => {
    const input = route.request().postDataJSON(); sourceRequests.push(input);
    return json(route, { source_ref_id: input.source_ref_id,
      material_title: input.source_ref_id === 'rs6-a' ? '概率与推断' : null,
      label: input.source_ref_id === 'rs6-a' ? '正文 · 独立性' : '另一本材料 · 第 9 页',
      stale: input.source_ref_id === 'rs6-b', can_open_in_reader: false, highlighted_quote: '', context_before: '', context_after: '' });
  });
  await page.addInitScript(() => {
    (window as any).rs6Painted = [];
    const draw = CanvasRenderingContext2D.prototype.drawImage;
    CanvasRenderingContext2D.prototype.drawImage = function(...args: any[]) {
      const image = args[0];
      if (this.canvas.width === 1080 && image instanceof HTMLImageElement && image.src.startsWith('data:image/svg+xml')) {
        const xml = new TextDecoder().decode(Uint8Array.from(atob(image.src.split(',')[1]), ch => ch.charCodeAt(0)));
        const doc = new DOMParser().parseFromString(xml, 'image/svg+xml');
        doc.querySelectorAll('style').forEach(node => node.remove());
        (window as any).rs6Painted.push(doc.documentElement.textContent);
      }
      return (draw as any).apply(this, args);
    };
  });
  await page.setViewportSize({ width, height: 900 }); await page.goto('/');
  await expect(page.getByRole('button', { name: '适合栏宽', exact: true })).toBeVisible();
  if (width < 1024) await page.locator('.workspace-mobile-nav').getByRole('button', { name: '问答', exact: true }).click();
  const composer = page.getByRole('textbox', { name: '围绕当前阅读内容提问', exact: true });
  await composer.fill('未发送的问题保留');
  const turn = page.locator('[data-turn-id="rs6-old-turn"]');
  await expect(page.locator('[data-turn-id="rs6-incomplete"] .answer-actions')).toHaveCount(0);
  await turn.locator('.answer-actions summary').click();
  const trigger = turn.getByRole('button', { name: '生成理解卡', exact: true });
  await trigger.click();
  const panel = page.getByRole('dialog', { name: '生成分享图', exact: true });
  await expect(panel.getByLabel('排版')).toHaveValue('understanding');
  await expect(panel.getByRole('button', { name: '预览图片' })).toBeEnabled();
  expect(sourceRequests.map(request => request.turn_id)).toEqual(['rs6-old-turn', 'rs6-old-turn']);
  await panel.getByLabel('标题', { exact: true }).fill('条件与结论');
  await panel.getByLabel('我的感想', { exact: true }).fill('先核对独立性。');
  await panel.getByRole('button', { name: '调整回答范围' }).click();
  await panel.getByLabel('原回答文字').evaluate((field: HTMLTextAreaElement) => { field.focus(); field.setSelectionRange(0, field.value.indexOf('\n')); });
  await panel.getByRole('button', { name: '使用选中文字' }).click();
  await panel.getByRole('button', { name: '预览图片' }).click();
  await expect(panel.getByRole('img')).toBeVisible();
  const painted = await page.evaluate(() => (window as any).rs6Painted.join(''));
  expect(painted).toContain('助手解释'); expect(painted).toContain('仅当样本独立时，这个结论才成立。');
  expect(painted).toContain('我的感想'); expect(painted).toContain('《概率与推断》');
  expect(painted).toContain('另一本材料 · 第 9 页（来源暂不可用）'); expect(painted).toContain('回答节选');
  expect(painted).not.toContain('先检查假设'); expect(painted).not.toContain('未完成回答'); expect(painted).not.toContain('rs6-old-turn');
  const bytes = await panel.getByRole('img').evaluate(async img => Array.from(new Uint8Array(await (await fetch((img as HTMLImageElement).src)).arrayBuffer())));
  const downloadEvent = page.waitForEvent('download'); await panel.getByRole('button', { name: '下载 PNG' }).click();
  const downloaded = await downloadEvent;
  const path = info.outputPath(`answer-${width}.png`); await downloaded.saveAs(path);
  expect(readFileSync(path).equals(Buffer.from(bytes))).toBe(true);
  expect(await panel.evaluate(node => node.scrollWidth <= node.clientWidth + 1)).toBe(true);
  await page.screenshot({ path: info.outputPath(`answer-panel-${width}.png`) });
  await panel.getByRole('button', { name: '调整回答范围' }).click();
  await panel.getByRole('button', { name: '包含全部回答文字' }).click();
  await expect(panel.getByRole('img')).toHaveCount(0);
  await expect(panel.getByLabel('我的感想', { exact: true })).toHaveValue('先核对独立性。');
  await panel.getByRole('button', { name: '关闭分享' }).click();
  await expect(composer).toHaveValue('未发送的问题保留');
  await expect(trigger).toBeFocused();
  expect(calls.notes).toHaveLength(0); expect(calls.agent).toHaveLength(0); expect(calls.opens).toHaveLength(0);
  // Native answer selection opens the same sharing flow with only that excerpt.
  await turn.locator('.answer-markdown p').first().evaluate(node => {
    const range = document.createRange(); range.selectNodeContents(node);
    const selection = window.getSelection()!; selection.removeAllRanges(); selection.addRange(range);
    node.dispatchEvent(new MouseEvent('mouseup', { bubbles: true }));
  });
  await page.locator('.answer-popover').getByRole('button', { name: '生成理解卡' }).click();
  await expect(panel).toBeVisible();
  await panel.locator('.share-original summary').click();
  await expect(panel.locator('.share-original pre')).toHaveText('仅当样本独立时，这个结论才成立。');
});

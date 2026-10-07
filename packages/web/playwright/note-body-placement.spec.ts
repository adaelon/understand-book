import { expect, test, type Page, type Route } from "@playwright/test";

function pdfFixture(): Buffer {
  const content = [
    "BT /F1 18 Tf",
    "72 700 Td (First placement target for an Agent note.) Tj",
    "0 -80 Td (Second placement target for moving the note.) Tj",
    "ET",
  ].join("\n");
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
  body += `xref\n0 ${objects.length + 1}\n0000000000 65535 f \n`;
  body += offsets.slice(1).map((offset) => `${String(offset).padStart(10, "0")} 00000 n \n`).join("");
  body += `trailer\n<< /Size ${objects.length + 1} /Root 1 0 R >>\nstartxref\n${xrefOffset}\n%%EOF\n`;
  return Buffer.from(body, "ascii");
}

const sourceFingerprint = "a".repeat(64);
const sourceMap = {
  version: "pdf_source_map.v1" as const,
  book_id: "paper-a",
  coordinate_system: {
    space: "pdf_user_space",
    origin: "bottom_left",
    unit: "pt",
    rotation_applied: false,
  },
  pages: [{ pageIndex: 0, page_label: "1", width: 612, height: 792, rotate: 0, view: [0, 0, 612, 792] }],
  entries: [
    {
      lid: "1.1",
      source_span: { start: 0, end: 42 },
      status: "word_mapped",
      regions: [{ region_id: "first", pageIndex: 0, bbox: [70, 690, 390, 722] }],
      alignment: { confidence: 1 },
    },
    {
      lid: "1.2",
      source_span: { start: 42, end: 88 },
      status: "word_mapped",
      regions: [{ region_id: "second", pageIndex: 0, bbox: [70, 610, 410, 642] }],
      alignment: { confidence: 1 },
    },
  ],
  excluded_regions: [],
  page_region_index: {},
  page_excluded_index: {},
  config_hash: "cfg-v1",
};

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
    bottom_lid: "1.2",
    width: 2,
    visible_lids: ["1.1", "1.2"],
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

function json(route: Route, value: unknown, status = 200) {
  return route.fulfill({ status, contentType: "application/json", body: JSON.stringify(value) });
}

function profileMemory() {
  return {
    current_book_id: "paper-a",
    status: {
      document_revision: 1,
      projection_revision: 1,
      profile_status: "current",
      pending_sensitive_confirmation: false,
      pending_review_jobs: 0,
      review_error: null,
    },
    snapshot: {
      source_revision: 1,
      profile_status: "current",
      global_core: [],
      applicable_global: [],
      book_state_core: [],
      profile_projection: [],
      pending_context: [],
    },
    facts: [],
    pending_candidates: [],
    evidence: [],
    collection_rules: [],
  };
}

type FixtureSurface = "markdown" | "pdf";

async function installFixture(page: Page, surface: FixtureSurface = "pdf") {
  const records: Array<Record<string, any>> = [{
    mem_id: "legacy-old",
    type: "note",
    layer: "long_term",
    book_id: "paper-a",
    anchor: { lid: "1.1", concept: null },
    content: "legacy note",
    note_placement: null,
  }];
  const calls = {
    saves: [] as Array<Record<string, any>>,
    reanchors: [] as Array<Record<string, any>>,
    failNextReanchor: false,
    uncertainNextReanchor: false,
  };
  let moveSequence = 0;
  const outcome = {
    answer: "Agent answer excerpt without a quote source.",
    incomplete: false,
    warning: null,
    turns: 1,
    tokens_spent: 4,
    effects: [],
    trace: [],
    profile_usage: {
      snapshot_revision: 0,
      injected_fact_ids: [],
      claimed_used_fact_ids: [],
      influences: [],
    },
    memory_updates: [],
  };
  const history = {
    active_session_id: "chat-a",
    sessions: [{
      id: "chat-a",
      title: "Fixture",
      created_at: "2026-07-28T00:00:00Z",
      updated_at: "2026-07-28T00:00:00Z",
      turn_count: 1,
      turns: [{ user: "Question", question_source_label: null, question_quote: null }],
    }],
    current: {
      id: "chat-a",
      book_id: "paper-a",
      title: "Fixture",
      created_at: "2026-07-28T00:00:00Z",
      updated_at: "2026-07-28T00:00:00Z",
      turns: [{
        turn_id: "turn-a",
        user_turn_ordinal: 1,
        user: "Question",
        status: "completed",
        outcome,
        error: null,
        question_source_label: null,
        question_quote: null,
        effect_labels: [],
      }],
    },
  };

  await page.route("**/api/**", async (route) => {
    const request = route.request();
    const path = new URL(request.url()).pathname;
    const body = request.postData() ? request.postDataJSON() as Record<string, any> : null;
    if (path === "/api/book/pdf/original") {
      return route.fulfill({ status: 200, contentType: "application/pdf", body: pdfFixture() });
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
        book_id: "paper-a",
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
        tree: [
          { lid: "1.1", children: [], span: { start: 0, end: 42 }, kind: "paragraph" },
          { lid: "1.2", children: [], span: { start: 42, end: 88 }, kind: "paragraph" },
        ],
        stats_by_lid: {},
      });
    }
    if (path === "/api/book/asset_manifest") {
      return json(route, { version: "asset_manifest.v1", book_id: "paper-a", images: [] });
    }
    if (path === "/api/book/source_fingerprint") {
      return json(route, { book_id: "paper-a", source_fingerprint: sourceFingerprint });
    }
    if (path === "/api/book/source_manifest") {
      return json(route, {
        version: "source_manifest.v2",
        book_id: "paper-a",
        canonical_source: {
          kind: "reconciled_markdown",
          path: "source.txt",
          citation_anchor: "lid",
          sha256: sourceFingerprint,
        },
        ...(surface === "pdf"
          ? { original_pdf: { path: "fixture.pdf", sha256: "pdf", citation_anchor: false } }
          : {}),
        capabilities: surface === "pdf"
          ? {
              view_pdf: { status: "available" },
              project_lid_to_pdf: { status: "available" },
              resolve_pdf_selection: { status: "available" },
              project_ranges_to_pdf: { status: "available" },
            }
          : {
              view_pdf: { status: "unavailable" },
              project_lid_to_pdf: { status: "unavailable" },
              resolve_pdf_selection: { status: "unavailable" },
              project_ranges_to_pdf: { status: "unavailable" },
            },
        alignment_quality: null,
      });
    }
    if (path === "/api/book/pdf_source_map") return json(route, sourceMap);
    if (path === "/api/profile/manifest") return json(route, { ...profile, projections: [], guided_reading_policy: {}, defaults: {} });
    if (path === "/api/profile/memory") return json(route, profileMemory());
    if (path === "/api/profile/backfill") return json(route, { sessions: [], jobs: [] });
    if (path === "/api/book/text") {
      const lid = new URL(request.url()).searchParams.get("lid") ?? "1.1";
      return json(route, {
        lid,
        text: lid === "1.2" ? "Second placement target for moving the note." : "First placement target for an Agent note.",
      });
    }
    if (path === '/api/reader/goto') return json(route, { ok: true, viewport: { ...readerState.viewport, top_lid: body?.lid, anchor_lid: body?.lid } });
    if (path === '/api/memory/replace') {
      const index = records.findIndex(r => r.mem_id === body?.mem_id);
      const record = { ...records[index], mem_id: body?.mem_id + '-edited', content: body?.content };
      records.splice(index, 1, record); return json(route, record);
    }
    if (path === '/api/memory/delete') {
      const index = records.findIndex(r => r.mem_id === body?.mem_id);
      if (index >= 0) records.splice(index, 1); return json(route, { ok: true });
    }
    if (path === "/api/reader/state") return json(route, readerState);
    if (path === "/api/agent/history") return json(route, history);
    if (path === "/api/build_intent/artifacts") return json(route, { overlay: null });
    if (path === "/api/build_intent/usage.event") return json(route, { accepted: true });
    if (path === "/api/memory/recall") return json(route, records);
    if (path === "/api/reader/pdf_ranges.project") return json(route, { projections: [] });
    if (path === "/api/memory/save") {
      calls.saves.push(body ?? {});
      const placement = body?.note_placement;
      const existing = records.find((record) =>
        record.type === "note"
        && record.content === body?.content
        && JSON.stringify(record.note_placement) === JSON.stringify(placement));
      if (existing) return json(route, { status: "EXISTING", record: existing });
      const record = {
        mem_id: "note-created",
        type: "note",
        layer: "long_term",
        book_id: "paper-a",
        anchor: { lid: placement.lid, concept: null },
        content: body?.content,
        note_placement: placement,
      };
      records.push(record);
      return json(route, { status: "CREATED", record });
    }
    if (path === "/api/memory/reanchor") {
      calls.reanchors.push(body ?? {});
      const index = records.findIndex((record) => record.mem_id === body?.mem_id);
      if (index < 0) {
        return json(route, { error_code: "STALE", category: "conflict", message: "record changed" }, 409);
      }
      if (calls.failNextReanchor) {
        calls.failNextReanchor = false;
        return json(route, {
          error_code: "NOTE_REANCHOR_CONFLICT",
          category: "conflict",
          message: "fixture conflict",
        }, 409);
      }
      const old = records[index];
      const moved = {
        ...old,
        mem_id: old.mem_id.startsWith("legacy") ? "legacy-moved" : `note-moved-${++moveSequence}`,
        anchor: { lid: body!.note_placement.lid, concept: null },
        note_placement: body!.note_placement,
      };
      records.splice(index, 1, moved);
      if (calls.uncertainNextReanchor) {
        calls.uncertainNextReanchor = false;
        return route.abort("connectionfailed");
      }
      return json(route, moved);
    }
    if (path === "/api/memory/promote") {
      const record = records.find((candidate) => candidate.mem_id === body?.mem_id);
      return record ? json(route, { ...record, layer: "long_term" }) : json(route, {}, 404);
    }
    return json(route, {
      error_code: "UNMOCKED",
      category: "internal",
      message: `Unmocked fixture route: ${path}`,
    }, 500);
  });
  return { calls, records, history };
}

async function selectAgentExcerpt(page: Page) {
  const answer = page.locator(".answer-markdown").first();
  await expect(answer).toContainText("Agent answer excerpt");
  if (!(await answer.isVisible())) await page.locator('.workspace-mobile-nav').getByRole('button', { name: '问答', exact: true }).click();
  await answer.scrollIntoViewIfNeeded();
  await answer.evaluate((element) => {
    const range = document.createRange();
    range.selectNodeContents(element);
    const selection = window.getSelection()!;
    selection.removeAllRanges();
    selection.addRange(range);
    element.closest(".ans-text")!.dispatchEvent(new MouseEvent("mouseup", { bubbles: true }));
  });
  await page.locator(".answer-popover button").click();
}

async function clickPdfRegion(page: Page, bbox: [number, number, number, number]) {
  const shell = page.locator(".pdf-page-shell").first();
  await shell.scrollIntoViewIfNeeded();
  let box = await shell.boundingBox();
  if (!box) throw new Error("PDF page shell has no box");
  const centerX = (bbox[0] + bbox[2]) / 2;
  const centerY = (bbox[1] + bbox[3]) / 2;
  let x = box.x + (centerX / 612) * box.width;
  let y = box.y + ((792 - centerY) / 792) * box.height;
  const pointHitsPage = await page.evaluate(({ x, y }) => {
    const shell = document.querySelector(".pdf-page-shell");
    const target = document.elementFromPoint(x, y);
    return !!shell && !!target && shell.contains(target);
  }, { x, y });
  if (!pointHitsPage) {
    await page.evaluate((targetY) => window.scrollBy(0, targetY - 180), y);
    box = await shell.boundingBox();
    if (!box) throw new Error("PDF page shell left the viewport");
    x = box.x + (centerX / 612) * box.width;
    y = box.y + ((792 - centerY) / 792) * box.height;
  }
  await page.mouse.move(box.x + 2, box.y + 2);
  await page.mouse.move(x, y);
  await expect(page.locator(".pdf-note-placement-candidate")).toBeVisible();
  await page.mouse.click(x, y);
}

async function openNotes(page: Page) {
  const mobile = page.locator('.workspace-mobile-nav').getByRole('button', { name: '笔记', exact: true });
  if (await mobile.isVisible()) await mobile.click();
  else await page.locator('button.tab').filter({ hasText: '笔记' }).click();
}

async function startNotePlacementFromList(page: Page, memId: string) {
  const note = page.locator(`details[data-mem-id="${memId}"]`);
  const action = note.locator("[data-note-placement-action]");
  if (!(await action.isVisible())) await note.evaluate((element) => element.setAttribute("open", ""));
  await expect(action).toBeVisible();
  await action.click();
  await expect(page.locator(".note-placement-status")).toBeVisible();
}

async function clickMarkdownLid(page: Page, lid: string) {
  const target = page.locator(`.reader-pane [data-lid="${lid}"]`).first();
  await expect(target).toBeVisible();
  await target.hover();
  await expect(target).toHaveClass(/note-placement-candidate/);
  await target.click();
}

const viewports = [
  { name: "desktop", width: 1440, height: 900 },
  { name: "mobile", width: 390, height: 844 },
] as const;

for (const viewport of viewports) {
  test(`PDF create, reanchor, legacy and recovery persist on ${viewport.name}`, async ({ page }) => {
    await page.setViewportSize({ width: viewport.width, height: viewport.height });
    const fixture = await installFixture(page);
    await page.goto("/");
    const firstPdfPage = page.locator('.pdf-page-shell[data-page-index="0"]');
    await expect(firstPdfPage).toBeVisible();
    await expect(firstPdfPage.locator(".pdf-text-layer")).toContainText(
      "First placement target",
      { timeout: 15_000 },
    );

    await selectAgentExcerpt(page);
    await expect(page.locator(".banner")).toContainText("PDF 正文区域");
    await expect(page.locator(".note-placement-status")).toContainText("PDF 正文区域");
    await clickPdfRegion(page, [70, 690, 390, 722]);
    await expect.poll(() => fixture.calls.saves.length).toBe(1);
    expect(fixture.calls.saves[0]).toMatchObject({
      type: "note",
      layer: "long_term",
      note_placement: {
        kind: "pdf_region",
        source_fingerprint: sourceFingerprint,
        lid: "1.1",
        source_map_version: "pdf_source_map.v1",
        source_map_config_hash: "cfg-v1",
        page_index: 0,
        region_id: "first",
      },
    });
    expect(fixture.calls.saves[0]).not.toHaveProperty("anchor_lid");
    await expect(page.locator(".note-placement-status")).toHaveCount(0);
    await expect(page.locator(".pdf-note-marker")).toHaveCount(1);

    await openNotes(page);
    await startNotePlacementFromList(page, "note-created");
    if (viewport.name === "desktop") fixture.calls.failNextReanchor = true;
    else fixture.calls.uncertainNextReanchor = true;
    await clickPdfRegion(page, [70, 610, 410, 642]);
    await expect.poll(() => fixture.calls.reanchors.length).toBe(1);

    if (viewport.name === "desktop") {
      expect(fixture.records.some((record) => record.mem_id === "note-created")).toBe(true);
      await expect(page.locator(".note-placement-status")).toBeVisible();
      await clickPdfRegion(page, [70, 610, 410, 642]);
      await expect.poll(() => fixture.calls.reanchors.length).toBe(2);
    }
    await expect(page.locator(".note-placement-status")).toHaveCount(0);
    await expect.poll(() => fixture.records.some((record) =>
      record.note_placement?.region_id === "second" && record.content.includes("Agent answer excerpt"))).toBe(true);

    await page.reload();
    await expect(page.locator(".pdf-note-marker")).toHaveCount(1);
    await openNotes(page);
    await startNotePlacementFromList(page, "legacy-old");
    await clickPdfRegion(page, [70, 690, 390, 722]);
    await expect.poll(() => fixture.records.some((record) => record.mem_id === "legacy-moved")).toBe(true);
    await expect(page.locator(".pdf-note-marker")).toHaveCount(2);

    await page.reload();
    await expect(page.locator(".pdf-note-marker")).toHaveCount(2);
    expect(fixture.records.every((record) => record.note_placement?.kind === "pdf_region")).toBe(true);
  });
}

for (const viewport of viewports) {
  test(`Markdown create, reanchor, legacy and recovery persist on ${viewport.name}`, async ({ page }) => {
    await page.setViewportSize({ width: viewport.width, height: viewport.height });
    const fixture = await installFixture(page, "markdown");
    await page.goto("/");
    const firstBlock = page.locator(`.reader-pane [data-lid="1.1"]`).first();
    await expect(firstBlock).toContainText("First placement target");

    await selectAgentExcerpt(page);
    await expect(page.locator(".banner")).toContainText("Note");
    await expect(page.locator(".note-placement-status")).toContainText("Note");
    expect(fixture.calls.saves).toHaveLength(0);
    await clickMarkdownLid(page, "1.1");
    await expect.poll(() => fixture.calls.saves.length).toBe(1);
    expect(fixture.calls.saves[0]).toMatchObject({
      type: "note",
      layer: "long_term",
      note_placement: {
        kind: "lid_block",
        source_fingerprint: sourceFingerprint,
        lid: "1.1",
      },
    });
    expect(fixture.calls.saves[0]).not.toHaveProperty("anchor_lid");
    await expect(page.locator(".note-placement-status")).toHaveCount(0);
    await expect(page.locator(".reader-pane .annotation-marker")).toHaveCount(1);

    await openNotes(page);
    await startNotePlacementFromList(page, "note-created");
    if (viewport.name === "desktop") fixture.calls.failNextReanchor = true;
    else fixture.calls.uncertainNextReanchor = true;
    await clickMarkdownLid(page, "1.2");
    await expect.poll(() => fixture.calls.reanchors.length).toBe(1);

    if (viewport.name === "desktop") {
      expect(fixture.records.some((record) => record.mem_id === "note-created")).toBe(true);
      await expect(page.locator(".note-placement-status")).toBeVisible();
      await clickMarkdownLid(page, "1.2");
      await expect.poll(() => fixture.calls.reanchors.length).toBe(2);
    }
    await expect(page.locator(".note-placement-status")).toHaveCount(0);
    await expect.poll(() => fixture.records.some((record) =>
      record.note_placement?.lid === "1.2" && record.content.includes("Agent answer excerpt"))).toBe(true);

    await page.reload();
    await expect(page.locator(".reader-pane .annotation-marker")).toHaveCount(1);
    await openNotes(page);
    await startNotePlacementFromList(page, "legacy-old");
    await clickMarkdownLid(page, "1.1");
    await expect.poll(() => fixture.records.some((record) => record.mem_id === "legacy-moved")).toBe(true);
    await expect(page.locator(".reader-pane .annotation-marker")).toHaveCount(2);

    await page.reload();
    await expect(page.locator(".reader-pane .annotation-marker")).toHaveCount(2);
    expect(fixture.records.every((record) => record.note_placement?.kind === "lid_block")).toBe(true);
  });
}

function addReaderAnnotations(fixture: Awaited<ReturnType<typeof installFixture>>) {
  fixture.records.push({
    mem_id: 're-note', type: 'note', layer: 'long_term', book_id: 'paper-a',
    anchor: { lid: '1.1' }, content: '对这段证据的批注。',
    note_placement: { kind: 'lid_block', source_fingerprint: sourceFingerprint, lid: '1.1' },
  }, {
    mem_id: 're-cross', type: 'note', layer: 'long_term', book_id: 'paper-a',
    anchor: { lid: '1.1' }, content: '> Agent note. Second placement target\n\n跨段引用的解释。',
    selection_context: { status: 'resolved', raw_quote: 'Agent note. Second placement target', resolved_quote: 'Agent note. Second placement target',
      ranges: [{ lid: '1.1', range: { start: 30, end: 41 } }, { lid: '1.2', range: { start: 0, end: 23 } }] },
  }, {
    mem_id: 're-highlight', type: 'highlight', layer: 'long_term', book_id: 'paper-a',
    anchor: { lid: '1.1' }, content: 'First placement', range: { start: 0, end: 15 },
  });
}

for (const viewport of viewports) {
  test(`RE3 RE4 annotations, edit identity, geometry and source return on ${viewport.name}`, async ({ page }) => {
    await page.setViewportSize({ width: viewport.width, height: viewport.height });
    await page.emulateMedia({ reducedMotion: viewport.name === 'mobile' ? 'reduce' : 'no-preference' });
    const fixture = await installFixture(page, 'markdown');
    addReaderAnnotations(fixture);
    await page.goto('/');
    const marker = page.locator('[data-annotation-lid="1.1"]');
    await expect(marker).toContainText('3');
    await page.evaluate(() => document.fonts.ready);
    const body = page.locator('.prose [data-lid="1.2"]');
    const before = await body.boundingBox();
    await page.screenshot({ path: `../../docs/performance/reader-re3-re4/${viewport.name}-closed.png` });
    await marker.focus(); await page.keyboard.press('Enter');
    const preview = page.getByRole('dialog', { name: '正文批注', exact: true });
    await expect(preview).toBeVisible();
    await expect(preview.locator('header')).toContainText('3 条');
    expect(await body.boundingBox()).toEqual(before);
    if (viewport.name === 'mobile') await expect(preview).toHaveCSS('animation-name', 'none');
    await preview.getByRole('button', { name: '笔记 2', exact: true }).click();
    await expect(preview.locator('.annotation-source')).toContainText('Agent note. Second placement target');
    await preview.getByRole('button', { name: '高亮 3', exact: true }).click();
    await expect(preview.getByRole('button', { name: '高亮 3', exact: true })).toHaveAttribute('aria-pressed', 'true');
    await expect(preview.locator('.annotation-content')).toContainText('First placement');
    await page.screenshot({ path: `../../docs/performance/reader-re3-re4/${viewport.name}-open.png` });
    await page.keyboard.press('Escape');
    await expect(preview).toHaveCount(0); await expect(marker).toBeFocused();
    await marker.click(); await page.keyboard.press('Escape'); await marker.click();
    await expect(preview).toHaveCount(1);
    const selected = await page.locator('.prose').evaluate(el => {
      const range = document.createRange(); range.selectNodeContents(el);
      getSelection()!.removeAllRanges(); getSelection()!.addRange(range);
      return getSelection()!.toString();
    });
    expect(selected).toContain('First placement'); expect(selected).toContain('Second placement');
    expect(selected).not.toMatch(/批注|笔记|编辑|对这段证据/);
    await page.evaluate(() => getSelection()!.removeAllRanges());
    await preview.getByRole('button', { name: '编辑', exact: true }).click();
    await expect(preview).toHaveCount(0);
    const editor = page.locator('.note-modal textarea');
    await editor.fill('编辑后仍是同一条批注');
    await page.locator('.reader-pane').dispatchEvent('scroll');
    await expect(editor).toHaveValue('编辑后仍是同一条批注');
    await page.locator('.note-modal').getByRole('button', { name: '保存', exact: true }).click();
    await expect(page.locator('.note-modal')).toHaveCount(0);
    expect(fixture.records.some(r => r.mem_id === 're-note')).toBe(false);
    expect(fixture.records.some(r => r.mem_id === 're-note-edited')).toBe(true);
    await marker.click();
    await expect(preview).toContainText('编辑后仍是同一条批注');
    await preview.getByRole('button', { name: '查看来源', exact: true }).click();
    await expect(page.locator('.source-preview-dialog')).toBeVisible();
    await page.getByRole('button', { name: '在阅读区打开', exact: true }).click();
    await expect(page.locator('[data-reader-arrival]')).toHaveCount(1);
    await page.getByRole('button', { name: '返回阅读位置', exact: true }).click();
    expect(await body.boundingBox()).toEqual(before);
    await marker.click();
    await preview.getByRole('button', { name: '在笔记中查看', exact: true }).click();
    const card = page.locator('details[data-mem-id="re-note-edited"]');
    await expect(card).toHaveAttribute('open', ''); await expect(card).toBeFocused();
    await card.locator('.note-source-button').click();
    await page.getByRole('button', { name: '在阅读区打开', exact: true }).click();
    await page.getByRole('button', { name: '返回笔记', exact: true }).click();
    await expect(card).toBeVisible(); await expect(card).toBeFocused();
    page.once('dialog', dialog => dialog.accept());
    await card.getByRole('button', { name: '删除', exact: true }).click();
    await expect(card).toHaveCount(0);
    expect(fixture.records.some(r => r.mem_id === 're-note-edited')).toBe(false);
    if (viewport.name === 'mobile') await page.locator('.workspace-mobile-nav').getByRole('button', { name: '阅读', exact: true }).click();
    await expect(marker).toContainText('2');
  });
}


test('RE4 closing a loading source rejects its late content', async ({ page }) => {
  const fixture = await installFixture(page, 'markdown'); addReaderAnnotations(fixture);
  await page.goto('/');
  await page.locator('[data-annotation-lid="1.1"]').click();
  let release!: () => void; let requests = 0;
  const held = new Promise<void>(resolve => { release = resolve; });
  await page.route('**/api/book/text?*', async route => { requests++; await held; await route.fallback(); });
  await page.getByRole('dialog', { name: '正文批注', exact: true }).getByRole('button', { name: '查看来源', exact: true }).click();
  await expect.poll(() => requests).toBeGreaterThan(0);
  await page.getByRole('button', { name: '关闭来源预览', exact: true }).click();
  release();
  await page.waitForResponse(r => r.url().includes('/api/book/text?'));
  await expect(page.locator('.source-preview-modal')).toHaveCount(0);
});


test('RE4 a narrow Notes source return restores the hidden long-paragraph position', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  const fixture = await installFixture(page, 'markdown'); addReaderAnnotations(fixture);
  await page.route('**/api/book/text?lid=1.1', route => json(route, { lid: '1.1', text: 'Long paragraph preserves the reading position through Notes and source navigation. '.repeat(70) }));
  await page.goto('/');
  await expect(page.locator('.prose')).toContainText('Long paragraph');
  await page.evaluate(() => document.fonts.ready);
  const reader = page.locator('.reader-pane');
  await reader.evaluate(el => { el.scrollTop = 900; });
  await openNotes(page);
  const card = page.locator('details[data-mem-id="re-note"]');
  await card.locator('.note-source-button').click();
  await page.getByRole('button', { name: '在阅读区打开', exact: true }).click();
  await page.getByRole('button', { name: '返回笔记', exact: true }).click();
  await expect(card).toBeVisible();
  await page.locator('.workspace-mobile-nav').getByRole('button', { name: '阅读', exact: true }).click();
  await expect.poll(() => reader.evaluate(el => el.scrollTop)).toBeCloseTo(900, 0);
});

for (const width of [1440, 1024, 390, 320]) {
  test(`RE5 focus preserves the source character, draft and layout authority at ${width}`, async ({ page }) => {
    await page.setViewportSize({ width, height: 900 });
    const fixture = await installFixture(page, 'markdown');
    fixture.history.current.turns[0].outcome.answer = 'A long answer keeps its message position while reading.\n\n'.repeat(100);
    await page.route('**/api/book/manifest', route => json(route, { tree: [
      { lid: '1', kind: 'chapter', title: '第一章 · 阅读与理解：在同一材料中保持位置和思考', children: ['1.1', '1.2'], span: { start: 0, end: 88 } },
      { lid: '1.1', kind: 'paragraph', children: [], span: { start: 0, end: 42 } },
      { lid: '1.2', kind: 'paragraph', children: [], span: { start: 42, end: 88 } },
    ], stats_by_lid: {} }));
    await page.route('**/api/book/text?lid=1.1', route => json(route, { lid: '1.1', text: 'Reading keeps the same source character through focus and return. '.repeat(130) }));
    const layoutWrites: string[] = [];
    page.on('request', request => { if (request.method() === 'POST' && /layout|bootstrap|attach/.test(request.url())) layoutWrites.push(request.url()); });
    await page.goto('/');
    const reader = page.locator('.reader-pane');
    await expect(reader).toContainText('Reading keeps');
    await page.evaluate(() => document.fonts.ready);
    await reader.evaluate(el => { el.scrollTop = 800; });
    // Capture a real DOM character through the exposed Reader contract, with its source offset.
    const anchor = await reader.evaluate((el: any) => el.__vueParentComponent.exposed.captureScrollAnchor(['1.1']));
    await reader.evaluate(el => el.setAttribute('data-instance', 'stable-reader'));
    const rail = page.locator('.right-rail');
    await rail.evaluate(el => el.setAttribute('data-instance', 'stable-rail'));
    if (width < 1024) await page.locator('.workspace-mobile-nav').getByRole('button', { name: '问答', exact: true }).click();
    await page.locator('.agent-input textarea').fill('Keep this question draft');
    const transcript = page.locator('.transcript');
    await transcript.evaluate(el => { el.scrollTop = 180; });
    if (width < 1024) await page.locator('.workspace-mobile-nav').getByRole('button', { name: '阅读', exact: true }).click();
    const beforeWrites = layoutWrites.length;
    await page.getByRole('button', { name: '专注阅读', exact: true }).filter({ visible: true }).click();
    await expect(page.locator('.workspace-shell')).toHaveAttribute('data-foreground', 'reader');
    await expect(rail).toBeHidden();
    const characterOffset = () => reader.evaluate((el, position) => {
      const paragraph = el.querySelector('[data-lid="1.1"]')!;
      const walker = document.createTreeWalker(paragraph, NodeFilter.SHOW_TEXT);
      let remaining = position.start, node: Node | null;
      while ((node = walker.nextNode())) {
        if (remaining < node.textContent!.length) {
          const range = document.createRange(); range.setStart(node, remaining); range.setEnd(node, remaining + 1);
          return Math.abs(range.getBoundingClientRect().top - el.getBoundingClientRect().top - position.top);
        }
        remaining -= node.textContent!.length;
      }
      return Infinity;
    }, anchor.textPosition);
    await expect.poll(characterOffset).toBeLessThanOrEqual(36);
    await page.screenshot({ path: `../../docs/performance/reader-re5-re6/focus-${width}.png` });
    await page.locator('.workspace-mobile-top').getByRole('button', { name: '目录', exact: true }).click();
    await expect(page.locator('#reader-outline')).toBeVisible();
    await expect(page.locator('.outline-item[aria-current="location"]')).toContainText('阅读与理解');
    if (width === 390) {
      await page.locator('#reader-outline').evaluate(el => Promise.all(el.getAnimations().map(animation => animation.finished)));
      await page.screenshot({ path: '../../docs/performance/reader-re5-re6/outline-mobile.png' });
    }
    await page.keyboard.press('Escape');
    await expect(page.locator('#reader-outline')).toBeHidden();
    await page.getByRole('button', { name: '退出专注', exact: true }).filter({ visible: true }).click();
    await expect.poll(characterOffset).toBeLessThanOrEqual(36);
    if (width < 1024) await page.locator('.workspace-mobile-nav').getByRole('button', { name: '问答', exact: true }).click();
    await expect(page.locator('.agent-input textarea')).toHaveValue('Keep this question draft');
    expect(await transcript.evaluate(el => el.scrollTop)).toBe(180);
    await expect(page.locator('[data-instance="stable-reader"]')).toHaveCount(1);
    await expect(page.locator('[data-instance="stable-rail"]')).toHaveCount(1);
    expect(layoutWrites.length).toBe(beforeWrites);
    await page.screenshot({ path: `../../docs/performance/reader-re5-re6/workspace-${width}.png` });
  });
}

for (const width of [1440, 390]) {
  test(`RE6 surface return preserves PDF page position and scale at ${width}`, async ({ page }) => {
    await page.setViewportSize({ width, height: 700 });
    await installFixture(page);
    await page.goto('/');
    const pdf = page.locator('.pdf-reader-pane');
    await expect(page.locator('.pdf-text-layer span').first()).toHaveText(/First placement/);
    await page.getByLabel('PDF 缩放倍率（相对适宽）').selectOption('2');
    await expect(page.locator('.pdf-page-list')).toHaveAttribute('aria-busy', 'false');
    await page.locator('.pdf-page-list').evaluate(el => { el.scrollTop = 200; });
    const before = await pdf.evaluate((el: any) => el.__vueParentComponent.exposed.captureReadingAnchor());
    await page.getByRole('button', { name: 'Markdown', exact: true }).click();
    await expect(page.locator('.reader-pane')).toBeVisible();
    await page.getByRole('button', { name: 'PDF', exact: true }).click();
    await expect(page.locator('.pdf-page-list')).toHaveAttribute('aria-busy', 'false');
    await expect(page.getByLabel('PDF 缩放倍率（相对适宽）')).toHaveValue('2');
    await expect.poll(async () => {
      const after = await pdf.evaluate((el: any) => el.__vueParentComponent.exposed.captureReadingAnchor());
      return after ? Math.abs(after.pageRatio - before.pageRatio) : 1;
    }).toBeLessThan(0.003);
    await page.screenshot({ path: `../../docs/performance/reader-re5-re6/pdf-return-${width}.png` });
  });
}

import { expect, type Locator, type Page, type TestInfo } from '@playwright/test';
import { writeFile } from 'node:fs/promises';

/** Check the drawn SVG against the rendered node cards, in either orientation. */
export async function inspectGraphArrows(graph: Locator) {
  return graph.evaluate(section => {
    const nodes = new Map([...section.querySelectorAll<HTMLElement>('[data-node-id]')]
      .map(node => [node.dataset.nodeId!, node.getBoundingClientRect()] as const));
    const borderDistance = (point: DOMPoint, box: DOMRect) => {
      const inside = point.x >= box.left && point.x <= box.right && point.y >= box.top && point.y <= box.bottom;
      if (inside) return Math.min(point.x - box.left, box.right - point.x, point.y - box.top, box.bottom - point.y);
      return Math.hypot(Math.max(box.left - point.x, 0, point.x - box.right),
        Math.max(box.top - point.y, 0, point.y - box.bottom));
    };
    return [...section.querySelectorAll<SVGGElement>('g[data-edge-id]')].map(group => {
      const id = group.dataset.edgeId!;
      const path = group.querySelector<SVGPathElement>('path[marker-end]')!;
      const markerId = path.getAttribute('marker-end')!.match(/^url\(#(.+)\)$/)![1];
      const tip = document.getElementById(markerId)!.querySelector('path')!;
      const label = section.querySelector<HTMLElement>(`[data-edge-label="${CSS.escape(id)}"]`)!.getBoundingClientRect();
      const labelX = label.left + label.width / 2, labelY = label.top + label.height / 2;
      const from = nodes.get(group.dataset.from!), to = nodes.get(group.dataset.to!);
      const length = path.getTotalLength();
      const pointAt = (distance: number) => path.getPointAtLength(distance).matrixTransform(path.getScreenCTM()!);
      const crosses = new Set<string>();
      let hit: { x: number; y: number } | null = null;
      let labelDistance = Infinity;
      for (let distance = 3; distance < length - 2; distance += 3) {
        const point = pointAt(distance);
        labelDistance = Math.min(labelDistance, Math.hypot(point.x - labelX, point.y - labelY));
        for (const [nodeId, box] of nodes) {
          if (point.x > box.left + 3 && point.x < box.right - 3
            && point.y > box.top + 3 && point.y < box.bottom - 3) crosses.add(nodeId);
        }
        if (!hit && document.elementFromPoint(point.x, point.y)?.closest('g[data-edge-id]') === group) {
          hit = { x: point.x, y: point.y };
        }
      }
      return { id, length, crosses: [...crosses], labelDistance, hit,
        active: group.dataset.active === 'true', stroke: getComputedStyle(path).stroke, tipFill: getComputedStyle(tip).fill,
        startError: from && length ? borderDistance(pointAt(0), from) : null,
        endError: to && length ? borderDistance(pointAt(length), to) : null };
    });
  });
}

export async function checkGraphArrows(graph: Locator) {
  await expect(async () => {
    const arrows = await inspectGraphArrows(graph);
    for (const arrow of arrows) {
      expect(arrow.stroke, `Focus color of ${arrow.id}`).toBe(arrow.active ? 'rgb(182, 90, 23)' : 'rgb(146, 138, 127)');
      expect(arrow.tipFill, `Arrowhead color of ${arrow.id}`).toBe(arrow.stroke);
      expect(arrow.length, arrow.id).toBeGreaterThan(8);
      expect(arrow.startError, arrow.id).toBeLessThan(5);
      expect(arrow.endError, arrow.id).toBeLessThan(5);
      expect(arrow.labelDistance, `Relation label misses arrow ${arrow.id}`).toBeLessThan(4);
      expect(arrow.crosses, `Arrow ${arrow.id} runs through a node`).toEqual([]);
    }
  }).toPass({ timeout: 3000 });
}

/** Capture one delivered comparison throughout, including the default 384px reader rail. */
export async function checkComparisonLayouts(page: Page, card: Locator, info: TestInfo, locale: 'zh' | 'en',
  focus = card.locator('.teach-back-select[data-status="divergent"]').first()) {
  const fullScreen = page.getByRole('button', { name: locale === 'zh' ? '问答全屏' : 'Full-screen Q&A', exact: true });
  const exit = page.getByRole('button', { name: locale === 'zh' ? '退出问答全屏' : 'Exit full-screen Q&A', exact: true });
  for (const mode of ['wide', 'rail', '390', '320'] as const) {
    const width = mode === '390' || mode === '320' ? Number(mode) : 1440;
    await page.setViewportSize({ width, height: width < 600 ? 844 : 1000 });
    if (width >= 600) await expect(fullScreen.or(exit)).toBeVisible();
    if (mode === 'wide' && await fullScreen.isVisible()) await fullScreen.click();
    if (mode === 'rail' && await exit.isVisible()) await exit.click();
    if (width < 600) {
      const openChat = page.locator('.workspace-mobile-nav:visible').getByRole('button', { name: locale === 'zh' ? '问答' : 'Q&A', exact: true });
      const chatHeader = page.locator('.right-rail .mobile-back:visible');
      await expect(openChat.or(chatHeader).first()).toBeVisible();
      if (await openChat.isVisible()) await openChat.click();
      await expect(chatHeader).toBeVisible();
    }
    await focus.click();
    await expect(focus.locator('.focus-topic')).toBeVisible();
    await expect(focus.locator('.focus-topic')).toHaveText(/\S/);
    await expect(card.locator('.focus-heading')).toContainText(await focus.locator('.focus-topic').innerText());
    const graphs = card.locator('.teach-back-graph');
    await checkGraphArrows(graphs.first());
    await checkGraphArrows(graphs.last());
    const dimensions = await graphs.evaluateAll(elements => elements.map(graph => {
      const { x, y, width, height } = graph.getBoundingClientRect();
      const nodes = [...graph.querySelectorAll<HTMLElement>('[data-node-id]')];
      const edges = [...graph.querySelectorAll<SVGElement>('g[data-edge-id]')];
      const uniqueNodes = new Set(nodes.map(node => node.dataset.nodeId)).size === nodes.length;
      const visibleArrows = edges.every(edge => {
        const path = edge.querySelector<SVGPathElement>('path[marker-end]');
        const marker = path?.getAttribute('marker-end')?.match(/#([^)]*)/);
        return path && path.getTotalLength() > 8 && marker && document.getElementById(marker[1]);
      });
      return { side: graph.getAttribute('data-side'), x, y, width, height, uniqueNodes, visibleArrows,
        edges: edges.length, nodes: nodes.length, fits: graph.scrollWidth <= graph.clientWidth + 1
          && nodes.every(node => node.scrollWidth <= node.clientWidth + 1 && node.scrollHeight <= node.clientHeight + 1) };
    }));
    expect(dimensions.map(graph => graph.side)).toEqual(['book', 'student']);
    expect(dimensions.every(g => g.fits && g.uniqueNodes && g.visibleArrows && g.nodes > 0)).toBe(true);
    expect(dimensions[0].edges).toBeGreaterThan(0);
    const rail = (await page.locator('.right-rail').boundingBox())!;
    const cardBox = (await card.boundingBox())!;
    const [book, student] = dimensions;
    if (mode === 'wide') {
      expect(cardBox.width).toBeGreaterThanOrEqual(560);
      expect(book.y).toBeCloseTo(student.y, 0);
      expect(student.x).toBeGreaterThanOrEqual(book.x + book.width - 1);
    } else {
      if (mode === 'rail') expect(rail.width).toBe(384);
      expect(cardBox.width).toBeLessThan(560);
      expect(book.x).toBeCloseTo(student.x, 0);
      expect(student.y).toBeGreaterThanOrEqual(book.y + book.height - 1);
    }
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    if (mode === '320' && locale === 'en') {
      const title = await page.locator('.mobile-learning-title > span').evaluate(el => {
        const range = document.createRange();
        range.selectNodeContents(el);
        return range.getBoundingClientRect().toJSON();
      });
      const records = (await page.locator('.mobile-records').boundingBox())!;
      expect(title.right <= records.x + 1 || title.bottom <= records.y + 1).toBe(true);
    }
    await writeFile(info.outputPath(`${locale}-${mode}-layout.json`),
      JSON.stringify({ viewport: width, rail: rail.width, card: cardBox.width, dimensions }, null, 2));
    await card.evaluate(el => el.scrollIntoView({ block: 'start' }));
    await page.screenshot({ path: info.outputPath(`${locale}-${mode}-comparison.png`) });
    if (mode !== 'wide') {
      await graphs.last().evaluate(el => el.scrollIntoView({ block: 'start' }));
      await page.screenshot({ path: info.outputPath(`${locale}-${mode}-student.png`) });
    }
    const evidence = card.locator('.teach-back-evidence');
    await evidence.evaluate(el => el.scrollIntoView({ block: 'start' }));
    await expect(evidence.locator('blockquote').first()).toBeInViewport();
    await page.screenshot({ path: info.outputPath(`${locale}-${mode}-evidence.png`) });
  }
}

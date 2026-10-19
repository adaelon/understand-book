import { expect, test, type FrameLocator, type Page } from '@playwright/test';
import fs from 'node:fs';
import path from 'node:path';

const asset = path.resolve('../../skills/presentation/methods/discrete-update/v1/example/index.html');
const output = process.env.SE1_OUTPUT;
const host = process.env.SE1_READER_HOST ?? 'http://127.0.0.1:4185';
test.skip(!output, 'SE1_OUTPUT names this run evidence directory');
test.setTimeout(180_000);

type Scene = { eta: number; semantic_state: number; transition_progress: number; playing: boolean };
type Surface = Page | FrameLocator;
async function seek(surface: Surface, k: number, p: number) {
  return surface.locator('body').evaluate((_, { k, p }) =>
    (window as any).presentationScene.seek({ semantic_state: k, transition_progress: p }), { k, p });
}
async function snapshot(surface: Surface): Promise<Scene> {
  return surface.locator('body').evaluate(() => (window as any).presentationScene.snapshot());
}
async function observed(surface: Surface) {
  return surface.locator('body').evaluate(() => {
    const number = (id: string) => Number(document.getElementById(id)!.textContent);
    const attr = (id: string, name: string) => Number(document.getElementById(id)!.getAttribute(name));
    const trail = document.getElementById('trail') as unknown as SVGPathElement;
    const end = trail.getPointAtLength(trail.getTotalLength());
    return {
      state: (window as any).presentationScene.snapshot() as Scene,
      current: number('current-value'), gradient: number('gradient-value'), delta: number('delta-value'),
      next: number('next-value'), visual: number('visual-value'), loss: number('loss-value'),
      moving: [attr('moving-point', 'cx'), attr('moving-point', 'cy')],
      currentPoint: [attr('current-point', 'x') + 7, attr('current-point', 'y') + 7],
      nextPoint: [attr('next-point', 'cx'), attr('next-point', 'cy')],
      arrow: [attr('update-line', 'x1'), attr('update-line', 'x2')],
      arrowHidden: getComputedStyle(document.getElementById('update-arrow')!).display === 'none',
      nextHidden: getComputedStyle(document.getElementById('next-point')!).display === 'none',
      trailEnd: [end.x, end.y], history: document.querySelectorAll('#history circle').length,
    };
  });
}

// Independent closed form; the page computes a recurrence. Coordinates follow the documented SVG scale.
function oracle(eta: number, k: number, p: number) {
  const at = (n: number) => 2 + 2 * (1 - 2 * eta) ** n;
  const values = Array.from({ length: 9 }, (_, n) => at(n));
  const current = at(k), next = at(Math.min(8, k + 1));
  const visual = (1 - p) * current + p * next;
  const low = Math.min(0, ...values), high = Math.max(4, ...values), margin = (high - low) / 10;
  const xmin = low - margin, xmax = high + margin, ymax = Math.max((xmin - 2) ** 2, (xmax - 2) ** 2);
  const xy = (x: number) => [30 + (x - xmin) / (xmax - xmin) * 548, 216 - (x - 2) ** 2 / ymax * 196];
  return { current, next, visual, gradient: 2 * (current - 2), delta: next - current,
    loss: (visual - 2) ** 2, currentPoint: xy(current), nextPoint: xy(next), moving: xy(visual) };
}
function geometryMatches(actual: number[], expected: number[]) {
  return actual.every((value, index) => Math.abs(value - expected[index]) < 0.001);
}
function verify(actual: Awaited<ReturnType<typeof observed>>, eta: number, k: number, p: number) {
  const expected = oracle(eta, k, p);
  expect(actual.state.eta).toBe(eta); expect(actual.state.semantic_state).toBe(k);
  expect(actual.state.transition_progress).toBeCloseTo(p, 12); expect(actual.state.playing).toBe(false);
  for (const key of ['current', 'gradient', 'visual', 'loss'] as const) expect(actual[key]).toBeCloseTo(expected[key], 5);
  if (k < 8) for (const key of ['delta', 'next'] as const) expect(actual[key]).toBeCloseTo(expected[key], 5);
  for (const key of ['moving', 'currentPoint', 'nextPoint'] as const)
    expect(geometryMatches(actual[key], expected[key]), `${key}: η=${eta}, k=${k}, p=${p}`).toBe(true);
  expect(geometryMatches(actual.trailEnd, expected.moving)).toBe(true);
  expect(geometryMatches(actual.arrow, [expected.currentPoint[0], expected.nextPoint[0]])).toBe(true);
  expect(actual.history).toBe(k + 1);
  expect(actual.arrowHidden).toBe(expected.delta === 0); expect(actual.nextHidden).toBe(k === 8);
}
function save(name: string, value: unknown) {
  fs.mkdirSync(output!, { recursive: true });
  fs.writeFileSync(path.join(output!, name), JSON.stringify(value, null, 2));
}

test('SE1 math, actual geometry and frozen-geometry negative control', async ({ page }) => {
  const html = fs.readFileSync(asset, 'utf8');
  await page.setContent(html);
  const results = [];
  for (const eta of [0, .25, .49, .5, .51, .75, .99, 1, 1.01, 1.2]) {
    await page.locator('#eta').fill(String(eta));
    for (let k = 0; k <= 8; k++) for (const p of k === 8 ? [0] : [0, .35, .999]) {
      await seek(page, k, p);
      const actual = await observed(page); verify(actual, eta, k, p);
      results.push({ eta, k, p, actual });
    }
  }
  await page.locator('[data-eta="0.25"]').click();
  await seek(page, 7, .999); await seek(page, 2, .35);
  const once = await observed(page); await seek(page, 2, .35);
  expect(await observed(page)).toEqual(once);
  verify(once, .25, 2, .35);
  const bad = html.replace("$('moving-point').setAttribute('cx',X(visual)); $('moving-point').setAttribute('cy',Y(visual));",
    "$('moving-point').setAttribute('cx',X(4)); $('moving-point').setAttribute('cy',Y(4));");
  expect(bad).not.toBe(html);
  await page.setContent(bad); await seek(page, 2, .35);
  const frozen = await observed(page), expected = oracle(.25, 2, .35);
  expect(frozen.state.semantic_state).toBe(2); expect(frozen.visual).toBe(expected.visual);
  expect(geometryMatches(frozen.moving, expected.moving)).toBe(false);
  await page.screenshot({ path: path.join(output!, 'frozen-geometry.png'), fullPage: true });
  save('math.json', { status: 'passed', cases: results, repeated_backward: 'passed',
    negative_control: { status: 'detected', labels_correct: true, frozen, expected } });
});

test('SE1 Reader touch/mouse, playback, exact save and storage reopen', async ({ browser }) => {
  const content = JSON.parse(fs.readFileSync(path.join(output!, 'content.json'), 'utf8'));
  expect(content.content_files['index.html']).toBe(fs.readFileSync(asset, 'utf8'));
  const results = [];
  for (const viewport of [{ width: 320, height: 420 }, { width: 640, height: 240 }, { width: 960, height: 720 }]) {
    const touch = viewport.width !== 960;
    const context = await browser.newContext({ viewport, hasTouch: touch });
    const page = await context.newPage(), errors: string[] = [];
    page.on('pageerror', error => errors.push(error.message));
    await page.route('**/api/**', async route => {
      const response = await route.fetch({ url: route.request().url().replace(/^.*\/api/, host) });
      await route.fulfill({ response });
    });
    await page.request.post(`${host}/reset-scene`);
    await page.goto('/agent-presentation-visual.html');
    const frame = page.frameLocator('.agent-presentation iframe');
    await expect(frame.locator('#current-value')).toHaveText('4');
    await page.getByRole('button', { name: '展开', exact: true }).click();
    const press = async (selector: string) => {
      if (touch) await frame.locator(selector).tap(); else await frame.locator(selector).click();
    };
    await press('#next'); verify(await observed(frame), .25, 1, 0);
    await press('#back'); verify(await observed(frame), .25, 0, 0);
    await press('#play');
    await expect.poll(async () => (await snapshot(frame)).transition_progress).toBeGreaterThan(.05);
    expect((await snapshot(frame)).playing).toBe(true);
    await press('#play');
    const paused = await snapshot(frame);
    expect(paused.playing).toBe(false);
    await page.waitForTimeout(120);
    expect(await snapshot(frame)).toEqual(paused);
    await press('#play');
    await press('[data-eta="0.75"]'); verify(await observed(frame), .75, 0, 0);
    await seek(frame, 7, .95); await press('#play');
    await expect.poll(async () => (await snapshot(frame)).semantic_state).toBe(8);
    verify(await observed(frame), .75, 8, 0);
    await press('#reset');
    await frame.locator('#position').focus(); await frame.locator('#position').press('End');
    verify(await observed(frame), .75, 8, 0);
    await press('#reset'); verify(await observed(frame), .75, 0, 0);
    // Real pointer drag on the range, including movement before release.
    await frame.locator('#position').scrollIntoViewIfNeeded();
    const box = (await frame.locator('#position').boundingBox())!;
    const from = { x: box.x + 8, y: box.y + box.height / 2 }, to = { x: box.x + box.width * .65, y: from.y };
    if (touch) {
      const cdp = await context.newCDPSession(page);
      await cdp.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [from] });
      await cdp.send('Input.dispatchTouchEvent', { type: 'touchMove', touchPoints: [to] });
      expect((await snapshot(frame)).semantic_state).toBeGreaterThan(0);
      await cdp.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
      await cdp.detach();
    } else {
      await page.mouse.move(from.x, from.y); await page.mouse.down(); await page.mouse.move(to.x, to.y, { steps: 6 });
      expect((await snapshot(frame)).semantic_state).toBeGreaterThan(0); await page.mouse.up();
    }
    const dragged = await snapshot(frame); verify(await observed(frame), .75, dragged.semantic_state, dragged.transition_progress);
    const layout = await frame.locator('body').evaluate(() => ({
      overflow: document.documentElement.scrollWidth > document.documentElement.clientWidth + 1,
      smallTargets: [...document.querySelectorAll('button,input')].filter(node => {
        const b = node.getBoundingClientRect(); return b.width < 44 || b.height < 44;
      }).map(node => node.id),
    }));
    expect(layout).toEqual({ overflow: false, smallTargets: [] });
    const fixture = await page.request.get(`${host}/fixture`).then(r => r.json());
    const read = () => page.request.post(`${host}/agent/presentation.read`, { data: fixture }).then(r => r.json());
    const restores = [];
    for (const [k, p] of [[2, .35], [7, .999], [8, 0]]) {
      await seek(frame, k, p);
      await frame.locator('body').evaluate(() => (window as any).presentation.commitState());
      await expect.poll(async () => (await read()).restored_state?.values?.page?.semantic_state).toBe(k);
      await expect.poll(async () => (await read()).restored_state?.values?.page?.transition_progress).toBeCloseTo(p, 12);
      const before = await observed(frame), saved = (await read()).restored_state;
      await page.request.post(`${host}/reopen`); await page.reload();
      await expect(frame.locator('#eta-value')).toHaveText('0.75');
      await expect(page.getByText('已恢复上次保存的演示状态')).toBeVisible();
      verify(await observed(frame), .75, k, p); expect(await observed(frame)).toEqual(before);
      restores.push({ k, p, saved, after: await observed(frame) });
    }
    await page.getByRole('button', { name: '展开', exact: true }).click();
    await seek(frame, 2, .35);
    for (const [name, selector] of [['graph', '#moving-point'], ['graph-bottom', '#minimum-label'], ['readout', '#current-value'], ['equation', '#equation'], ['controls', '#play'], ['parameters', '#eta']] as const) {
      await frame.locator(selector).scrollIntoViewIfNeeded();
      await page.screenshot({ path: path.join(output!, `reader-${viewport.width}-${name}.png`) });
    }
    expect(errors).toEqual([]);
    results.push({ viewport, touch, paused, dragged, layout, restores, errors });
    await page.unrouteAll({ behavior: 'ignoreErrors' }); await context.close();
  }
  save('reader.json', { status: 'passed', results });
});

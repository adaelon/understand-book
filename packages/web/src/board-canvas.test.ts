// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { mount } from '@vue/test-utils';
import BoardCanvas from './components/BoardCanvas.vue';
import type { BoardGraphic } from './generated/BoardGraphic';
import type { BoardItem } from './generated/BoardItem';
import type { BoardLink } from './generated/BoardLink';
import type { BoardView } from './generated/BoardView';
import { focusBoardBounds, graphicTextLines, linkGeometry, targetBounds } from './board-canvas-geometry';

const view: BoardView = { x: 20, y: 30, zoom: 1, selected_item_id: null, selected_block_id: null, reading_item_id: null, playback: null, presentation_states: {} };
function drawing(id: string, graphic: BoardGraphic, x = 0, y = 0): BoardItem {
  return { item_id: id, title: id, readable_content: '保留完整解释', explanation_blocks: [], position: { x, y }, size: { width: 240, height: 100 }, created_at: '2026-10-25', source_bindings: [], target: { kind: 'drawing', graphic, based_on: [], session_id: 'session', turn_id: 'turn' } };
}
const makeItems = () => [
  drawing('text', { kind: 'text', text: 'ABCD' }),
  drawing('shape', { kind: 'shape', shape: 'rectangle', text: '从证据更新判断', color: '#b86628' }, 330),
  drawing('path', { kind: 'path', points: [{ x: 4, y: 60 }, { x: 35, y: 20 }, { x: 220, y: 80 }], color: '#788baa', width: 3 }, 0, 180),
  drawing('ellipse', { kind: 'shape', shape: 'ellipse', text: '先验', color: '#769780' }, 330, 180),
];
const link: BoardLink = { link_id: 'cause', from: { kind: 'item', item_id: 'text' }, to: { kind: 'item', item_id: 'shape' }, label: '更新', based_on: [] };
const wrappers: ReturnType<typeof mount>[] = [];
function setup(props: Partial<InstanceType<typeof BoardCanvas>['$props']> = {}, viewport = { width: 800, height: 600 }) {
  const wrapper = mount(BoardCanvas, { props: { items: makeItems(), view: { ...view }, links: [link], ...props } }); wrappers.push(wrapper);
  Object.assign(wrapper.element, { setPointerCapture: vi.fn(), hasPointerCapture: () => true, releasePointerCapture: vi.fn() });
  Object.defineProperties(wrapper.element, { clientWidth: { value: viewport.width }, clientHeight: { value: viewport.height } });
  return wrapper;
}
afterEach(() => { wrappers.splice(0).forEach(wrapper => wrapper.unmount()); });

describe('native board graphics in the shared canvas', () => {
  it('keeps fit-to-content separate from fullscreen and leaves preview canvases without a fullscreen action', async () => {
    const wrapper = setup();
    expect(wrapper.find('[aria-label="全屏画板"]').exists()).toBe(false);
    await wrapper.setProps({ fullscreen: false });
    expect(wrapper.get('[aria-label="全屏画板"]').text()).toBe('全屏画板');
    await wrapper.get('[aria-label="全屏画板"]').trigger('click');
    expect(wrapper.emitted('toggle-fullscreen')).toEqual([[]]);
    expect(wrapper.emitted('view')).toBeUndefined();
    await wrapper.get('[aria-label="显示全部内容"]').trigger('click');
    expect(wrapper.emitted('view')).toHaveLength(1);
    expect(wrapper.emitted('toggle-fullscreen')).toHaveLength(1);
    await wrapper.setProps({ fullscreen: true });
    expect(wrapper.get('[aria-label="退出全屏"]').attributes('aria-pressed')).toBe('true');
  });
  it('connects reply cards without turning drawing provenance into a fan of automatic arrows', async () => {
    const answer = (id: string, y: number): BoardItem => ({ ...drawing(id, { kind: 'text', text: id }, 0, y),
      target: { kind: 'answer', session_id: 'session', turn_id: id, question: '没懂', answer_parts: [] } });
    const original = answer('original-answer', 0);
    const reply = { ...answer('reply-answer', 180), reply_to_item_id: original.item_id };
    const drawings = Array.from({ length: 10 }, (_, index) => ({
      ...drawing(`example-${index}`, { kind: 'shape', shape: 'rectangle', text: String(index), color: '#769780' }, 330 + index % 5 * 260, Math.floor(index / 5) * 180),
      reply_to_item_id: original.item_id,
    }));
    const explicit = { ...link, from: { kind: 'item' as const, item_id: drawings[0].item_id }, to: { kind: 'item' as const, item_id: drawings[1].item_id } };
    const items = [original, reply, ...drawings];
    const retained = JSON.stringify(items);
    const wrapper = setup({ items, links: [explicit] });
    expect(wrapper.findAll('[data-board-reply-link]').map(edge => edge.attributes('data-board-reply-link'))).toEqual([reply.item_id]);
    expect(wrapper.findAll('[data-board-link]')).toHaveLength(1);
    expect(wrapper.findAll('[data-board-graphic]')).toHaveLength(10);
    expect(JSON.stringify(items)).toBe(retained);

    // A follow-up to one primitive still retains that origin, but a visual
    // relation must be drawn explicitly rather than inferred from provenance.
    await wrapper.setProps({ items: [original, { ...reply, reply_to_item_id: drawings[0].item_id }, ...drawings] });
    expect(wrapper.findAll('[data-board-reply-link]')).toHaveLength(0);
    expect(wrapper.findAll('[data-board-link]')).toHaveLength(1);
  });
  it('renders native SVG text, both shapes and the retained local path rather than generic content cards', () => {
    const wrapper = setup();
    expect(wrapper.findAll('.board-card')).toHaveLength(0);
    expect(wrapper.findAll('[data-board-graphic]')).toHaveLength(4);
    expect(wrapper.get('[data-item-id="text"] [data-board-drawing-text]').text()).toBe('ABCD');
    expect(wrapper.find('[data-item-id="shape"] [data-board-shape="rectangle"]').exists()).toBe(true);
    expect(wrapper.find('[data-item-id="ellipse"] [data-board-shape="ellipse"]').exists()).toBe(true);
    expect(wrapper.get('.board-native-path').attributes('d')).toBe('M 4 60 L 35 20 L 220 80');
    expect(wrapper.get('.board-native-path').attributes('stroke')).toBe('#788baa');
    expect(wrapper.get('[data-board-link="cause"]').attributes('aria-label')).toBe('更新');
  });
  it('changes visibility, highlights, stroke progress and text reveal only through controlled props', async () => {
    const items = makeItems(), original = JSON.stringify(items);
    const wrapper = setup({ items, transition: 'draw', progress: { path: .25, cause: .4, shape: .3 }, highlightTargets: [{ kind: 'item', item_id: 'shape' }, { kind: 'link', link_id: 'cause' }] });
    expect(wrapper.get('.board-native-path').attributes('stroke-dashoffset')).toBe('0.75');
    expect(wrapper.get('.board-link-stroke').attributes('stroke-dashoffset')).toBe('0.6');
    expect(wrapper.get('[data-item-id="shape"] .board-shape-outline').attributes()).toMatchObject({ 'stroke-dashoffset': '0.7', 'fill-opacity': '0.3' });
    expect(wrapper.get('[data-item-id="shape"] .board-shape-echo').attributes('stroke-dashoffset')).toBe('0.7');
    expect(wrapper.get('[data-item-id="shape"]').classes()).toContain('board-highlighted-item');
    expect(wrapper.get('[data-board-link="cause"]').classes()).toContain('board-highlighted-link');
    await wrapper.setProps({ transition: 'reveal_text', progress: { text: .5 } });
    expect(wrapper.get('[data-item-id="text"] [data-board-drawing-text]').text()).toBe('AB');
    expect(wrapper.get('[data-item-id="text"] [role="button"]').attributes('aria-label')).toBe('ABCD');
    expect(wrapper.get('.board-native-path').attributes('stroke-dashoffset')).toBe('0');
    await wrapper.setProps({ transition: 'instant' });
    expect(wrapper.get('[data-item-id="text"] [data-board-drawing-text]').text()).toBe('ABCD');
    await wrapper.setProps({ visibleItemIds: ['shape'], visibleLinkIds: [] });
    expect(wrapper.findAll('[data-item-id]')).toHaveLength(1); expect(wrapper.findAll('[data-board-link]')).toHaveLength(0);
    expect(JSON.stringify(items)).toBe(original); expect(wrapper.emitted('move')).toBeUndefined(); expect(wrapper.emitted('view')).toBeUndefined();
  });
  it('keeps title and wrapped body positions stable throughout text reveal without losing source content', async () => {
    const item = drawing('state', { kind: 'shape', shape: 'rectangle', text: '① 固定池\n把新信息放进固定大小的状态\n每一步都保留更新后的结果', color: '#269b92' });
    item.title = '固定池'; item.size = { width: 240, height: 170 };
    const wrapper = setup({ items: [item], links: [] });
    const lines = () => wrapper.findAll('[data-board-drawing-text] tspan');
    const positions = lines().map(line => ({ x: line.attributes('x'), y: line.attributes('y'), fontSize: line.attributes('font-size') }));
    expect(lines()[0].attributes('font-weight')).toBe('620');
    expect(lines().slice(1).every(line => line.attributes('font-weight') === '400')).toBe(true);
    expect(wrapper.get('[data-board-drawing-text]').text()).toBe('① 固定池把新信息放进固定大小的状态每一步都保留更新后的结果');
    await wrapper.setProps({ transition: 'reveal_text', progress: { state: .45 } });
    expect(lines().map(line => ({ x: line.attributes('x'), y: line.attributes('y'), fontSize: line.attributes('font-size') }))).toEqual(positions);
    expect(wrapper.get('[data-board-drawing-text]').text()).toBe('① 固定池把新信息放进固定大');
    await wrapper.setProps({ progress: { state: 1 } });
    expect(wrapper.get('[data-board-drawing-text]').text()).toBe('① 固定池把新信息放进固定大小的状态每一步都保留更新后的结果');
  });
  it('renders mathematical notation as intact formulas and reveals them without replacing their measured markup', async () => {
    const item = drawing('update', { kind: 'shape', shape: 'rectangle', text: '状态更新\n写入 $S_t = S_{t-1} + k_t v_t^T$\n保持固定尺寸', color: '#3c928b' });
    item.title = '状态更新'; item.size = { width: 300, height: 160 };
    const wrapper = setup({ items: [item], links: [], transition: 'reveal_text', progress: { update: 0 } });
    expect(wrapper.find('foreignObject[data-board-drawing-text]').exists()).toBe(true);
    expect(wrapper.find('[data-board-rich-text] .katex').exists()).toBe(true);
    expect(wrapper.find('.katex-error').exists()).toBe(false);
    const formula = wrapper.get('[data-board-math]');
    const element = formula.element, html = formula.html();
    expect(formula.attributes('style')).toContain('visibility: hidden');
    expect(wrapper.findAll('.board-rich-character').every(character => character.attributes('style')?.includes('visibility: hidden'))).toBe(true);
    await wrapper.setProps({ progress: { update: .5 } });
    expect(wrapper.get('[data-board-math]').element).toBe(element);
    expect(wrapper.get('[data-board-math]').attributes('style')).toContain('visibility: visible');
    expect(wrapper.get('[data-board-math]').html().replace('visibility: visible', 'visibility: hidden')).toBe(html);
    await wrapper.get('[data-item-id="update"] [role="button"]').trigger('keydown', { key: 'ArrowRight' });
    expect(wrapper.emitted('move')).toEqual([['update', { x: 12, y: 0 }]]);
    expect(item.target.kind === 'drawing' && item.target.graphic.kind === 'shape' && item.target.graphic.text).toBe('状态更新\n写入 $S_t = S_{t-1} + k_t v_t^T$\n保持固定尺寸');
  });
  it('activates explicit links by mouse and keyboard and selects drawings without conflating the two targets', async () => {
    const wrapper = setup();
    await wrapper.get('[data-board-link="cause"]').trigger('click');
    expect(wrapper.emitted('activate-target')?.[0]).toEqual([{ kind: 'link', link_id: 'cause' }]);
    await wrapper.get('[data-board-link="cause"]').trigger('keydown', { key: 'Enter' });
    expect(wrapper.emitted('activate-target')).toHaveLength(2);
    await wrapper.get('[data-item-id="shape"]').trigger('keydown', { key: 'Enter' });
    expect(wrapper.emitted('select')?.[0]).toEqual(['shape']);
    expect(wrapper.emitted('user-control')).toEqual([['keyboard'], ['keyboard']]);
  });
  it('places a shared note relation label above adjacent cards when their 40px gap cannot contain it', async () => {
    const notes: BoardItem[] = ['first-note', 'second-note'].map((noteId, index) => ({
      item_id: `item-${noteId}`, title: noteId, readable_content: '练习记录', explanation_blocks: [],
      position: { x: index * 360, y: 0 }, size: { width: 320, height: 220 },
      created_at: '2026-10-27', source_bindings: [], target: { kind: 'note', note_id: noteId, missing: false },
    }));
    const relation: BoardLink = {
      link_id: 'shared-practice-link', note_relation_id: 'practice-relation',
      from: { kind: 'item', item_id: notes[0].item_id }, to: { kind: 'item', item_id: notes[1].item_id },
      label: '提供练习实例', reason: '这次练习记录展示了反馈如何帮助调整下一次练习。', based_on: [],
    };
    const wrapper = setup({ items: notes, links: [relation] });
    const connection = wrapper.get('[data-board-link="shared-practice-link"]');
    const background = connection.get('.board-note-link-label-bg');
    // SVG attributes are world coordinates; the label must clear both cards,
    // whose top edge is y = 0, rather than sit behind their content.
    expect(Number(background.attributes('y')) + Number(background.attributes('height'))).toBeLessThanOrEqual(0);
    expect(connection.get('.board-link-label').text()).toBe(relation.label);
    expect(connection.attributes('aria-label')).toBe(relation.label);
    expect(connection.get('title').text()).toContain(relation.reason);
    expect(connection.attributes('tabindex')).toBe('0');
    await connection.trigger('keydown', { key: 'Enter' });
    await connection.trigger('keydown', { key: ' ' });
    expect(wrapper.emitted('activate-target')).toEqual([
      [{ kind: 'link', link_id: relation.link_id }], [{ kind: 'link', link_id: relation.link_id }],
    ]);
    expect(wrapper.emitted('user-control')).toEqual([['keyboard'], ['keyboard']]);
  });
  it('recalculates links during a native drag and commits the new world position once on release', async () => {
    const wrapper = setup({ view: { ...view, zoom: .5 } });
    const path = wrapper.get('.board-link-stroke').attributes('d');
    await wrapper.get('[data-item-id="shape"]').trigger('pointerdown', { button: 0, pointerId: 7, clientX: 100, clientY: 100 });
    await wrapper.trigger('pointermove', { pointerId: 7, clientX: 125, clientY: 120 });
    expect(wrapper.get('.board-link-stroke').attributes('d')).not.toBe(path);
    expect(wrapper.get('[data-item-id="shape"]').attributes('style')).toContain('left: 380px');
    expect(wrapper.emitted('move')).toBeUndefined();
    await wrapper.trigger('pointerup', { pointerId: 7, clientX: 125, clientY: 120 });
    expect(wrapper.emitted('move')).toEqual([['shape', { x: 380, y: 40 }]]);
    expect(wrapper.emitted('user-control')).toEqual([['drag']]); expect(wrapper.emitted('select')).toBeUndefined();
  });
  it('fits multiple current targets programmatically without interrupting playback, but reports actual camera controls', async () => {
    const wrapper = setup();
    wrapper.vm.focusTargets([{ kind: 'item', item_id: 'text' }, { kind: 'item', item_id: 'ellipse' }]);
    const camera = wrapper.emitted('view')![0][0] as BoardView;
    expect(camera.x).toBeCloseTo(115); expect(camera.y).toBeCloseTo(160);
    wrapper.vm.fit(); wrapper.vm.locate('shape');
    expect(wrapper.emitted('user-control')).toBeUndefined();
    await wrapper.get('[aria-label="放大画板"]').trigger('click');
    await wrapper.trigger('wheel', { deltaX: 0, deltaY: 20 });
    await wrapper.get('[data-item-id="shape"]').trigger('keydown', { key: 'ArrowRight' });
    expect(wrapper.emitted('user-control')).toEqual([['zoom'], ['pan'], ['keyboard']]);
    expect(wrapper.emitted('move')).toEqual([['shape', { x: 342, y: 0 }]]);
  });
  it('restores a readable explicit focus on a 390px canvas without zooming ordinary selection or manual browsing', async () => {
    const item = drawing('speed', { kind: 'text', text: '120 米 ÷ 80 秒 = 1.5 m/s' }, 800, 300);
    item.size.width = 290;
    const wrapper = setup({ items: [item], links: [], view: { ...view, zoom: .25 } }, { width: 390, height: 500 });
    await wrapper.setProps({ highlightTargets: [{ kind: 'item', item_id: 'speed' }] });
    await wrapper.get('[data-item-id="speed"]').trigger('keydown', { key: 'Enter' });
    expect(wrapper.emitted('select')).toEqual([['speed']]); expect(wrapper.emitted('view')).toBeUndefined();
    wrapper.vm.focusTargets([{ kind: 'item', item_id: 'speed' }]);
    const camera = wrapper.emitted('view')![0][0] as BoardView;
    expect(camera.zoom).toBe(1); expect(item.size.width * camera.zoom).toBe(290);
    expect((item.position.x + item.size.width / 2) * camera.zoom + camera.x).toBe(195);
    wrapper.vm.locate('speed');
    expect((wrapper.emitted('view')![1][0] as BoardView).zoom).toBe(1);
    await wrapper.setProps({ view: { ...camera, zoom: .6 } });
    await wrapper.trigger('wheel', { deltaX: 15, deltaY: 25 });
    expect((wrapper.emitted('view')!.at(-1)![0] as BoardView).zoom).toBe(.6);
    expect(wrapper.emitted('user-control')).toEqual([['keyboard'], ['pan']]);
  });
  it('uses the same renderer for a readonly preview while disabling edits and preserving manual camera browsing', async () => {
    const wrapper = setup({ readonly: true });
    await wrapper.get('[data-item-id="shape"]').trigger('pointerdown', { button: 0, pointerId: 4, clientX: 10, clientY: 10 });
    await wrapper.trigger('pointermove', { pointerId: 4, clientX: 80, clientY: 80 });
    await wrapper.trigger('pointerup', { pointerId: 4 });
    await wrapper.get('[data-item-id="shape"]').trigger('keydown', { key: 'ArrowRight' });
    expect(wrapper.emitted('move')).toBeUndefined();
    await wrapper.trigger('wheel', { deltaY: 24, deltaX: 0 });
    expect(wrapper.emitted('view')).toHaveLength(1);
    expect(wrapper.get('[data-item-id="text"] [data-board-drawing-text]').text()).toBe('ABCD');
  });
  it('gives thin native paths and explicit connections a 44px control without changing retained world geometry', () => {
    const item = drawing('thin', { kind: 'path', points: [{ x: 0, y: 4 }, { x: 220, y: 4 }], color: '#788baa', width: 2 }, 40, 80);
    item.size = { width: 240, height: 8 };
    const wrapper = setup({ items: [item, makeItems()[1]], view: { ...view, zoom: .25 }, links: [{ ...link, from: { kind: 'item', item_id: 'thin' } }] });
    expect(wrapper.get('[data-item-id="thin"]').attributes('style')).toContain('height: 8px');
    expect(wrapper.get('[data-item-id="thin"]').attributes('viewBox')).toBe('0 0 240 8');
    expect(wrapper.find('[data-item-id="thin"] .board-graphic-touch-target').exists()).toBe(false);
    expect(wrapper.get('[data-item-id="thin"] .board-graphic-path-hit').attributes('stroke-width')).toBe('176');
    expect(wrapper.get('[data-item-id="thin"] .board-graphic-path-hit').attributes('d')).toBe('M 0 4 L 220 4');
    expect(wrapper.get('[data-board-link="cause"] .board-link-touch-target').attributes('height')).toBe('176');
    expect(item.position).toEqual({ x: 40, y: 80 }); expect(item.size).toEqual({ width: 240, height: 8 });
  });
  it('uses only the annotation stroke as a pointer target while keeping its keyboard and drag controls', async () => {
    const node = drawing('state', { kind: 'shape', shape: 'rectangle', text: '状态矩阵', color: '#6b8d7a' }, 70, 70);
    node.size = { width: 120, height: 70 };
    const circle = drawing('circle', { kind: 'path', points: [{ x: 15, y: 15 }, { x: 250, y: 15 }, { x: 250, y: 190 }, { x: 15, y: 190 }, { x: 15, y: 15 }], color: '#4d83bd', width: 3.5 });
    circle.size = { width: 265, height: 205 };
    const wrapper = setup({ items: [node, circle], links: [] });
    const annotation = wrapper.get('[data-item-id="circle"]');
    // happy-dom does not hit-test SVG geometry. These assertions enforce the
    // browser hit primitives; the browser check covers actual click-through.
    expect(annotation.classes()).toContain('board-path-drawing');
    expect(annotation.find('.board-graphic-touch-target').exists()).toBe(false);
    const hit = annotation.get('.board-graphic-path-hit');
    expect(hit.attributes()).toMatchObject({ fill: 'none', stroke: 'transparent', 'pointer-events': 'stroke', 'stroke-width': '44' });
    expect(hit.attributes('d')).toBe(annotation.get('.board-native-path').attributes('d'));
    expect(wrapper.find('[data-item-id="state"] .board-graphic-touch-target').exists()).toBe(true);
    const control = annotation.get('[role="button"]');
    expect(control.attributes('tabindex')).toBe('0');
    await control.trigger('keydown', { key: 'Enter' });
    await control.trigger('keydown', { key: 'ArrowRight', shiftKey: true });
    expect(wrapper.emitted('select')).toEqual([['circle']]);
    expect(wrapper.emitted('move')).toEqual([['circle', { x: 40, y: 0 }]]);
    await hit.trigger('pointerdown', { button: 0, pointerId: 8, clientX: 15, clientY: 15 });
    await wrapper.trigger('pointermove', { pointerId: 8, clientX: 35, clientY: 45 });
    await wrapper.trigger('pointerup', { pointerId: 8, clientX: 35, clientY: 45 });
    expect(wrapper.emitted('move')?.at(-1)).toEqual(['circle', { x: 20, y: 30 }]);
  });
});

describe('board world geometry', () => {
  it('fits an oversized explicit target inside the actual viewport even below the overview zoom limit', () => {
    const bounds = { x: 600, y: 240, width: 1800, height: 600 };
    const camera = focusBoardBounds(bounds, 390, 500, { ...view, zoom: .25 });
    expect(camera.zoom).toBeCloseTo(294 / 1800);
    expect(bounds.x * camera.zoom + camera.x).toBeCloseTo(48);
    expect((bounds.x + bounds.width) * camera.zoom + camera.x).toBeCloseTo(342);
    expect(bounds.y * camera.zoom + camera.y).toBeGreaterThanOrEqual(60);
    expect((bounds.y + bounds.height) * camera.zoom + camera.y).toBeLessThanOrEqual(440);
  });
  it('wraps the actual 290px speed label without orphaning the unit, while retaining Chinese and long-token wrapping', () => {
    const text = '120 米 ÷ 80 秒 = 1.5 m/s';
    const lines = graphicTextLines(text, 290 - 16, 22);
    expect(lines).toEqual(['120 米 ÷ 80 秒 = 1.5', 'm/s']);
    expect(lines.join(' ')).toBe(text);
    expect(graphicTextLines('甲乙丙丁', 45, 20)).toEqual(['甲乙', '丙丁']);
    expect(graphicTextLines('甲ABCDEFGHI', 30, 10)).toEqual(['甲', 'ABCDE', 'FGHI']);
  });
  it('resolves current connection bounds, follows link endpoints and refuses a different presentation version', () => {
    const items = makeItems();
    const second: BoardLink = { ...link, link_id: 'explanation', from: { kind: 'link', link_id: 'cause' }, to: { kind: 'item', item_id: 'ellipse' } };
    const first = linkGeometry(link, items, [link, second])!;
    expect(targetBounds({ kind: 'link', link_id: 'cause' }, items, [link, second])).toEqual(first.bounds);
    expect(linkGeometry(second, items, [link, second])).not.toBeNull();
    const presentation: BoardItem = { ...items[0], item_id: 'demo', target: { kind: 'presentation', receipt: { session_id: 's', turn_id: 't', reference: { presentation_id: 'p', revision: 3 }, state_revision: 1, saved_state_ref: 'state' } } };
    const anchor = { kind: 'presentation_anchor' as const, item_id: 'demo', reference: { presentation_id: 'p', revision: 3 }, anchor_id: 'result' };
    expect(targetBounds(anchor, [presentation], [])).toEqual({ ...presentation.position, ...presentation.size });
    expect(targetBounds({ ...anchor, reference: { presentation_id: 'p', revision: 2 } }, [presentation], [])).toBeNull();
    expect(graphicTextLines('甲乙\nABCD', 45, 20)).toEqual(['甲乙', 'ABC', 'D']);
  });
});

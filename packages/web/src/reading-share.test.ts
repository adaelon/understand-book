// @vitest-environment happy-dom
import { describe, expect, it } from 'vitest';
import { answerShareSource, deliveredAnswerText, shareSections, excerptShareSource, noteShareSource, type ShareDraft } from './reading-share';
import type { MemoryRecord } from './api';

const record: MemoryRecord = { mem_id: 'private-id', book_id: 'private-book', type: 'note', layer: 'long_term', anchor: {}, content: '先提出自己的解释，再用证据核对。' };
const draft = (): ShareDraft => ({ source: noteShareSource(record, ['无已记录出处']), selected: ['body'], title: '阅读与理解', reflection: '', layout: 'understanding', palette: 'paper' });

it('RS6 shares only delivered answer text with explicit assistant identity and no private fields', () => {
  const turn = { turnId: 'private-turn', pending: false, outcome: { answer: 'fallback', answer_view: { parts: [
    { kind: 'markdown', text: '当条件成立时，' }, { kind: 'sources', source_ref_ids: ['private-source'] },
    { kind: 'markdown', text: '结论成立。' }, { kind: 'presentation', presentation_id: 'private-presentation', revision: 1 },
  ], sources: [] } } } as any;
  const full = deliveredAnswerText(turn);
  expect(full).toBe('当条件成立时，结论成立。');
  expect(deliveredAnswerText({ ...turn, pending: true })).toBe('');
  expect(deliveredAnswerText({ ...turn, error: 'failed' })).toBe('');
  expect(deliveredAnswerText({ ...turn, outcome: { ...turn.outcome, incomplete: true } })).toBe('');
  const source = answerShareSource(full, full, 'private-session', turn.turnId, []);
  expect(source.sources).toEqual(['无已记录出处']);
  const sections = shareSections({ ...draft(), source, reflection: '我补充的感想' }).map(section => section.textContent).join('');
  expect(sections).toBe('助手解释当条件成立时，结论成立。\n我的感想我补充的感想');
  expect(sections).not.toContain('private-');
});

describe('RS1 selected content and readable page', () => {
  it('keeps legacy identity neutral, structured parts separate, and protocol fields out of images', () => {
    expect(draft().source.parts[0].label).toBe('阅读笔记');
    const newNote: MemoryRecord = { ...record, note: { material: { book_id: 'private-book', publication_id: 'private-publication' },
      association: { kind: 'presentation', title: '斜率演示', receipt: { session_id: 'private-chat', turn_id: 'private-turn', reference: { presentation_id: 'private-presentation', revision: 2 }, state_revision: 1, saved_state_ref: 'private-state' } },
      retained_excerpt: { kind: 'assistant', text: '助手保留片段' }, source_bindings: [] } };
    const source = noteShareSource(newNote, ['微积分 · 导数']);
    expect(source.parts.map(p => p.label)).toEqual(['我的笔记', '助手解释']);
    expect(source.association).toBe('记录于演示「斜率演示」');
    const selected = { ...draft(), source, selected: ['excerpt'] as const };
    const output = shareSections({ ...selected, selected: [...selected.selected] }).map(section => section.textContent).join('');
    expect(output).toContain('助手保留片段'); expect(output).not.toContain(record.content); expect(output).not.toContain('private-');
    expect(newNote.content).toBe(record.content);
  });
  it('foregrounds the excerpt only in excerpt layout without rewriting content or attribution', () => {
    const input = draft(); input.source.parts.push({ id: 'excerpt', label: '原文摘录', text: '原文中的一句话。' }); input.selected.push('excerpt');
    const normal = shareSections(input).map(section => section.firstElementChild!.textContent);
    const excerpt = shareSections({ ...input, layout: 'excerpt' }).map(section => section.firstElementChild!.textContent);
    expect(normal.indexOf('阅读笔记')).toBeLessThan(normal.indexOf('原文摘录'));
    expect(excerpt.indexOf('原文摘录')).toBeLessThan(excerpt.indexOf('阅读笔记'));
  });
  it('renders paragraphs, lists, code, formulas, links and tables using the reader format', () => {
    const input = draft(); input.source.parts[0].text = '# 标题\n\n**重点**与 `code`\n\n- 第一项\n- 第二项\n\n```js\nconst x = 1;\n```\n\n$$x^2 + y^2$$\n\n|列一|列二|\n|---|---|\n|值一|值二|';
    const section = shareSections(input)[0];
    expect(section.querySelector('strong')?.textContent).toBe('重点');
    expect(section.querySelectorAll('li')).toHaveLength(2);
    expect(section.querySelector('pre')?.textContent).toBe('const x = 1;\n');
    expect(section.querySelector('.katex annotation')?.textContent?.trim()).toBe('x^2 + y^2');
    expect(section.querySelector('td')?.textContent).toBe('值一');
  });
  it('keeps PDF punctuation literal and reflections separate from source Markdown', () => {
    const input = draft(); input.source = excerptShareSource('2 * 3 * 4 与 $price', ['第 2 页'], 'plain'); input.selected = ['excerpt']; input.reflection = '**我的原话**';
    const sections = shareSections(input);
    expect(sections[0].querySelector('p')?.textContent).toBe('2 * 3 * 4 与 $price');
    expect(sections[0].querySelector('em,.katex')).toBeNull();
    expect(sections[1].textContent).toBe('我的感想**我的原话**');
  });
  it('rejects missing resources and empty selections with an actionable reason', () => {
    const input = draft(); input.source.parts[0].text = '![图](missing.png)';
    expect(() => shareSections(input)).toThrow(/图片或内嵌资源/);
    expect(() => shareSections({ ...draft(), selected: [] })).toThrow(/至少选择/);
  });
});


it('RS7 keeps the diagram and its state readable in both orientations', () => {
  const source = { parts: [{ id: 'body' as const, label: '图解现场', text: '步骤 2：参数 3', format: 'plain' as const }], sources: ['原材料'], association: '图解与说明', diagram: { png: 'data:image/png;base64,AA==', width: 960, height: 360, title: '图解' } };
  for (const orientation of ['landscape', 'portrait'] as const) {
    const sections = shareSections({ source, selected: ['body'], title: '图解', reflection: '', layout: 'diagram', palette: 'blue', orientation });
    expect(sections[0].querySelector('img')?.src).toBe(source.diagram.png);
    expect(sections[1].textContent).toContain('步骤 2：参数 3');
  }
  source.diagram.height = 3000;
  expect(() => shareSections({ source, selected: ['body'], title: '图解', reflection: '', layout: 'diagram', palette: 'blue' })).toThrow('难以阅读');
});

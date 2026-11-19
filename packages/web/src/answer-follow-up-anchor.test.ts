// @vitest-environment happy-dom
import { expect, it } from 'vitest';
import type { AgentAnswerPart } from './api';
import type { FollowUpAnchor } from './follow-up-conversations';
import { answerDisplayParts } from './answer-markdown';
import { answerFollowUpAnchor, answerFollowUpRange, markAnswerFollowUps } from './answer-follow-up-anchor';

function display(parts: AgentAnswerPart[], partIndex = 0) {
  const part = answerDisplayParts(parts, [])[partIndex];
  if (part.kind !== 'html') throw Error('Expected text');
  const root = document.createElement('div');
  root.innerHTML = part.html;
  return { part, root };
}

it('opens the selected occurrence and restores that exact occurrence after rendering again', () => {
  const source = '🧪对照组和第二个**对照组**。';
  const { part, root } = display([{ kind:'markdown', text:source }]);
  const range = document.createRange();
  range.selectNodeContents(root.querySelector('strong')!);
  const anchor = answerFollowUpAnchor(range, part, root)!;
  const start = source.lastIndexOf('对照组');
  expect(anchor).toEqual({ start_part:0, start_offset:start, end_part:0, end_offset:start + 3, excerpt:'对照组' });
  root.innerHTML = markAnswerFollowUps(part, [{ session_id:'child', anchor }]);
  expect(root.querySelectorAll('[data-follow-up-id]')).toHaveLength(1);
  const marked = root.querySelector('strong > mark[data-follow-up-id="child"]')!;
  expect(marked.textContent).toBe('对照组');
  expect(marked.getAttribute('tabindex')).toBe('0');
  const restored = answerFollowUpRange(part, root, anchor)!;
  expect(restored.startContainer).toBe(marked.firstChild);
  expect(restored.toString()).toBe(anchor.excerpt);
  expect(answerFollowUpAnchor(restored, part, root)).toEqual(anchor);
});

it('keeps the stored part indices across a citation without making the citation a branch link', () => {
  const { part, root } = display([
    { kind:'markdown', text:'先看对照' }, { kind:'sources', source_ref_ids:['source'] },
    { kind:'markdown', text:'组再比较' },
  ]);
  const p = root.querySelector('p')!;
  const range = document.createRange();
  range.setStart(p.firstChild!, 2);
  range.setEnd(p.lastChild!, 1);
  const anchor = answerFollowUpAnchor(range, part, root)!;
  expect(anchor).toEqual({ start_part:0, start_offset:2, end_part:2, end_offset:1, excerpt:'对照组' });
  root.innerHTML = markAnswerFollowUps(part, [{ session_id:'child', anchor }]);
  expect(Array.from(root.querySelectorAll('mark'), m => m.textContent)).toEqual(['对照', '组']);
  expect(root.querySelector('button mark')).toBeNull();
  const ranges = answerFollowUpRange(part, root, anchor)!;
  expect(answerFollowUpAnchor(ranges, part, root)).toEqual(anchor);
  range.selectNodeContents(root.querySelector('.agent-source-button span')!);
  expect(answerFollowUpAnchor(range, part, root)).toBeNull();
});

it('leaves changed or missing text unresolved even if the same quote occurs elsewhere', () => {
  const { part, root } = display([{ kind:'markdown', text:'先看分数，再看分组' }]);
  const stale = { start_part:0, start_offset:2, end_part:0, end_offset:4, excerpt:'分组' };
  expect(answerFollowUpRange(part, root, stale)).toBeNull();
  expect(markAnswerFollowUps(part, [{ session_id:'child', anchor:stale }])).toBe(part.html);
});

it('keeps playable music controls outside a cross-part quote and maps text after them', () => {
  const { part, root } = display([
    { kind:'markdown', text:'先听' },
    { kind:'music_example', example:{ ref_id:'one', title:'一组音', kind:'score', tempo_bpm:60, source_ref_id:null,
      label:'C4', duration_ms:400, notes:[{ midi_pitch:60, start_ms:0, end_ms:400 }] } },
    { kind:'markdown', text:'再猜' },
  ]);
  const range = document.createRange();
  range.selectNodeContents(root.querySelector('p')!);
  expect(answerFollowUpAnchor(range, part, root)).toBeNull();
  range.selectNodeContents(root.querySelector('p')!.lastChild!);
  expect(answerFollowUpAnchor(range, part, root)).toEqual({ start_part:2,start_offset:0,end_part:2,end_offset:2,excerpt:'再猜' });
});

it('gives a separate Socratic question the original answer part index', () => {
  const { part, root } = display([
    { kind:'markdown', text:'先比较两组。' }, { kind:'socratic_question', text:'你会怎样分组？' },
  ], 1);
  const range = document.createRange();
  range.selectNodeContents(root.querySelector('p')!);
  expect(answerFollowUpAnchor(range, part, root)?.start_part).toBe(1);
});

it('keeps nested quotes distinguishable without changing the rendered sentence', () => {
  const { part, root } = display([{kind:'markdown',text:'两组学生都上课。'}]);
  const make = (start:number, end:number): FollowUpAnchor => ({
    start_part:0, start_offset:start, end_part:0, end_offset:end, excerpt:part.markdown[0].slice(start,end),
  });
  root.innerHTML = markAnswerFollowUps(part, [{session_id:'wide',anchor:make(0,7)},{session_id:'narrow',anchor:make(2,4)}]);
  expect(root.querySelector('[data-follow-up-id="narrow"]')?.textContent).toBe('学生');
  expect(root.querySelector('p')?.textContent).toBe(part.markdown[0]);
});

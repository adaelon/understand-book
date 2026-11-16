import { t as uiText } from './i18n';
import type { LearningPurpose } from './generated/LearningPurpose';
import type { ReaderManifest, OutlineItem } from './reader-outline';

export type LearningScopeKind = 'selection' | 'chapter' | 'section' | 'media' | 'material';
export interface LearningScopeOption { value: LearningScopeKind; label: string; lid?: string }
export interface LearningPurposeStart { message?: string; display?: string }

/** A document-wide heading is the material, not a second chapter-sized scope. */
export function currentLearningSection(manifest: ReaderManifest | null, outline: OutlineItem[], anchor: string | null): OutlineItem | null {
  if (!manifest || !anchor) return null;
  const at = manifest.tree.find(node => node.lid === anchor);
  if (!at) return null;
  const { start, end } = manifest.tree.reduce((span, node) => ({ start: Math.min(span.start, node.span.start), end: Math.max(span.end, node.span.end) }), { start: Infinity, end: -Infinity });
  const container = manifest.tree.filter(node => (node.kind === 'chapter' || node.kind === 'section')
    && (anchor === node.lid || anchor.startsWith(`${node.lid}.`))
    && (node.span.start > start || node.span.end < end))
    .sort((a, b) => b.lid.split('.').length - a.lid.split('.').length)[0];
  return container ? outline.find(item => item.lid === container.lid) ?? {
    lid: container.lid, kind: container.kind, depth: container.lid.split('.').length - 1,
    title: container.title || uiText('正文'), span: container.span,
  } : null;
}

export const learningPurposes: { purpose: LearningPurpose; label: string; description: string }[] = [
  { purpose: 'discover', label: uiText('先了解重点'), description: uiText('直接梳理所选内容的要点与关系') },
  { purpose: 'understand', label: uiText('带我读懂'), description: uiText('告诉我哪里没懂，一起讲清楚') },
  { purpose: 'exam', label: uiText('复习备考'), description: uiText('先确定考试或复习目标') },
  { purpose: 'explore', label: uiText('探索一个问题'), description: uiText('写下疑问，或先找一个切入点') },
  { purpose: 'practice', label: uiText('练一练'), description: uiText('先确定练什么，再出一道题') },
];

export const musicLearningPurposes: typeof learningPurposes = [
  { purpose: 'discover', label: uiText('先了解重点'), description: uiText('先听主题与声部的关系') },
  { purpose: 'understand', label: uiText('带我听懂'), description: uiText('边听边看乐谱，逐步理解') },
  { purpose: 'exam', label: uiText('复习备考'), description: uiText('先确定考试或复习目标') },
  { purpose: 'explore', label: uiText('探索一个问题'), description: uiText('写下疑问，或先找一个切入点') },
  { purpose: 'practice', label: uiText('练一练'), description: uiText('先确定练什么，再出一道题') },
];

export const videoLearningPurposes: typeof learningPurposes = [
  { purpose: 'discover', label: uiText('先了解重点'), description: uiText('先看这段内容的重点与关系') },
  { purpose: 'understand', label: uiText('带我看懂'), description: uiText('结合画面与讲解逐步理解') },
  { purpose: 'exam', label: uiText('复习备考'), description: uiText('先确定考试或复习目标') },
  { purpose: 'explore', label: uiText('探索一个问题'), description: uiText('写下疑问，或先找一个切入点') },
  { purpose: 'practice', label: uiText('练一练'), description: uiText('先确定练什么，再出一道题') },
];

export function learningPurposeLabel(purpose: LearningPurpose, musicMode = false, videoMode = false): string {
  return (musicMode ? musicLearningPurposes : videoMode ? videoLearningPurposes : learningPurposes).find(item => item.purpose === purpose)!.label;
}

export function learningPurposeSetup(purpose: LearningPurpose, musicMode = false, videoMode = false) {
  switch (purpose) {
    case 'understand': return {
      question: musicMode ? uiText('想听懂哪里？') : videoMode ? uiText('想看懂哪里？') : uiText('想读懂哪里？'),
      placeholder: uiText('写一个概念、一段话，或你卡住的地方。'),
      outcome: uiText('先围绕你的困惑讲清一个重点，再用一个小例子确认理解。'),
      submit: uiText('从这里开始讲解'), fallback: uiText('还没想好，从所选内容的重点开始'),
      fallbackMessage: uiText('我还没有具体困惑。请先从所选内容中选出一个值得理解的重点，说明它是什么、为什么重要，再带我逐步理解。'),
    };
    case 'exam': return {
      question: uiText('准备什么考试，或想复习什么？'),
      placeholder: uiText('例如：考试名称、考查范围、题型，或最不熟悉的知识点。'),
      outcome: uiText('按你的目标先做一次小检查，再讲解薄弱点；每次只推进一题。'),
      submit: uiText('按这个目标复习'), fallback: uiText('没有具体考试，先检查这部分的掌握情况'),
      fallbackMessage: uiText('我没有具体考试目标。请围绕所选内容先检查我的掌握情况，从一个基础问题开始，根据我的回答决定接下来复习什么。'),
    };
    case 'explore': return {
      question: uiText('你想探索什么问题？'),
      placeholder: uiText('写下想追问的原因、想比较的观点，或需要查证的说法。'),
      outcome: uiText('围绕这个问题回到材料找依据，分清已有解释与仍待查证的部分。'),
      submit: uiText('围绕这个问题探索'), fallback: uiText('还没有问题，先推荐几个切入点'),
      fallbackMessage: uiText('我还没有具体问题。请根据所选材料推荐两到三个值得追问的问题。每项只写一句问题、一个简短的探索理由和原文出处，保留原文的适用条件与程度，不把“有时、更多”说成“总是、只能”。不要开场概述或展开例子，先不要回答这些问题，等我选一个后再展开。'),
    };
    case 'practice': return {
      question: uiText('想练哪个学过的内容？'),
      placeholder: uiText('写下学过的概念、方法，或想尝试解决的一类问题。'),
      outcome: uiText('围绕你学过的内容出一道新情境题，等你作答后再反馈；卡住时可以要提示。'),
      submit: uiText('围绕这个内容出题'), fallback: musicMode ? uiText('还没学过，先带我听懂') : videoMode ? uiText('还没学过，先带我看懂') : uiText('还没学过，先带我读懂'), fallbackMessage: '',
    };
    default: return null;
  }
}

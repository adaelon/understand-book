import type { LearningPurpose } from './generated/LearningPurpose';

export type LearningScopeKind = 'selection' | 'chapter' | 'material';
export interface LearningScopeOption { value: LearningScopeKind; label: string }

export const learningPurposes: { purpose: LearningPurpose; label: string; description: string }[] = [
  { purpose: 'discover', label: '先了解重点', description: '先看主要内容与关系' },
  { purpose: 'understand', label: '带我读懂', description: '结合讲解与尝试逐步理解' },
  { purpose: 'exam', label: '复习备考', description: '从回忆和应用找到需要补的地方' },
  { purpose: 'explore', label: '探索一个问题', description: '找到值得追问和查证的问题' },
];

export const musicLearningPurposes: typeof learningPurposes = [
  { purpose: 'discover', label: '先了解重点', description: '先听主题与声部的关系' },
  { purpose: 'understand', label: '带我听懂', description: '边听边看乐谱，逐步理解' },
  { purpose: 'exam', label: '复习备考', description: '从重听和辨认找到需要补的地方' },
  { purpose: 'explore', label: '探索一个问题', description: '追问听到的变化，回到音乐中查证' },
];

export function learningPurposeLabel(purpose: LearningPurpose, musicMode = false): string {
  return (musicMode ? musicLearningPurposes : learningPurposes).find(item => item.purpose === purpose)!.label;
}

import { t as uiText } from './i18n';
import type { LearningPurpose } from './generated/LearningPurpose';

export type LearningScopeKind = 'selection' | 'chapter' | 'media' | 'material';
export interface LearningScopeOption { value: LearningScopeKind; label: string }

export const learningPurposes: { purpose: LearningPurpose; label: string; description: string }[] = [
  { purpose: 'discover', label: uiText('先了解重点'), description: uiText('先看主要内容与关系') },
  { purpose: 'understand', label: uiText('带我读懂'), description: uiText('结合讲解与尝试逐步理解') },
  { purpose: 'exam', label: uiText('复习备考'), description: uiText('从回忆和应用找到需要补的地方') },
  { purpose: 'explore', label: uiText('探索一个问题'), description: uiText('找到值得追问和查证的问题') },
];

export const musicLearningPurposes: typeof learningPurposes = [
  { purpose: 'discover', label: uiText('先了解重点'), description: uiText('先听主题与声部的关系') },
  { purpose: 'understand', label: uiText('带我听懂'), description: uiText('边听边看乐谱，逐步理解') },
  { purpose: 'exam', label: uiText('复习备考'), description: uiText('从重听和辨认找到需要补的地方') },
  { purpose: 'explore', label: uiText('探索一个问题'), description: uiText('追问听到的变化，回到音乐中查证') },
];

export const videoLearningPurposes: typeof learningPurposes = [
  { purpose: 'discover', label: uiText('先了解重点'), description: uiText('先看这段内容的重点与关系') },
  { purpose: 'understand', label: uiText('带我看懂'), description: uiText('结合画面与讲解逐步理解') },
  { purpose: 'exam', label: uiText('复习备考'), description: uiText('从回忆和应用找到需要补的地方') },
  { purpose: 'explore', label: uiText('探索一个问题'), description: uiText('回到画面和字幕中追问、查证') },
];

export function learningPurposeLabel(purpose: LearningPurpose, musicMode = false, videoMode = false): string {
  return (musicMode ? musicLearningPurposes : videoMode ? videoLearningPurposes : learningPurposes).find(item => item.purpose === purpose)!.label;
}

import type { Percentage } from './types';

export function percentage(value: Percentage | null | undefined): string {
  if (!value) return '待配置比例';
  if (value.nonzero && value.below_tenth) return value.value.startsWith('-') ? '减少低于 0.1%' : '低于 0.1%';
  return `${value.value}%`;
}
export function percentageInput(text: string): number {
  const match = text.trim().match(/^(-?)(\d+)(?:\.(\d{1,3}))?$/);
  if (!match) throw new Error('百分比最多保留 3 位小数。');
  const integer = BigInt(match[2] + (match[3] ?? '').padEnd(3, '0')) * (match[1] ? -1n : 1n);
  const value = Number(integer);
  if (!Number.isSafeInteger(value)) throw new Error('百分比超出可输入范围。');
  return value;
}
export function milliPercentage(value: number): string { return `${value / 1000}%`; }

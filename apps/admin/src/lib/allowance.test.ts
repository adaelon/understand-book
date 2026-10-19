import { expect, it } from 'vitest';
import { percentage, percentageInput, milliPercentage } from './allowance';
it('keeps fixed server projections and parses exact signed sales steps', () => {
  expect(percentage({ value: '120.7', nonzero: true, below_tenth: false })).toBe('120.7%');
  expect(percentage({ value: '0', nonzero: true, below_tenth: true })).toBe('低于 0.1%');
  expect(percentage(null)).toBe('待配置比例');
  expect(percentageInput(' 50.025 ')).toBe(50025);
  expect(percentageInput('-0.001')).toBe(-1);
  expect(milliPercentage(-1500)).toBe('-1.5%');
  for (const invalid of ['1.1234', '1e2', '0x10', '', '9007199254740992']) expect(() => percentageInput(invalid)).toThrow();
});

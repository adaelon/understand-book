import { describe, it, expect } from 'vitest';
import { hongKongDay, usageQuery, taskChargesPath } from './usage';
import type { UsageTask } from './types';

describe('usage report navigation', () => {
  it('uses Hong Kong calendar days across UTC midnight', () => {
    expect(hongKongDay(Date.parse('2026-10-07T15:59:59Z'))).toBe('2026-10-07');
    expect(hongKongDay(Date.parse('2026-10-07T16:00:00Z'))).toBe('2026-10-08');
    expect(() => usageQuery('2026-10-09','2026-10-08')).toThrow();
  });
  it('preserves account, date range, reference kind and pagination when drilling into tasks', () => {
    for (const kind of ['run','task'] as const) {
      const task={user_id:'A',cost_owner:'account',reference_kind:kind,reference:'ref / + &'} as UsageTask;
      const url=new URL(taskChargesPath(task,'2026-10-07','2026-10-08',20),'https://reader.example');
      expect(Object.fromEntries(url.searchParams)).toEqual({user_id:'A',from:'2026-10-07',to:'2026-10-08',offset:'20',limit:'20',[`${kind}_ref`]:'ref / + &'});
    }
  });
  it('opens platform task charges without an account filter', () => {
    const task={user_id:null,cost_owner:'platform',reference_kind:'operator_task',reference:'public / + &'} as UsageTask;
    const url=new URL(taskChargesPath(task,'2026-10-07','2026-10-08',20),'https://reader.example');
    expect(Object.fromEntries(url.searchParams)).toEqual({from:'2026-10-07',to:'2026-10-08',offset:'20',limit:'20',operator_task_ref:'public / + &'});
  });
});

import { describe, expect, it } from 'vitest';
import { dispositionLabel, effectId, type EffectDisposition } from './effect-disposition';
import { initialRun, reduceRun } from './agent-run-state';
import type { AgentEffect } from './api';

describe('durable effect disposition', () => {
  const highlight: AgentEffect = { kind: 'Highlight', mem_id: 'original', lid: '1.1' };
  it('keeps identity through reorder and new result object mapping', () => {
    const effects: AgentEffect[] = [highlight, { kind: 'Goto', before_anchor: '1.1', after_anchor: '1.2' }];
    const saved: EffectDisposition = { started: { disposition_id: 'd1', effect_id: effectId(highlight), action: 'keep' },
      receipt: { disposition_id: 'd1', original_object_id: 'original', result_object_id: 'retained', error: null } };
    const restored = JSON.parse(JSON.stringify({ [effectId(highlight)]: saved }));
    effects.reverse();
    expect(dispositionLabel(restored[effectId(effects[1])], effects[1])).toBe('已保留');
    expect(dispositionLabel(restored[effectId(effects[0])], effects[0])).toBeUndefined();
    expect(dispositionLabel({ ...saved, receipt: null }, highlight)).toBe('待核对');
    expect(dispositionLabel({ ...saved, receipt: { ...saved.receipt!, error: { error_code: 'FAILED', category: 'storage', message: '保存失败' } } }, highlight)).toBe('处理失败：保存失败');
  });
  it('distinguishes dismissal of an unapplied proposal from undo', () => {
    const proposal = { kind: 'LayoutProposal', proposal: { proposal_id: 'p1', actions: [], base_layout_rev: 0n, summary: 'proposal' } } as AgentEffect;
    const saved: EffectDisposition = { started: { disposition_id: 'd2', effect_id: effectId(proposal), action: 'dismiss' },
      receipt: { disposition_id: 'd2', original_object_id: 'p1', result_object_id: null, error: null } };
    expect(dispositionLabel(saved, proposal)).toBe('已忽略');
  });
  it('updates the same live navigation receipt without creating another effect', () => {
    let state = initialRun({ book_id: 'book', session_id: 'chat', turn_id: 'turn' });
    for (const [seq, after] of [[1, '1.2'], [2, '1.3']] as const) {
      state = reduceRun(state, { turn_id: 'turn', seq, elapsed_ms: 0, type: 'effect.created',
        payload: { effect_id: 'navigation', effect: { kind: 'Goto', before_anchor: '1.1', after_anchor: after } } });
    }
    expect(state.effects).toEqual([{ effect_id: 'navigation', effect: { kind: 'Goto', before_anchor: '1.1', after_anchor: '1.3' } }]);
  });
});

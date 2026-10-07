import type { AgentEffect } from './api';

export interface EffectDisposition {
  started: { disposition_id: string; effect_id: string; action: 'keep' | 'undo' | 'dismiss'; result_object_id?: string | null };
  receipt: { disposition_id: string; original_object_id: string | null; result_object_id: string | null;
    error: { error_code: string; category: string; message: string } | null } | null;
}

export function effectId(effect: AgentEffect): string {
  switch (effect.kind) {
    case 'Goto': return 'navigation';
    case 'Note': case 'Highlight': return `memory:${effect.mem_id}`;
    case 'Layout': return `layout:${effect.effect.after.rev}`;
    case 'LayoutProposal': return `layout-proposal:${effect.proposal.proposal_id}`;
    case 'PaperMinimap': return `minimap:${effect.effect.effect_id}`;
    case 'PaperMinimapProposal': return `minimap-proposal:${effect.proposal.proposal_id}`;
  }
}

export function dispositionLabel(disposition: EffectDisposition | undefined, effect: AgentEffect): string | undefined {
  if (!disposition) return undefined;
  if (!disposition.receipt) return '待核对';
  if (disposition.receipt.error) return `处理失败：${disposition.receipt.error.message}`;
  if (disposition.started.action === 'dismiss') return '已忽略';
  if (disposition.started.action === 'undo') return '已撤销';
  return effect.kind === 'LayoutProposal' || effect.kind === 'PaperMinimapProposal' ? '已应用' : '已保留';
}

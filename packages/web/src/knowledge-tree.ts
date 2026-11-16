import type { KnowledgeNode } from './generated/KnowledgeNode';
import type { KnowledgeSource } from './generated/KnowledgeSource';
import type { UnderstandingSummary } from './note-thinking';
import { ref } from 'vue';
import type { KnowledgeReference } from './generated/KnowledgeReference';
export const pendingKnowledgeReference=ref<KnowledgeReference|null>(null);
export type KnowledgeNodeView = Omit<KnowledgeNode, 'children'> & { child_count: number; fruit_count: number; preview?: string[]; understanding?: UnderstandingSummary[] };
export interface KnowledgeTreeView {
  map_groups?: { topic: KnowledgeNodeView; nodes: KnowledgeNodeView[] }[];
  node: KnowledgeNodeView; children: KnowledgeNodeView[]; breadcrumbs: { id: string; label: string }[];
  next: number | null; total: number; fruits: UnderstandingSummary[]; next_fruit?: number | null; current_ids: string[];
  modality: 'text' | 'music' | 'video';
  current_scope?: string | null;
  entry_point?: KnowledgeNodeView | null;
  introduction?: { text: string; is_opening: boolean } | null;
  updates?: {job_ref:string;status:string}[];
}
export interface KnowledgeTreeRequest { overview?: boolean; node_id?: string; reference?: KnowledgeReference; after?: number; fruit_after?: number; query?: string; current_lid?: string; position_ms?: number }
export const knowledgeKinds: Record<string, string> = {
  material:'内容全貌', chapter:'章节', section:'小节', passage:'原文', concept:'概念', entity:'对象', claim:'论断',
  definition:'定义', explanation:'解释', example:'示例', counterexample:'反例', limitation:'条件', discourse:'讲解',
  music_event:'音乐事件', music_section:'曲式段落', video_unit:'视频片段', discourse_group:'解释与论证', procedure_step:'步骤', method_description:'方法', summary:'小结',key_stop:'关键内容',theme:'主题',themes:'跨章节主题',
  claims_group:'论断', concepts_group:'概念与对象',
};
export const knowledgeRelations: Record<string, string> = {
  supports:'支持', rebuts:'反驳', explains:'解释', exemplifies:'举例说明', example_of:'举例说明', depends_on:'依赖',
  contrasts:'对照', contrasts_with:'对照', elaborates:'展开', causes:'导致', results_in:'带来结果', part_of:'组成',
  repetition:'重复', variation:'变奏', handoff:'交接', response:'回应', support:'支撑', contrast:'对照',
};
export function openKnowledgeSource(source: KnowledgeSource, readerKey: string) {
  window.dispatchEvent(new CustomEvent('knowledge-open-source', { detail: { source, reader_key: readerKey } }));
}
export function discussKnowledge(node: KnowledgeNodeView, readerKey: string) {
  window.dispatchEvent(new CustomEvent('knowledge-discuss', { detail: { node, reader_key: readerKey } }));
}

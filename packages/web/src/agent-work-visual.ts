import { createApp, defineComponent, h, ref } from 'vue';
import RightRail from './components/RightRail.vue';
import type { RunActivity } from './agent-run-state';
import type { OuterOutcome } from './api';
import { renderMarkdown } from './md';
import './style.css';
import './agent-work-visual.css';

// Vite-only preview; the production build does not include this entry.
const query = new URLSearchParams(location.search);
const mobile = query.has('mobile');
const embedded = query.has('embedded');
const activity = (id: number, kind: string, name: string, label: string, status: RunActivity['status'] = 'succeeded'): RunActivity => ({
  step_id: id, parent_step_id: null, kind, name, label, status, started_ms: id * 1000, duration_ms: status === 'running' ? null : 500,
  result_count: null, error_code: null, usage_total_tokens: null, usage: null, model_first_text_ms: null, model_name: null,
  model_name_source: null, accepted_evidence_count: null, evidence_refs: [],
});
const initial = [
  activity(1, 'model', 'outer', '生成回答'),
  activity(2, 'commentary', 'agent.commentary', '我会先读取 2.1 节，找出各个部分的职责和它们之间的联系，再用演示把这套架构串起来。'),
  activity(3, 'tool', 'book.structure', '查看书籍结构'),
  activity(4, 'tool', 'book.read', '读取 2.1 节原文'),
  activity(5, 'commentary', 'agent.commentary', '该节原文已读取，演示方式也已选定。接下来会制作可逐步查看的架构演示，并检查它与原文是否对应。'),
  activity(6, 'tool', 'presentation.methods', '查看可用的演示方式'),
  activity(7, 'tool', 'presentation.create', '制作架构演示', 'running'),
];
const answer = '可以先把 2.1 节的整体架构理解为三个相互配合的部分：**入口接收请求，核心模块协调工作，输出层返回结果。**\n\n沿着一次请求的路径看这三个部分，会比逐个记忆模块名称更容易理解。';
const outcome: OuterOutcome = { answer, answer_view: { parts: [{ kind: 'markdown', text: answer }], sources: [] }, effects: [], trace: [], memory_updates: [], incomplete: false, warning: null, turns: 1, tokens_spent: 0, profile_usage: { snapshot_revision: 0, injected_fact_ids: [], claimed_used_fact_ids: [], influences: [] } };

createApp(defineComponent({
  setup() {
    const state = ref<'running' | 'stopping' | 'completed' | 'cancelled'>('running');
    const input = ref('演示里也帮我标出请求流向');
    const fullscreen = ref(false);
    let stopTimer: ReturnType<typeof setTimeout> | undefined;
    function setState(next: typeof state.value) { clearTimeout(stopTimer); state.value = next; }
    function stop() { setState('stopping'); stopTimer = setTimeout(() => { state.value = 'cancelled'; }, 1200); }
    const controls = () => h('nav', { class: 'work-preview-controls', 'aria-label': '预览状态' }, [
      h('strong', '工作过程预览'),
      h('button', { onClick: () => setState('running') }, '运行中'),
      h('button', { onClick: () => setState('completed') }, '完成'),
      h('button', { onClick: stop }, '模拟停止'),
      !embedded && h('a', { href: mobile ? '/agent-work-visual.html' : '/agent-work-visual.html?mobile=1' }, mobile ? '桌面' : '390 px 手机'),
    ]);
    return () => mobile && !embedded
      ? h('main', { class: 'work-preview-mobile' }, [h('a', { href: '/agent-work-visual.html' }, '返回桌面预览'), h('iframe', { src: '/agent-work-visual.html?embedded=1', title: '390 像素手机工作过程' })])
      : h('main', { class: 'work-preview' }, [controls(), h(RightRail, {
        chat: [{ turnId: 'work-preview', user: '先帮我建立一个 2.1 节的整体架构的心智模型吧，我希望给我演示页啊',
          pending: state.value === 'running' || state.value === 'stopping', outcome: state.value === 'completed' ? outcome : null,
          questionAnchorLid: null, questionQuote: null, questionSelection: null, effectLabels: [],
          runStatus: { running: '正在制作演示', stopping: '正在停止，等待当前请求退出', completed: '', cancelled: '已停止' }[state.value],
          activities: initial.map(item => item.status === 'running' && state.value !== 'running' && state.value !== 'stopping' ? { ...item, status: state.value === 'completed' ? 'succeeded' as const : 'cancelled' as const, duration_ms: 7800 } : item),
        }],
        chatSessions: [], activeChatSessionId: 'work-preview', agentInput: input.value,
        sending: state.value === 'running' || state.value === 'stopping', canStop: state.value === 'running', stopping: state.value === 'stopping', fullscreen: fullscreen.value,
        selectedLid: null, selectedFormula: null, contextNotes: [], contextHighlights: [], renderMarkdown,
        effLabel: () => '', effState: () => undefined, isGoto: () => false, showEffectPrimary: () => false, showEffectSecondary: () => false,
        effectPrimaryLabel: () => '', effectSecondaryLabel: () => '', gotoBack: () => '', askDraft: null,
        'onUpdate:agentInput': (value: string) => { input.value = value; }, onStopAgent: stop,
        onToggleFullscreen: () => { fullscreen.value = !fullscreen.value; },
      })]);
  },
})).mount('#app');

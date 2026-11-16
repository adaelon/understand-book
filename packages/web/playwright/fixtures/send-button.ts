import { createApp, h, ref } from 'vue';
import RightRail from '../../src/components/RightRail.vue';
import { network } from '../../src/network-context';
import { renderMarkdown } from '../../src/md';

export function openConversation(fullscreen = false, running = false) {
  network.value = { ...network.value, enabled: false };
  const host = document.createElement('div');
  host.style.cssText = 'position:absolute;inset:0;background:var(--canvas);display:flex;justify-content:flex-end';
  document.querySelector('.application-content')!.append(host);
  const draft = ref('我又加了些东西，船往下沉了一点，可是还浮着。它都变重了，为什么没有一直沉到底呀？');
  const sends = ref(0);
  const stops = ref(0);
  createApp({ render: () => h(RightRail, {
    style: `width:min(100%, ${running ? 380 : 900}px);height:100%`, fullscreen,
    chat: running ? [{ turnId: 'running-answer', user: '围绕当前音乐练一练', pending: true, outcome: null,
      questionAnchorLid: null, questionQuote: null, questionSelection: null, effectLabels: [], runStatus: '生成回答',
      activities: Array.from({ length: 18 }, (_, i) => ({ step_id: `read-${i}`, parent_step_id: null,
        kind: 'tool' as const, name: 'music.read', label: '查看音乐与乐谱', status: 'succeeded' as const,
        result_count: 1, duration_ms: 100, usage_total_tokens: null, error_code: null })),
    }] : [], chatSessions: [], activeChatSessionId: 'send-button', agentInput: draft.value,
    sending: running, canStop: running, askDraft: null, selectedLid: null, selectedFormula: null,
    contextNotes: [], contextHighlights: [], renderMarkdown,
    effLabel: () => '', effState: () => undefined, isGoto: () => false,
    showEffectPrimary: () => false, showEffectSecondary: () => false,
    effectPrimaryLabel: () => '', effectSecondaryLabel: () => '', gotoBack: () => '',
    'onUpdate:agentInput': (value: string) => { draft.value = value; },
    onSendAgent: () => { host.dataset.sends = String(++sends.value); },
    onStopAgent: () => { host.dataset.stops = String(++stops.value); },
  }, { 'tutor-control': () => h('span', '连续教学') }) }).mount(host);
  host.dataset.sends = '0';
  host.dataset.stops = '0';
}

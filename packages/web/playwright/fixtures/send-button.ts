import { createApp, h, ref } from 'vue';
import RightRail from '../../src/components/RightRail.vue';
import { network } from '../../src/network-context';
import { renderMarkdown } from '../../src/md';

export function openConversation(fullscreen = false) {
  network.value = { ...network.value, enabled: false };
  const host = document.createElement('div');
  host.style.cssText = 'position:absolute;inset:0;background:var(--canvas);display:flex;justify-content:flex-end';
  document.querySelector('.application-content')!.append(host);
  const draft = ref('我又加了些东西，船往下沉了一点，可是还浮着。它都变重了，为什么没有一直沉到底呀？');
  const sends = ref(0);
  createApp({ render: () => h(RightRail, {
    style: 'width:min(100%, 900px);height:100%', fullscreen,
    chat: [], chatSessions: [], activeChatSessionId: 'send-button', agentInput: draft.value,
    sending: false, askDraft: null, selectedLid: null, selectedFormula: null,
    contextNotes: [], contextHighlights: [], renderMarkdown,
    effLabel: () => '', effState: () => undefined, isGoto: () => false,
    showEffectPrimary: () => false, showEffectSecondary: () => false,
    effectPrimaryLabel: () => '', effectSecondaryLabel: () => '', gotoBack: () => '',
    'onUpdate:agentInput': (value: string) => { draft.value = value; },
    onSendAgent: () => { host.dataset.sends = String(++sends.value); },
  }, { 'tutor-control': () => h('span', '连续教学') }) }).mount(host);
  host.dataset.sends = '0';
}

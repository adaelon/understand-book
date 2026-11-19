import { createApp, h, ref } from 'vue';
import MessageThoughts from '@understand-book/web/reader/components/MessageThoughts.vue';
import BoardWorkspace from '@understand-book/web/reader/components/BoardWorkspace.vue';
import AgentPresentation from '@understand-book/web/reader/components/AgentPresentation.vue';
import { network } from '@understand-book/web/reader/reader-context';
import { boardFixtureReceipt } from '@understand-book/web/reader/fixtures/board-workspace';

export function coverWithBoard() {
  // App keeps its reader and conversation mounted beneath the board overlay.
  const host = document.createElement('div');
  host.className = 'board-workspace-overlay';
  host.style.cssText = 'position:absolute;inset:0;z-index:140;background:#f8f6ef;display:flex';
  document.querySelector('.application-content')!.append(host);
  createApp({render:()=>h(BoardWorkspace,{boardId:'board-probability',style:'flex:1;min-height:0'})}).mount(host);
}

export function openPlacementScene(initial = '对话') {
  // Use the ordinary HTTP transport without requiring a mounted ReaderWorkspace.
  network.value = {...network.value, enabled:false};
  const host = document.createElement('div');
  host.style.cssText='position:absolute;inset:0;background:#f8f6ef;z-index:15;overflow:auto';
  document.querySelector('.application-content')!.append(host);
  const tab = ref(initial);
  createApp({render:()=>h('div',{'data-placement-scene':'',style:'height:100%;display:flex;flex-direction:column'},[
    h('header',{style:'display:flex;gap:24px;align-items:center;padding:20px 32px;border-bottom:1px solid #e3dccd'},[
      h('strong',{style:'margin-right:auto;font-size:17px;white-space:nowrap'},'概率与判断'),...['对话','画布','演示'].map(name=>h('button',{style:'border:0;background:none;font-size:14px;color:#836b4d;cursor:pointer;white-space:nowrap',onClick:()=>tab.value=name},name)),
    ]),
    tab.value==='对话'?h('article',{style:'width:min(720px,calc(100% - 48px));margin:40px auto;font-size:16px;line-height:1.9;color:#49443b'},[
      h('small',{style:'color:#9d8c73'},'关于先验与证据的讨论'),
      h('h2',{style:'font-size:25px;font-weight:500'},'为什么同一份证据会有不同的含义？'),
      h(MessageThoughts,{sessionId:'chat-original',turnId:'turn-original',role:'assistant',text:'先验会影响后验。\n\n先验会影响后验。看证据时，也要考虑原来的发生比例。'},
        {default:()=>[h('p',{'data-paragraph':'first'},'先验会影响后验。'),h('p',{'data-paragraph':'second'},'先验会影响后验。看证据时，也要考虑原来的发生比例。')]}),
      h('p',{style:'margin-top:32px;color:#8a8173'},'不妨换一个人群，再检查这个判断。'),
    ]):tab.value==='画布'?h(BoardWorkspace,{boardId:'board-probability',style:'flex:1;min-height:0'}):
      h('div',{style:'width:min(980px,calc(100% - 40px));margin:24px auto'},[h(AgentPresentation,{sessionId:'chat-original',turnId:'turn-original',reference:boardFixtureReceipt.reference})]),
  ])}).mount(host);
}

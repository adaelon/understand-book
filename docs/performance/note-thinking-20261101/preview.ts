import { createApp, h } from 'vue';
import './style.css';
import PersonalNotes from './components/PersonalNotes.vue';
import UnderstandingSpace from './components/UnderstandingSpace.vue';
import { openPersonalNotes } from './personal-notes';
const fixture=await fetch('/__thinking_fixture').then(r=>r.json());
const overview=new URLSearchParams(location.search).has('overview');
createApp({render:()=>h('main',{style:'min-height:100vh;background:#f4f1e9;padding:32px;box-sizing:border-box;font-family:system-ui;color:#39352e;'},[
  h('div',{style:'max-width:760px;margin:0 auto;'},[h('small','UNDERSTAND BOOK'),h('h1',{style:'font-size:23px;margin:12px 0 20px'},'从一个想法，看见理解的变化'),overview?h(UnderstandingSpace):h('p','笔记保留原话，理解说明依据真实讨论逐步更新。')]),h(PersonalNotes),
])}).mount('#preview');
if(!overview)openPersonalNotes({note_id:fixture.note.note_id});

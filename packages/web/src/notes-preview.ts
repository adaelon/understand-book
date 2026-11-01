// Isolated visual fixture, reachable only through notes-preview.html.
// These in-memory notes are never written to a real learning workspace.
import { createApp } from 'vue';
import NotesWorkspace from './components/NotesWorkspace.vue';
import { api } from './api';
import type { WorkspaceNote } from './generated/WorkspaceNote';
import type { WorkspaceNoteRelation } from './generated/WorkspaceNoteRelation';
import './style.css';
import 'katex/dist/katex.min.css';

const now = '2026-10-27T09:00:00Z';
const example = (id: string, title: string, content: string, author: WorkspaceNote['author'] = 'user'): WorkspaceNote => ({
  note_id: id, learning_workspace_id: 'notes-preview', title, content, revision: 1,
  created_at: now, updated_at: now, author, updated_by: author, origin: null, source_bindings: [],
});
let notes: WorkspaceNote[] = [
  example('practice', '练习的意义，是让下一次有所不同', '重复一个动作，会让它越来越熟悉。但熟悉不一定意味着，我已经知道自己哪里需要调整。\n\n## 一个值得留下的区别\n\n有效的练习，需要一个可以观察的目标：这次想解决什么，实际发生了什么，下一次准备改变什么。\n\n例如，练琴时把注意力放在某一处换弓，记录两次尝试的差别，比笼统地要求自己“再拉好一点”更容易继续。\n\n> 我想保留的不是一次练习的分数，而是下一次可以接着做的线索。\n\n还想继续追问：怎样选择一个足够小、又确实重要的练习目标？'),
  example('feedback', '反馈需要指向具体的表现', '“还不够好”很难指导下一次尝试。\n\n反馈如果指向一个可定位的现象，就能帮助学习者比较变化，并判断调整有没有作用。\n\n这份解释来自一次讨论，可以继续用自己的经验补充。', 'assistant'),
  example('recording', '给每次尝试留一小段记录', '保留短录音，也记下当时关注的问题。\n\n下次回看时，既能听见变化，也能知道为什么要比较这两次尝试。'),
  example('questions', '看懂以后，怎样确认自己真的理解了？', '尝试离开原文，用自己的话解释一遍。\n\n解释中接不上的地方，可以成为下一次阅读和讨论的入口。'),
];
notes[1].origin = { kind: 'answer', book_id: 'book-preview', publication_id: null, session_id: 'session-preview', turn_id: 'turn-preview', mem_id: null };
notes[1].source_bindings = [{ source_ref_id: 'source-preview', book_id: 'book-preview', evidence_text_digest: '', label_snapshot: '学习讨论 · 反馈与练习', preview_snapshot: '把关注点缩小到一个具体的现象，才有机会比较前后的变化。' }];
let relations: WorkspaceNoteRelation[] = [
  { relation_id: 'first-link', from_note_id: 'practice', to_note_id: 'feedback', label: '需要具体反馈', reason: '明确这一次发生了什么，才能决定下一次怎样调整。', revision: 1, author: 'user', created_at: now, updated_at: now },
  { relation_id: 'second-link', from_note_id: 'recording', to_note_id: 'practice', label: '提供比较依据', reason: '留下短记录，可以让两次练习之间的变化变得可观察。', revision: 1, author: 'assistant', created_at: now, updated_at: now },
];
const params = new URLSearchParams(location.search);
if (params.get('state') === 'empty') { notes = []; relations = []; }
function read(id: string) { const note = notes.find(item => item.note_id === id); if (!note) throw new Error('笔记不存在'); return structuredClone(note); }
api.workspaceNotes = async () => structuredClone({ notes, relations });
api.workspaceNote = async id => read(id);
api.workspaceNoteCreate = async body => { const note = example(crypto.randomUUID(), body.title, body.content); notes.unshift(note); return structuredClone(note); };
api.workspaceNoteUpdate = async body => {
  const note = read(body.note_id); if (body.expected_revision !== note.revision) throw new Error('笔记已经更新');
  Object.assign(note, { title: body.title, content: body.content, revision: note.revision + 1, updated_by: 'user' });
  notes = notes.map(item => item.note_id === note.note_id ? note : item); return structuredClone(note);
};
api.workspaceNoteDelete = async body => { notes = notes.filter(note => note.note_id !== body.note_id); relations = relations.filter(relation => relation.from_note_id !== body.note_id && relation.to_note_id !== body.note_id); return { deleted: true }; };
api.workspaceNoteRelationSave = async body => {
  const relation: WorkspaceNoteRelation = { ...body, relation_id: body.relation_id ?? crypto.randomUUID(), revision: (body.expected_revision ?? 0) + 1, author: 'user', created_at: now, updated_at: now };
  relations = relations.filter(item => item.relation_id !== relation.relation_id); relations.push(relation); return structuredClone(relation);
};
api.workspaceNoteRelationDelete = async body => { relations = relations.filter(item => item.relation_id !== body.relation_id); return { deleted: true }; };
createApp(NotesWorkspace, { learningWorkspaceId: 'notes-preview', initialNoteId: params.get('state') === 'empty' ? null : 'practice', readonly: params.has('readonly') }).mount('#app');

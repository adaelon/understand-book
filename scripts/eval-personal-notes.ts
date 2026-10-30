import {mkdirSync,writeFileSync} from 'node:fs';
import path from 'node:path';
import {performance} from 'node:perf_hooks';
import {PersonalNoteSearch} from '../packages/core/src/personal-note-search';

const count=Number(process.env.PN_NOTE_COUNT ?? 100);
const root=path.resolve(process.env.PN_RETRIEVAL_OUTPUT ?? '.tmp/pn-real-retrieval');mkdirSync(root,{recursive:true});
process.env.UNDERSTAND_BOOK_EMBEDDING_CONFIG=path.resolve('.tmp/pn-minilm/config.json');
const seeds=[
  ['weights','推理时参数固定，输入不同会产生不同激活；上下文里的适应不能直接等同于模型更新权重。'],
  ['speed','我以前直接把两段速度相加再除二，后来发现应该用总路程除以总时间，停留也占用时间。'],
  ['probability','独立事件和互斥事件不同：互斥表示不能同时发生，独立表示知道一个发生不改变另一个发生的概率。'],
  ['cache','缓存像一张便签，省去重复计算；内容更新后必须失效，否则会继续拿旧答案。'],
  ['derivative','导数反映一个时刻的变化快慢，积分把一段时间里微小变化积累起来。'],
  ['sampling','样本再多也可能有选择偏差，增加数量不能纠正抽样方式带来的系统性偏差。'],
  ['tail','我先记录实验过程。'.repeat(300)+'\n最后的修正：梯度累积让小显存模拟较大的有效批次，但 BatchNorm 的每批统计量不会自动等同于大批次。'],
];
const unrelated=['周末沿河散步，看了白鹭筑巢。','番茄炖牛腩需要先煎香肉块再慢慢炖。','整理旅行照片，傍晚的光线比较柔和。','给阳台绿植浇水之前先摸一摸土壤。','练习钢琴时放慢速度，留意手腕放松。','今天读诗注意到句子停顿改变情绪。'];
const notes=seeds.map(([note_id,text])=>({note_id,revision:1,text}));
for(let n=0;n<count-seeds.length;n++)notes.push({note_id:`daily-${n}`,revision:1,text:`第 ${n+1} 天的日常记录。${unrelated[n%unrelated.length]}`});
const engine=new PersonalNoteSearch();
const base={kind:'personal_notes' as const,cache_dir:path.join(root,'index'),snapshot_revision:1,notes};
const start=performance.now();await engine.search(base);await engine.settled();const cold=performance.now()-start;
const queries=[['weights','为什么同一个模型换一个提示词就表现得不一样，它真的重新训练了吗？'],['speed','来回路上的平均快慢，要不要算上中途休息？'],['probability','不可能一起出现，是否代表彼此没有影响？'],['cache','资料改了之后系统怎么还在返回原来的结果？'],['tail','显存不够时把几次反向传播合起来，跟一次大批训练有什么差别？']];
const results=[];for(const [expected,query] of queries){const t=performance.now();const result=await engine.search({...base,query,top_k:8});results.push({expected,query,ms:performance.now()-t,...result,found:result.hits.slice(0,3).some(hit=>hit.note_id===expected)});}
const before=results.at(-1)!.embedded_chunks;
const edited=notes.map(note=>note.note_id==='weights'?{...note,revision:2,text:'补充：上下文改变激活与注意力，不等于训练更新参数。'}:note);
await engine.search({...base,snapshot_revision:2,notes:edited});await engine.settled();
const changed=await engine.search({...base,snapshot_revision:2,notes:edited,query:queries[0][1]});
await engine.dispose();const reopened=new PersonalNoteSearch();const r=performance.now();await reopened.search({...base,snapshot_revision:2,notes:edited});await reopened.settled();const reopen=await reopened.search({...base,snapshot_revision:2,notes:edited,query:queries[0][1]});await reopened.dispose();
const report={model:'Xenova/paraphrase-multilingual-MiniLM-L12-v2',notes:notes.length,cold_ms:cold,queries:results,update_embedded_chunks:changed.embedded_chunks-before,updated_hit:changed.hits.find(hit=>hit.note_id==='weights'),reopen_ms:performance.now()-r,reopen,all_top3:results.every(row=>row.found)};
writeFileSync(path.join(root,'report.json'),JSON.stringify(report,null,2));console.log(JSON.stringify({notes:notes.length,cold_ms:cold,all_top3:report.all_top3,query_ms:results.map(r=>r.ms),updated_hit:report.updated_hit,reopen_cache_hits:reopen.cache_hits}));

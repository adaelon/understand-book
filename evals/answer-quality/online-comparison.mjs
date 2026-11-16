// Import public observations from the deployed Reader and compare with Codex.
// No Reader answer, assessment text or post-run score audit enters Codex input.
import fs from 'node:fs';
import path from 'node:path';
import { candidatePrompt, observedMaterials, version, completionState } from './core.mjs';
import { runCodex } from '../tool-usability/codex-client.mjs';

const argv=process.argv.slice(2);
const arg=(name,fallback)=>argv.includes(name)?argv[argv.indexOf(name)+1]:fallback;
const out=path.resolve(arg('--out','.understand-book/answer-quality/production-parity-20261111'));
const selected=arg('--only','M01,M06').split(',');
const dataset=JSON.parse(fs.readFileSync('evals/music/answer-quality-cases.json','utf8'));
const configuration=JSON.parse(fs.readFileSync(path.join(out,'configuration.json'),'utf8'));
const baselineFile=path.resolve('.understand-book/answer-quality/music-20261111-main/run.json');
const baseline=JSON.parse(fs.readFileSync(baselineFile,'utf8'));
const write=(file,data)=>fs.writeFileSync(file,JSON.stringify(data,null,2)+'\n');
const runFile=path.join(out,'comparison.json');
const run=fs.existsSync(runFile)?JSON.parse(fs.readFileSync(runFile,'utf8')):{
  version,dataset_version:dataset.version,status:'running',started_at:new Date().toISOString(),
  configuration:{...configuration,baseline_model:'gpt-6-astra',baseline_effort:'medium',baseline_transport:'codex_limited_tools',
    reader_interface:'deployed_multi_user_browser',comparison_design:'online_product_vs_codex; different system prompts and tool workflows'},
  publication:configuration.publication,cases:[],samples:[]};
function report() {
  const lines=['# 产品路径 DeepSeek 与 Codex Astra：回答全文','',
    '口径：DeepSeek 通过真实 Reader 网页与产品模型接口运行；Astra 使用 Codex。两路系统提示、工具流程和推理配置不同，比较实际交付与解释质量。','',
    `测试程序：${configuration.release}。材料版本：${configuration.publication.publication_id}。`,
    ...(configuration.candidate_base_release ? [`候选基于线上版本 ${configuration.candidate_base_release}，冻结改动见 configuration.json 的 candidate_files；生产服务没有替换。`] : []),
    'DeepSeek：deepseek-v4-flash，官方 Messages 接口，high 推理，单次输出上限 131,072，保留线上重试规则，无额外累计 Token 上限。',
    'Astra：gpt-6-astra，medium 推理；实际客户端版本见每个新样本的 client.json，复用样本沿用原运行版本。有 Codex 系统上下文，关闭 shell、网页搜索、插件和记忆，仍可能查询资源目录。',
    '每题每路一次。DeepSeek 在独立账号的新对话中从网页发送原题，保留默认播放光标；不是用户历史会话回放。同材料题给 Astra 的是产品实际工具返回，没有产品最终答案、评审要求或参考事实。','',
    '| 题目 | 路径 | 返回状态 | 用时（秒） | Token |', '| --- | --- | --- | ---: | ---: |'];
  for(const row of run.samples)lines.push(`| ${row.case_id} | ${row.arm} | ${row.status} | ${row.elapsed_ms==null?'未知':(row.elapsed_ms/1000).toFixed(1)} | ${row.usage?.total_tokens??'未知'} |`);
  lines.push('','返回正文不代表回答正确或满足需求。耗时使用产品服务端记录；重连采集的浏览器等待时间不作为模型生成时长。','');
  for(const task of run.cases){
    lines.push(`## ${task.id} · ${task.title}`,'',task.question,'');
    for(const row of run.samples.filter(s=>s.case_id===task.id)){
      lines.push(`### ${row.arm} · ${row.status}`,'',row.answer||'无正文。','');
      if(row.warning)lines.push(`停止原因：${row.warning}。`,'');
      if(row.reused_from)lines.push('此答案复用此前相同模型、推理设置及完全相同输入的已有样本，不计为新增独立重复。','');
      if(row.recovered_observation)lines.push('此样本曾因评测脚本的工作区版本过期而中断观察，随后恢复原会话读取最终结果，没有重新提交问题；原采集错误记录保留。','');
    }
  }
  return lines.join('\n');
}
const persist=()=>{
  run.cases.sort((a,b)=>a.id.localeCompare(b.id));
  run.samples.sort((a,b)=>a.case_id.localeCompare(b.case_id)||(a.arm==='product'?-1:1));
  write(runFile,run);fs.writeFileSync(path.join(out,'answers.md'),report());
};
for (const task of dataset.cases.filter(t=>selected.includes(t.id))) {
  const directory=path.join(out,'online-samples',task.id),productDirectory=path.join(directory,'product');
  const product=JSON.parse(fs.readFileSync(path.join(productDirectory,'sample.json'),'utf8'));
  const records=JSON.parse(fs.readFileSync(path.join(productDirectory,'provider.json'),'utf8'));
  const observations=JSON.parse(fs.readFileSync(path.join(productDirectory,'observations.json'),'utf8'));
  if (!run.cases.some(t=>t.id===task.id))run.cases.push(task);
  const complete=records.length>0&&records.every(r=>Number.isFinite(r.usage?.total_tokens));
  product.usage={requests:records.length,total_tokens:complete?records.reduce((n,r)=>n+r.usage.total_tokens,0):null,missing_usage:records.filter(r=>!Number.isFinite(r.usage?.total_tokens)).length};
  const root=observations.find(o=>o.extra?.metadata?.ub_observation?.kind==='run');
  if(root?.start_time&&root?.end_time)product.server_elapsed_ms=Date.parse(root.end_time)-Date.parse(root.start_time);
  product.elapsed_ms=product.server_elapsed_ms??product.elapsed_ms;
  product.tools=observations.filter(o=>o.extra?.metadata?.ub_observation?.kind==='tool').map(o=>({name:o.name,error_code:o.extra.metadata.ub_observation.metadata?.error_code}));
  product.condition=configuration.candidate_base_release?'repaired_product_materials_and_tools':'deployed_product_materials_and_tools';
  const materials=observedMaterials(records);
  write(path.join(productDirectory,'materials.json'),materials);
  run.samples=run.samples.filter(s=>s.case_id!==task.id||s.arm!=='product');run.samples.push(product);persist();
  // Refresh exported Reader evidence on re-import without sampling Astra again.
  if (run.samples.some(t=>t.case_id===task.id && t.arm==='astra'))continue;
  const astraDirectory=path.join(directory,'astra');fs.mkdirSync(astraDirectory,{recursive:true});
  const prompt=candidatePrompt(task,task.baseline_mode==='same_material'?materials:[]);
  const prior=baseline.samples.find(s=>s.case_id===task.id&&s.arm==='astra'&&s.status==='delivered');
  let priorPrompt=path.join(path.dirname(baselineFile),task.id,'astra','prompt.txt');
  if(!fs.existsSync(priorPrompt)&&prior?.reused_from)priorPrompt=path.join(path.dirname(prior.reused_from),task.id,'astra','prompt.txt');
  let astra;
  if(prior&&fs.existsSync(priorPrompt)&&fs.readFileSync(priorPrompt,'utf8')===prompt){
    astra={...prior,reused_from:baselineFile,condition:task.baseline_mode};
    fs.writeFileSync(path.join(astraDirectory,'prompt.txt'),prompt);
    console.log(JSON.stringify({case_id:task.id,arm:'astra',status:'reused_exact_input'}));
  }else{
    if(task.baseline_mode==='same_material'&&!materials.length)throw new Error('No actual tool material available');
    console.log(JSON.stringify({case_id:task.id,arm:'astra',status:'running',material_count:materials.length}));
    const result=await runCodex({directory:astraDirectory,prompt,model:'gpt-6-astra',effort:'medium',executable:arg('--codex',process.env.TO14_CODEX),timeoutMs:300000});
    astra={case_id:task.id,arm:'astra',condition:task.baseline_mode,answer:result.final??'',status:completionState({answer:result.final,incomplete:!result.completed}),
      elapsed_ms:result.elapsed_ms,usage:result.usage?{...result.usage,total_tokens:result.total_tokens}:null,timed_out:result.timed_out,exit_code:result.exit_code};
    console.log(JSON.stringify({case_id:task.id,arm:'astra',status:astra.status,elapsed_ms:astra.elapsed_ms}));
  }
  fs.writeFileSync(path.join(astraDirectory,'answer.md'),astra.answer);write(path.join(astraDirectory,'sample.json'),astra);
  run.samples.push(astra);persist();
}
run.status=run.samples.every(s=>s.status==='delivered')?'completed':'completed_with_failures';run.finished_at=new Date().toISOString();persist();

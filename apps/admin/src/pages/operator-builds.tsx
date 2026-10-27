import { useEffect, useRef, useState } from 'react';
import { DataState, DataTable, Pagination, useData } from '@/components/data';
import { Field, OperationDialog, useOperations } from '@/components/operation';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { date, money, moneyInput } from '@/lib/format';
import type { BookRef, Charge, Page } from '@/lib/types';
import { bookKey, bookLabel, type ReleaseMaterial } from '@/lib/workspace-releases';
import { buildState, type OperatorBuild, type OperatorMaterial } from '@/lib/operator-builds';
import { session } from '@/lib/api';
import { ReleaseEditor } from './workspace-releases';
import { Amount, Distribution, Metric, Notice, StatusBadge, ViewSwitch } from '@/components/analytics';

const activeBuild = (state:string) => ['queued_prepare','preparing','queued','running'].includes(state);
const needsAttention = (state:string) => ['draft','awaiting_confirmation','interrupted','paused'].includes(state);
const buildTone = (state:string) => needsAttention(state) ? 'amber' as const : activeBuild(state) ? 'blue' as const : ['ready','published'].includes(state) ? 'green' as const : 'muted' as const;

export function OperatorBuildsPage() {
  const [offset,setOffset]=useState(0),[selected,setSelected]=useState<string|null>(null),[create,setCreate]=useState(false);
  const [filter,setFilter]=useState<'all'|'attention'|'active'|'ready'>('all');
  const query=useData<Page<OperatorBuild>>(`/admin/operator-builds?limit=20&offset=${offset}`);
  const ops=useOperations();
  return <section className="space-y-5"><div className="flex flex-wrap items-center justify-between gap-3"><h1>运营资料构建</h1><div className="flex gap-2"><Button variant="outline" onClick={()=>void query.refetch()}>刷新任务</Button><Button disabled={ops.blocked} onClick={()=>setCreate(true)}>创建准备任务</Button></div></div>
    <p className="text-sm text-muted-foreground">查看准备进度、处理待确认任务，并将就绪成果发布为共享工作区。</p>
    <DataState query={query}/>{query.data&&<><div className="analytics-panel"><Distribution label="本页构建状态" segments={[
      {label:'需要操作',value:query.data.items.filter(t=>needsAttention(t.state)).length,tone:'amber'},
      {label:'排队与构建中',value:query.data.items.filter(t=>activeBuild(t.state)).length,tone:'blue'},
      {label:'已就绪 / 已发布',value:query.data.items.filter(t=>['ready','published'].includes(t.state)).length,tone:'green'},
    ]} note={`共 ${query.data.total} 个任务，色条与筛选仅统计当前页。`} /><ViewSwitch label="筛选本页构建任务" value={filter} options={[["all","本页全部"],["attention","需要操作"],["active","构建中"],["ready","成果已就绪"]]} onChange={setFilter}/></div><DataTable rows={query.data.items.filter(t=>filter==='all'||(filter==='attention'?needsAttention(t.state):filter==='active'?activeBuild(t.state):['ready','published'].includes(t.state)))} columns={[
      {header:'任务',cell:({row})=><strong>{row.original.name}</strong>},
      {header:'状态',cell:({row})=><StatusBadge tone={buildTone(row.original.state)}>{buildState(row.original.state)}</StatusBadge>},
      {header:'平台预算上限',cell:({row})=><Amount value={row.original.max_micro_cny} />},
      {header:'更新时间',cell:({row})=>date(row.original.updated_at)},
      {header:'操作',cell:({row})=><Button variant="ghost" onClick={()=>setSelected(row.original.task_id)}>查看构建任务</Button>},
    ]}/><Pagination offset={offset} total={query.data.total} change={setOffset}/></>}
    {selected&&<BuildDetail key={selected} taskId={selected} close={()=>setSelected(null)}/>}{create&&<CreateBuild close={()=>setCreate(false)} created={setSelected}/>}
  </section>;
}
interface SourceInput {label:string;target_input:string;root_dir:string;material_kind:'book'|'video'|'music';build_tutor:boolean;pass2:'enabled'|'disabled';max_parallel:number}
const emptySource=():SourceInput=>({label:'',target_input:'',root_dir:'',material_kind:'book',build_tutor:false,pass2:'disabled',max_parallel:1});
function CreateBuild({close,created}:{close:()=>void;created:(taskId:string)=>void}) {
  const [bookOffset,setBookOffset]=useState(0),[reused,setReused]=useState<ReleaseMaterial[]>([]),[sources,setSources]=useState<SourceInput[]>([emptySource()]);
  const books=useData<{books:{published_book_ref:BookRef;material_kind?:'book'|'video'|'music'}[];total:number}>(`/admin/books?limit=20&offset=${bookOffset}`);
  const ops=useOperations();
  const activeSources=sources.filter(s=>s.label.trim()||s.target_input.trim()||s.root_dir.trim());
  const startMedia=activeSources.length>0&&activeSources.every(s=>s.material_kind!=='book');
  return <OperationDialog title="创建准备任务" submitLabel={startMedia?'创建并开始构建':'创建准备任务'} description="选择已有成果或填写新资料的位置。资料路径需要构建宿主与服务均可访问；视频与精选音乐可直接启动，音乐来源填写已审核的精选资料清单位置。" open onOpenChange={v=>{if(!v)close();}} onSubmit={async data=>{
    const max=moneyInput(String(data.get('max'))),reserved=moneyInput(String(data.get('reserved')));
    if(max<=0||reserved<=0||reserved>max)throw new Error('请填写正数预算，单次预计占用不能超过总预算。');
    const inputs=sources.filter(s=>s.label.trim()||s.target_input.trim()||s.root_dir.trim());
    if(!reused.length&&!inputs.length)throw new Error('请至少选择一份资料。');
    if(inputs.some(s=>!s.label.trim()||!s.target_input.trim()||!s.root_dir.trim()))throw new Error('请补全待准备资料的名称、来源位置与成果根目录。');
    const taskId=crypto.randomUUID();
    const saved=await ops.submit('创建运营构建','/admin/operator-builds',{task_id:taskId,name:String(data.get('name')).trim(),max_micro_cny:max,reserved_per_execution_micro_cny:reserved,
      visible_to_users:[...new Set(String(data.get('visible_to_users')).split(/[\s,，]+/).filter(Boolean))],
      materials:[...reused.map((material,i)=>({material_id:`reuse-${i+1}`,label:bookLabel(material.published_book_ref),...material})),...inputs.map((s,i)=>({material_id:`source-${i+1}`,...s,required_capabilities:s.material_kind==='music'?['music','music_score','music_events']:s.material_kind==='video'?['text','video','translation','visual_index','graph','discourse']:['text','tutor']}))]});
    if(!saved)return false;
    created(taskId);
    if(startMedia){
      // Read the saved revision so a denied start leaves an ordinary resumable task.
      const task=await session.request<OperatorBuild>(`/admin/operator-builds/${encodeURIComponent(taskId)}`);
      return ops.submit(inputs.every(s=>s.material_kind==='video')?'开始视频构建':'开始资料构建',`/admin/operator-builds/${encodeURIComponent(taskId)}/start`,{expected_revision:task.revision,max_micro_cny:max});
    }
    return true;
  }}><Field label="准备任务名称"><Input name="name" required/></Field><div className="grid gap-3 sm:grid-cols-2"><Field label="平台预算上限（元）"><Input name="max" inputMode="decimal" required/></Field><Field label="每次执行预计占用（元）"><Input name="reserved" inputMode="decimal" required/></Field></div>
    <Field label="成果授权账号（空格或逗号分隔）"><Input name="visible_to_users" required/></Field><p className="text-sm text-muted-foreground">任务会将选定的复用成果和新成果授予这些账号；公共发布时仍需选择可见范围。供应商完整费用缺失时保留待核算。</p>
    <fieldset className="my-4 space-y-2 rounded-md border p-3"><legend>复用已有成果</legend><DataState query={books}/>{books.data?.books.map(({published_book_ref:book,material_kind})=><label className="flex items-center gap-2 text-sm" key={bookKey(book)}><input type="checkbox" checked={reused.some(r=>bookKey(r.published_book_ref)===bookKey(book))} onChange={e=>setReused(rows=>e.target.checked?[...rows,{published_book_ref:book,required_capabilities:material_kind==='music'?['music','music_score']:material_kind==='video'?['text','video']:['text','tutor']}]:rows.filter(r=>bookKey(r.published_book_ref)!==bookKey(book)))}/>{bookLabel(book)}</label>)}{books.data&&<Pagination offset={bookOffset} total={books.data.total} change={setBookOffset}/>}</fieldset>
    {sources.map((source,i)=><fieldset key={i} className="my-3 rounded-md border p-3"><legend>待准备资料 {i+1}</legend><Field label="资料类型"><select aria-label="资料类型" value={source.material_kind} onChange={e=>setSources(rows=>rows.map((s,n)=>n===i?{...s,material_kind:e.target.value as 'book'|'video'|'music',build_tutor:false}:s))}><option value="book">书籍</option><option value="video">视频</option><option value="music">精选音乐</option></select></Field>{([['label','资料名称'],['target_input','来源文件或工作区位置'],['root_dir','成果根目录']] as const).map(([key,label])=><Field key={key} label={label}><Input value={source[key]} onChange={e=>setSources(rows=>rows.map((s,n)=>n===i?{...s,[key]:e.target.value,...(key==='target_input'&&/\.mp4$/i.test(e.target.value.trim())?{material_kind:'video' as const,build_tutor:false}:{})}:s))}/></Field>)}{source.material_kind==='music'?<p className="my-2 text-sm text-muted-foreground">来源填写精选音乐清单（music_curation.v1）：包含录音、MusicXML、素材许可、对齐校准和音乐事件。接纳已核对内容并生成谱面与听辨素材；本地处理无模型调用费用。</p>:source.material_kind==='video'?<p className="my-2 text-sm text-muted-foreground">准备字幕、画面、知识关系和讲解结构；不预生成 Tutor 教学材料。</p>:<><label className="my-3 flex items-center gap-2 text-sm"><input type="checkbox" checked={source.build_tutor} onChange={e=>setSources(rows=>rows.map((s,n)=>n===i?{...s,build_tutor:e.target.checked}:s))}/>额外预构建 Tutor 教学材料</label><Field label="跨段关系复核"><select value={source.pass2} onChange={e=>setSources(rows=>rows.map((s,n)=>n===i?{...s,pass2:e.target.value as 'enabled'|'disabled'}:s))}><option value="disabled">关闭</option><option value="enabled">启用</option></select></Field></>}<Button type="button" variant="ghost" onClick={()=>setSources(rows=>rows.filter((_,n)=>n!==i))}>移除此待准备资料</Button></fieldset>)}
    <Button type="button" variant="outline" onClick={()=>setSources(rows=>[...rows,emptySource()])}>添加待准备资料</Button>
  </OperationDialog>;
}
function BuildDetail({taskId,close}:{taskId:string;close:()=>void}) {
  const anchor=useRef<HTMLElement>(null);
  const query=useData<OperatorBuild>(`/admin/operator-builds/${encodeURIComponent(taskId)}`);
  const [action,setAction]=useState<'prepare'|'start'|'continue'|'pause'|null>(null),[publish,setPublish]=useState(false),[receipts,setReceipts]=useState(false);
  const ops=useOperations(),task=query.data;
  useEffect(()=>{if(task){anchor.current?.focus({preventScroll:true});anchor.current?.scrollIntoView?.({block:'start'});}},[!!task]);
  const videoTask=!!task&&task.materials.some(m=>!m.published_book_ref&&m.material_kind==='video')&&task.materials.every(m=>m.published_book_ref||m.material_kind==='video');
  const mediaTask=!!task&&task.materials.some(m=>!m.published_book_ref)&&task.materials.every(m=>m.published_book_ref||m.material_kind==='video'||m.material_kind==='music');
  const mediaStartLabel=videoTask?'开始视频构建':'开始音乐构建';
  useEffect(()=>{
    if(!task||!['queued_prepare','preparing','queued','running'].includes(task.state))return;
    const timer=setInterval(()=>{void query.refetch();},2000);
    return()=>clearInterval(timer);
  },[task?.state,query.refetch]);
  const labels={prepare:'准备构建计划',start:mediaTask?mediaStartLabel:'确认计划并启动',continue:'继续构建',pause:'暂停构建'};
  return <section ref={anchor} tabIndex={-1} className="analytics-panel outline-none" aria-label="运营构建任务详情"><div className="flex flex-wrap justify-between gap-2"><h2>{task?.name??'构建任务'}</h2><div><Button variant="ghost" onClick={()=>void query.refetch()}>刷新构建状态</Button><Button variant="ghost" onClick={close}>关闭构建详情</Button></div></div><DataState query={query}/>{task&&<>
    <StatusBadge tone={buildTone(task.state)}>{buildState(task.state)}{task.pause_requested&&['running','preparing'].includes(task.state)?' · 正在等待当前执行收尾':''}</StatusBadge>{task.error&&<div role="alert"><Notice warning>{task.error.includes('BUILD_BUDGET_EXHAUSTED')?'本任务剩余预算不足，已暂停后续执行。请核对费用回执或提高预算后继续。':task.error}</Notice></div>}
    {task.state.startsWith('queued')&&<p className="text-sm">等待连接到同一服务的构建宿主。宿主启动并恢复原运营会话后会接续任务。</p>}
    <dl className="grid gap-3 sm:grid-cols-2"><Metric label="剩余可准入预算" value={task.budget?<Amount value={task.budget.remaining_micro_cny}/>: '尚未开始'} detail="可用于后续执行" tone="green"/><Metric label="平台预算上限" value={<Amount value={task.max_micro_cny}/>} detail={`精确上限 ${money(task.max_micro_cny)} 元`}/></dl>
    {task.budget&&<><Distribution label="构建预算分配" total={task.max_micro_cny} segments={[
      {label:'已确认成本',value:task.budget.provider_cost_micro_cny,display:`${money(task.budget.provider_cost_micro_cny)} 元`,tone:'blue'},
      {label:'处理成本估计',value:task.budget.estimated_cost_micro_cny??0,display:`${money(task.budget.estimated_cost_micro_cny??0)} 元`,tone:'violet'},
      {label:'待核算占用',value:task.budget.pending_micro_cny,display:`${money(task.budget.pending_micro_cny)} 元`,tone:'amber'},
      {label:'当前执行占用',value:task.budget.reserved_micro_cny,display:`${money(task.budget.reserved_micro_cny)} 元`,tone:'muted'},
      {label:'剩余预算',value:task.budget.remaining_micro_cny,display:`${money(task.budget.remaining_micro_cny)} 元`,tone:'green'},
    ]}/>{task.budget.unknown_count>0&&<Notice warning>{task.budget.unknown_count} 笔费用未完整确认，请通过费用回执核对。</Notice>}</>}
    <div className="flex flex-wrap gap-2">{task.state==='draft'&&mediaTask?<Button disabled={ops.blocked} onClick={()=>setAction('start')}>{mediaStartLabel}</Button>:(task.state==='draft'||(task.state==='interrupted'&&!mediaTask&&task.materials.some(m=>!m.published_book_ref&&!m.plan)))&&<Button disabled={ops.blocked} onClick={()=>setAction('prepare')}>准备构建计划</Button>}{task.state==='awaiting_confirmation'&&<Button disabled={ops.blocked} onClick={()=>setAction('start')}>{labels.start}</Button>}{['interrupted','paused'].includes(task.state)&&(mediaTask||task.materials.every(m=>m.published_book_ref||m.plan))&&<Button disabled={ops.blocked} onClick={()=>setAction('continue')}>继续构建</Button>}{['running','preparing','queued','queued_prepare'].includes(task.state)&&<Button disabled={ops.blocked||task.pause_requested} variant="outline" onClick={()=>setAction('pause')}>暂停构建</Button>}{['ready','published'].includes(task.state)&&<Button disabled={ops.blocked} onClick={()=>setPublish(true)}>选择成果并发布</Button>}{task.budget&&<Button variant="outline" onClick={()=>setReceipts(v=>!v)}>{receipts?'收起费用回执':'查看费用回执'}</Button>}</div>
    {task.state==='interrupted'&&<p className="text-sm text-muted-foreground">已完成的单元会保留。若预算不足，可在继续构建时调整上限；待核算费用仍计入占用。</p>}
    {receipts&&<BuildReceipts taskId={task.task_id} active={['running','preparing','queued','queued_prepare'].includes(task.state)}/>}
    {task.materials.map(material=><article className="space-y-2 rounded-md border p-4" key={material.material_id}><h3>{material.label}</h3>{material.observation?.waiting_for_decision&&<p role="status">等待决定：请在已连接的构建宿主回答后继续。</p>}<p className="break-all text-sm">{material.published_book_ref?`成果已可用：${bookLabel(material.published_book_ref)}`:buildState(material.state)}</p><MaterialProgress material={material}/>{material.plan&&<details open={task.state==='awaiting_confirmation'}><summary>查看本资料的实际准备计划</summary><pre className="my-3 whitespace-pre-wrap break-words text-sm">{material.plan.review_markdown}</pre>{material.plan.model_runtime&&<p className="text-sm">模型：{material.plan.model_runtime.provider} / {material.plan.model_runtime.model}{material.plan.model_runtime.max_output_tokens!=null&&`；单次输出上限 ${material.plan.model_runtime.max_output_tokens} token`}</p>}</details>}{material.observation!=null&&<details><summary>构建状态记录</summary><pre className="max-h-64 overflow-auto whitespace-pre-wrap break-all text-xs">{JSON.stringify(material.observation,null,2)}</pre></details>}</article>)}
    {action&&<OperationDialog title={labels[action]} description={action==='pause'?'停止后续执行并收尾已启动任务，已经完成的单元保留。':action==='prepare'?'读取资料并生成可复核计划；此步不调用抽取模型。':mediaTask&&!videoTask?'导入精选资料，接纳有依据的对齐与事件，检查所需音乐能力后发布。本地处理不产生模型调用费用；实际伴听教学在用户互动时进行。':videoTask?'按预算准备视频字幕、画面、知识关系和讲解结构；复用已完成单元，不预生成 Tutor 教学材料。':'按已显示的确切资料计划执行。费用归平台；继续沿原构建引用复用已完成单元。'} open onOpenChange={v=>{if(!v)setAction(null);}} onSubmit={data=>ops.submit(labels[action],`/admin/operator-builds/${encodeURIComponent(task.task_id)}/${action}`,{expected_revision:task.revision,...(['start','continue'].includes(action)?{max_micro_cny:moneyInput(String(data.get('max')))}:{})})}>{['start','continue'].includes(action)&&<Field label="确认平台预算上限（元）"><Input name="max" inputMode="decimal" required defaultValue={money(task.max_micro_cny)}/></Field>}</OperationDialog>}
    {publish&&<ReleaseEditor existing={null} close={()=>setPublish(false)} publishPath={`/admin/operator-builds/${encodeURIComponent(task.task_id)}/publish`} initialAudience={task.visible_to_users} preparedMaterials={task.materials.flatMap(m=>m.published_book_ref?[{published_book_ref:m.published_book_ref,required_capabilities:m.required_capabilities??['text','tutor']}]:[])}/>}
  </>}</section>;
}

const stages:Record<string,string>={music_prepare:'正在准备精选音乐',music_import:'正在导入录音与谱本',music_score:'正在生成乐谱',music_alignment:'正在保存对齐结果与校准状态',music_events:'正在整理音乐事件',music_demonstrations:'正在生成谱面合成示范',music_ready:'音乐成果已就绪',music_publication:'正在保存音乐成果',completed:'构建完成',import:'正在导入视频',transcript:'正在转写字幕',complete:'构建完成',copying:'正在导入视频',probing:'正在读取视频信息',transcribing:'正在转写字幕',assembling:'正在整理字幕',translation:'正在翻译字幕',visual:'正在理解画面',graph:'正在整理知识关系',discourse:'正在整理讲解结构',ready:'构建完成',importing:'正在导入成果',preparing:'正在准备视频',video_translation:'正在翻译字幕',video_visual:'正在理解画面',video_graph:'正在整理知识关系',video_discourse:'正在整理讲解结构'};
function MaterialProgress({material}:{material:OperatorMaterial}){
  const progress=material.observation?.progress;
  if(!progress)return null;
  const counted=typeof progress.completed==='number'&&Number.isFinite(progress.completed)&&progress.completed>=0;
  const known=counted&&typeof progress.total==='number'&&Number.isFinite(progress.total)&&progress.total>0&&progress.completed!<=progress.total;
  const percent=known?Math.floor(progress.completed!/progress.total!*100):null;
  const units:Record<string,string>={chunks:'段',segments:'段',tasks:'项',batches:'批',units:'单元',frames:'帧',publications:'份成果'};
  const unit=units[progress.unit??'']??progress.unit??'单元';
  return <div className="space-y-2 text-sm" role="status" aria-label={`${material.label}构建进度`}>
    <p>{stages[progress.stage]??progress.stage}{counted&&` · 本阶段 ${progress.completed}${known?` / ${progress.total}`:''} ${unit}`}{percent!=null&&`（${percent}%）`}</p>
    {progress.message&&<p className="text-muted-foreground">{progress.message}</p>}
    {known?<progress className="block h-2 w-full accent-primary" aria-label={`${material.label}本阶段进度`} value={progress.completed} max={progress.total??undefined}/>:<div className="flex items-center gap-2 text-muted-foreground"><span aria-hidden="true" className="size-3 animate-spin rounded-full border-2 border-current border-r-transparent"/>等待本阶段进度，暂不估算百分比</div>}
  </div>;
}
function BuildReceipts({taskId,active}:{taskId:string;active:boolean}){
  const [offset,setOffset]=useState(0);
  const query=useData<Page<Charge>>(`/admin/charges?parent_task_ref=${encodeURIComponent(taskId)}&cost_owner=platform&limit=20&offset=${offset}`);
  useEffect(()=>{if(!active)return;const timer=setInterval(()=>{void query.refetch();},2000);return()=>clearInterval(timer);},[active,query.refetch]);
  return <section className="space-y-3 rounded-md border p-4" aria-label="构建费用回执"><div className="flex flex-wrap items-center justify-between gap-2"><h3>构建费用回执</h3><Button variant="ghost" onClick={()=>void query.refetch()}>刷新费用回执</Button></div><p className="text-sm text-muted-foreground">每次外部执行先准入，再登记费用。供应商未提供完整金额的回执保留为待核算；预占不是最终费用。</p><DataState query={query}/>{query.data&&<><DataTable rows={query.data.items} columns={[
    {header:'回执状态',cell:({row})=>row.original.needs_reconciliation?'回执冲突，待核算':({reserved:'已预占',sent:'执行中',pending:'待核算',settled:'已结算',released:'未发送，已释放'}[row.original.state]??row.original.state)},
    {header:'处理成本（元）',cell:({row})=><span>{money(row.original.provider_cost_micro_cny)}{row.original.provider_cost_status==='estimated'?'（估算）':row.original.provider_cost_status==='confirmed'?'（已确认）':'（待核算）'}</span>},
    {header:'当前占用（元）',cell:({row})=>money(['reserved','sent','pending'].includes(row.original.state)?row.original.reserved_micro_cny:0)},
    {header:'服务 / 用途',cell:({row})=>row.original.entry_kind==='service'?`${row.original.service_kind==='video_transcription'?'音频转写':row.original.service_kind??'资料处理'} / ${row.original.quantity_ms==null?'时长待确认':`${(row.original.quantity_ms/1000).toFixed(2)} 秒`}`:`${row.original.model??'—'} / ${row.original.purpose}`},
    {header:'执行',cell:({row})=><span className="block min-w-48 max-w-64 break-all whitespace-normal">{row.original.call_id}</span>},
  ]}/><Pagination offset={offset} total={query.data.total} change={setOffset}/></>}</section>;
}

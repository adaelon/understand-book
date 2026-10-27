import { useEffect, useRef, useState } from 'react';
import { DataState, DataTable, Pagination, useData } from '@/components/data';
import { Field, OperationDialog, useOperations } from '@/components/operation';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { session } from '@/lib/api';
import { date } from '@/lib/format';
import type { BookRef, Page } from '@/lib/types';
import { bookKey, bookLabel, releaseBody, type ExternalResource, type ReleaseMaterial, type ReleasePart, type ReleaseReadiness, type WorkspaceRelease } from '@/lib/workspace-releases';
import { Disclosure, Distribution, Notice, StatusBadge, ViewSwitch } from '@/components/analytics';

const textareaClass = 'min-h-20 w-full rounded-md border bg-transparent px-3 py-2 text-sm';
const capabilities = [['text','阅读原文'],['tutor','资料理解'],['teaching_assets','教学材料'],
  ['music','音乐播放'],['music_score','可查乐谱'],['music_events','音乐事件'],['music_note_alignment','逐音符跟谱'],['video','视频伴读'],['translation','视频中文'],['visual_index','视频画面索引'],['graph','视频知识图谱'],['discourse','视频讲解结构']] as const;
export function WorkspaceReleasesPage() {
  const [offset,setOffset]=useState(0),[selected,setSelected]=useState<WorkspaceRelease|null>(null);
  const [edit,setEdit]=useState<{existing:WorkspaceRelease|null}|null>(null),[error,setError]=useState('');
  const [archive,setArchive]=useState<WorkspaceRelease|null>(null);
  const [filter,setFilter]=useState<'all'|'attention'|'ready'|'archived'>('all');
  const query=useData<Page<WorkspaceRelease>>(`/admin/workspace-releases?limit=20&offset=${offset}`);
  const ops=useOperations();
  async function exportRelease(release:WorkspaceRelease) {
    setError('');
    try {
      const value=await session.request(`/admin/workspace-releases/${encodeURIComponent(release.release_id)}/versions/${release.version}/export`);
      const url=URL.createObjectURL(new Blob([JSON.stringify(value,null,2)],{type:'application/json'}));
      const a=document.createElement('a'); a.href=url; a.download=`${release.release_id}-v${release.version}.json`; a.click(); setTimeout(()=>URL.revokeObjectURL(url),0);
    } catch(failure) { setError((failure as Error).message); }
  }
  return <section className="space-y-5">
    <div className="flex flex-wrap items-center justify-between gap-3"><h1>共享工作区发布</h1><div className="flex gap-2"><Button variant="outline" onClick={()=>{setSelected(null);void query.refetch();}}>刷新发布</Button><Button disabled={ops.blocked} onClick={()=>setEdit({existing:null})}>发布新工作区</Button></div></div>
    <p className="text-sm text-muted-foreground">发布学习目标与路线，查看资料就绪情况，管理可见账号和套餐范围。</p>
    {error&&<p role="alert">{error}</p>}<DataState query={query}/>
    {query.data&&<><div className="analytics-panel"><Distribution label="本页发布状态" segments={[
      {label:'完整可用',value:query.data.items.filter(r=>!r.archived&&r.availability.status==='ready').length,tone:'green'},
      {label:'尚未就绪',value:query.data.items.filter(r=>!r.archived&&r.availability.status!=='ready').length,tone:'amber'},
      {label:'已归档',value:query.data.items.filter(r=>r.archived).length,tone:'muted'},
    ]} note={`共 ${query.data.total} 个发布版本，色条与筛选仅统计当前页。`} /><ViewSwitch label="筛选本页发布" value={filter} options={[["all","本页全部"],["attention","尚未就绪"],["ready","完整可用"],["archived","已归档"]]} onChange={setFilter}/></div><DataTable rows={query.data.items.filter(r=>filter==='all'||(filter==='archived'?r.archived:!r.archived&&(filter==='ready'?r.availability.status==='ready':r.availability.status!=='ready')))} columns={[
      {header:'名称 / 版本',cell:({row})=><><strong>{row.original.manifest.name}</strong><p className="break-all text-xs">{row.original.release_id} · v{row.original.version}</p></>},
      {header:'当前可用情况',cell:({row})=><StatusBadge tone={row.original.archived?'muted':row.original.availability.status==='ready'?'green':'amber'}>{row.original.archived?'已归档':row.original.availability.status==='ready'?'完整可用':'尚未完整就绪'}</StatusBadge>},
      {header:'可见账号',cell:({row})=><span className="block max-w-48 whitespace-normal">{[...row.original.visible_to_users,...(row.original.allowed_tiers??[]).map(tier=>({basic:'基础版',plus:'进阶版',pro:'专业版'}[tier]??tier))].join('、')}</span>},
      {header:'发布时间',cell:({row})=>date(row.original.created_at)},
      {header:'操作',cell:({row})=><div className="space-y-2"><Button variant="outline" onClick={()=>setSelected(row.original)}>查看版本</Button><details className="text-xs"><summary className="cursor-pointer text-muted-foreground">更多操作</summary><div className="mt-2 flex flex-col items-start gap-1"><Button variant="ghost" disabled={ops.blocked} onClick={()=>setEdit({existing:row.original})}>修订为新版本</Button><Button variant="ghost" onClick={()=>void exportRelease(row.original)}>导出公开清单</Button><Button variant="ghost" disabled={ops.blocked} onClick={()=>setArchive(row.original)}>{row.original.archived?'恢复共享入口':'归档共享入口'}</Button></div></details></div>},
    ]}/><Pagination offset={offset} total={query.data.total} change={value=>{setOffset(value);setSelected(null);}}/></>}
    {selected&&<ReleaseDetail key={`${selected.release_id}:${selected.version}`} initial={selected} close={()=>setSelected(null)}/>}
    {edit&&<ReleaseEditor existing={edit.existing} close={()=>setEdit(null)}/>}
    {archive&&<OperationDialog title={archive.archived?'恢复共享入口':'归档共享入口'} description="归档后停止创建新的个人实例；已开始学习的工作区和旧来源继续保留。" open onOpenChange={value=>{if(!value)setArchive(null);}} onSubmit={()=>ops.submit('更改共享入口',`/admin/workspace-releases/${encodeURIComponent(archive.release_id)}/archive`,{archived:!archive.archived})}><p>{archive.manifest.name}</p></OperationDialog>}
  </section>;
}
function Readiness({title,value}:{title:string;value:ReleaseReadiness}) {
  const materialRow=(material:ReleaseReadiness['materials'][number])=><li className="break-words rounded-md bg-muted/30 p-3" key={bookKey(material.published_book_ref)}><strong className="font-medium">{bookLabel(material.published_book_ref)}</strong><p className="mt-1 text-muted-foreground">{!material.publication_available?'资料版本不可用':material.missing_capabilities.length?`缺少能力 ${material.missing_capabilities.join('、')}`:'所需能力已就绪'}</p></li>;
  const audienceRow=(user:ReleaseReadiness['audience'][number])=><li className="break-words rounded-md bg-muted/30 p-3" key={user.user_id}>账号 {user.user_id}：{user.ready?'已具备材料授权':`授权未齐全 ${user.missing_grants.map(bookLabel).join('、')}`}</li>;
  return <section className="space-y-3 rounded-lg border p-4"><h3 className="font-semibold">{title}</h3><Notice warning={value.status!=='ready'}>{value.status==='ready'?'完整可用':'尚未完整就绪，请处理以下缺项'}</Notice><ul className="space-y-2 text-sm">{value.materials.filter(m=>!m.publication_available||m.missing_capabilities.length>0).map(materialRow)}{value.audience.filter(u=>!u.ready).map(audienceRow)}</ul><Disclosure title={`全部检查结果 · ${value.materials.length} 份资料 · ${value.audience.length} 个账号`}><ul className="space-y-2 text-sm">{value.materials.map(materialRow)}{value.audience.map(audienceRow)}</ul></Disclosure></section>;
}
function ReleaseDetail({initial,close}:{initial:WorkspaceRelease;close:()=>void}) {
  const anchor=useRef<HTMLElement>(null);
  const query=useData<WorkspaceRelease>(`/admin/workspace-releases/${encodeURIComponent(initial.release_id)}/versions/${initial.version}`);
  const release=query.data;
  useEffect(()=>{if(release){anchor.current?.focus({preventScroll:true});anchor.current?.scrollIntoView?.({block:'start'});}},[!!release]);
  return <section ref={anchor} tabIndex={-1} className="analytics-panel outline-none" aria-label="共享工作区版本详情"><div className="flex items-center justify-between gap-3"><h2>{initial.manifest.name} · v{initial.version}</h2><Button variant="ghost" onClick={close}>关闭版本</Button></div><DataState query={query}/>{release&&<><Readiness title="当前" value={release.availability}/><dl className="grid gap-4 text-sm sm:grid-cols-2"><div><dt className="text-muted-foreground">发布者</dt><dd>{release.manifest.publisher}</dd></div><div><dt className="text-muted-foreground">公开目标</dt><dd className="whitespace-pre-wrap">{release.manifest.public_goal}</dd></div><div><dt className="text-muted-foreground">适用范围</dt><dd className="whitespace-pre-wrap">{release.manifest.applicability}</dd></div><div><dt className="text-muted-foreground">先修要求</dt><dd className="whitespace-pre-wrap">{release.manifest.prerequisites || '无'}</dd></div></dl>{!!release.manifest.external_resources?.length&&<section><h3>外部链接导览</h3>{release.manifest.external_resources.map((resource,index)=><p key={index}><a href={resource.url} target="_blank" rel="noopener noreferrer">{resource.title} ↗</a><span className="block text-sm text-muted-foreground">{resource.description}</span></p>)}</section>}<Disclosure title="发布时的就绪情况"><Readiness title="发布时" value={release.readiness_at_publication}/></Disclosure>{([['主题联系',release.manifest.topic_links],['建议路线',release.manifest.suggested_route]] as const).map(([title,parts])=><section key={title}><h3>{title}</h3><ol className="list-inside list-decimal">{parts.map((part,index)=><li className="my-3" key={index}><strong>{part.title}</strong><p className="whitespace-pre-wrap text-sm">{part.description}</p><p className="text-xs text-muted-foreground">{part.materials.map(bookLabel).join('、')}</p></li>)}</ol>{!parts.length&&<p className="text-sm text-muted-foreground">尚未编排</p>}</section>)}</>}</section>;
}
export function ReleaseEditor({existing,close,preparedMaterials,publishPath,initialAudience}:{existing:WorkspaceRelease|null;close:()=>void;preparedMaterials?:ReleaseMaterial[];publishPath?:string;initialAudience?:string[]}) {
  const [bookOffset,setBookOffset]=useState(0);
  const books=useData<{books:{published_book_ref:BookRef;material_kind?:'book'|'video'|'music'}[];total:number}>(`/admin/books?limit=20&offset=${bookOffset}`);
  const [materials,setMaterials]=useState<ReleaseMaterial[]>(()=>(existing?.manifest.materials??preparedMaterials??[]).map(m=>({...m,required_capabilities:[...m.required_capabilities]})));
  const [topics,setTopics]=useState<ReleasePart[]>(()=>existing?.manifest.topic_links.map(p=>({...p,materials:[...p.materials]}))??[]);
  const [route,setRoute]=useState<ReleasePart[]>(()=>existing?.manifest.suggested_route.map(p=>({...p,materials:[...p.materials]}))??[]);
  const [externalResources,setExternalResources]=useState<ExternalResource[]>(()=>existing?.manifest.external_resources?.map(resource=>({...resource}))??[]);
  const ops=useOperations();
  function toggle(book:BookRef,checked:boolean) {
    const selected=preparedMaterials?.find(m=>bookKey(m.published_book_ref)===bookKey(book));
    const kind=books.data?.books.find(m=>bookKey(m.published_book_ref)===bookKey(book))?.material_kind;
    setMaterials(rows=>checked?[...rows,{published_book_ref:book,required_capabilities:selected?.required_capabilities??(kind==='music'?['music','music_score']:kind==='video'?['text','video']:['text'])}]:rows.filter(m=>bookKey(m.published_book_ref)!==bookKey(book)));
    if(!checked) { const remove=(parts:ReleasePart[])=>parts.map(part=>({...part,materials:part.materials.filter(ref=>bookKey(ref)!==bookKey(book))})); setTopics(remove);setRoute(remove); }
  }
  return <OperationDialog title={existing?'发布工作区新版本':'发布新工作区'} description={existing?'修改公开编排后生成独立新版本，旧版本和原资料引用保留。':'所填内容会成为公共发布清单；发布后显示实际就绪情况。'} open onOpenChange={value=>{if(!value)close();}} onSubmit={data=>{const release=releaseBody(data,materials,topics,route,existing,externalResources);return ops.submit('发布共享工作区',publishPath??'/admin/workspace-releases',publishPath?{release}:release);}}>
    {!existing&&<Field label="发布标识"><Input name="release_id" pattern="[A-Za-z0-9_-]+" required placeholder="例如 deep-learning"/></Field>}
    <Field label="工作区名称"><Input name="name" required defaultValue={existing?.manifest.name}/></Field>
    <Field label="公开发布者名称"><Input name="publisher" required defaultValue={existing?.manifest.publisher}/></Field>
    <Field label="公开学习目标"><textarea className={textareaClass} name="public_goal" required defaultValue={existing?.manifest.public_goal}/></Field>
    <Field label="适用范围"><textarea className={textareaClass} name="applicability" required defaultValue={existing?.manifest.applicability}/></Field>
    <Field label="先修要求"><textarea className={textareaClass} name="prerequisites" required placeholder="例如：无需先修知识" defaultValue={existing?.manifest.prerequisites}/></Field>
    <fieldset className="space-y-2 rounded-md border p-3"><legend>课程套餐开放范围</legend>{([['basic','基础版'],['plus','进阶版'],['pro','专业版']] as const).map(([tier,label])=><label className="mr-4 inline-flex items-center gap-2" key={tier}><input type="checkbox" name="allowed_tiers" value={tier} defaultChecked={existing?.allowed_tiers?.includes(tier)??false}/>{label}</label>)}<p className="text-sm text-muted-foreground">所选档位的有效月付或冲刺套餐均可使用。本课程公共资料不占个人上传空间。</p></fieldset>
    <Field label="额外可见账号（空格或逗号分隔）"><Input name="visible_to_users" defaultValue={(existing?.visible_to_users??initialAudience)?.join(', ')}/></Field>
    <fieldset className="my-4 space-y-2 rounded-md border p-3"><legend>选择确切资料版本</legend>{!preparedMaterials&&<DataState query={books}/>} {(preparedMaterials??books.data?.books)?.map(({published_book_ref:book})=><label className="flex items-center gap-2 text-sm" key={bookKey(book)}><input type="checkbox" checked={materials.some(m=>bookKey(m.published_book_ref)===bookKey(book))} onChange={e=>toggle(book,e.target.checked)}/>{bookLabel(book)}</label>)}{!preparedMaterials&&books.data&&<Pagination offset={bookOffset} total={books.data.total} change={setBookOffset}/>}</fieldset>
    <section className="space-y-3"><h3>已选资料与必要能力</h3>{materials.map(material=><fieldset className="rounded-md border p-3" key={bookKey(material.published_book_ref)}><legend className="break-all text-sm">{bookLabel(material.published_book_ref)}</legend><div className="flex flex-wrap gap-3">{capabilities.map(([key,label])=><label className="flex items-center gap-1 text-sm" key={key}><input type="checkbox" checked={material.required_capabilities.includes(key)} onChange={e=>setMaterials(rows=>rows.map(row=>row===material?{...row,required_capabilities:e.target.checked?[...row.required_capabilities,key]:row.required_capabilities.filter(c=>c!==key)}:row))}/>{label}</label>)}</div><Button type="button" variant="ghost" onClick={()=>toggle(material.published_book_ref,false)}>移除此资料</Button></fieldset>)}</section>
    <ExternalResourceEditor resources={externalResources} change={setExternalResources}/>
    <PartEditor title="主题联系" parts={topics} change={setTopics} materials={materials}/><PartEditor title="建议路线" parts={route} change={setRoute} materials={materials}/>
  </OperationDialog>;
}
function PartEditor({title,parts,change,materials}:{title:string;parts:ReleasePart[];change:(parts:ReleasePart[])=>void;materials:ReleaseMaterial[]}) {
  const update=(index:number,patch:Partial<ReleasePart>)=>change(parts.map((part,i)=>i===index?{...part,...patch}:part));
  return <section className="my-5 space-y-3"><div className="flex items-center justify-between"><h3>{title}</h3><Button type="button" variant="outline" onClick={()=>change([...parts,{title:'',description:'',materials:[]}])}>添加{title}</Button></div>{parts.map((part,index)=><fieldset className="space-y-2 rounded-md border p-3" key={index}><legend>{title} {index+1}</legend><Field label={`${title}标题`}><Input value={part.title} required onChange={e=>update(index,{title:e.target.value})}/></Field><Field label={`${title}说明`}><textarea className={textareaClass} value={part.description} required onChange={e=>update(index,{description:e.target.value})}/></Field><p className="text-sm">关联资料</p>{materials.map(({published_book_ref:book})=><label className="flex items-center gap-2 text-sm" key={bookKey(book)}><input type="checkbox" checked={part.materials.some(ref=>bookKey(ref)===bookKey(book))} onChange={e=>update(index,{materials:e.target.checked?[...part.materials,book]:part.materials.filter(ref=>bookKey(ref)!==bookKey(book))})}/>{bookLabel(book)}</label>)}<Button type="button" variant="ghost" onClick={()=>change(parts.filter((_,i)=>i!==index))}>移除{title}</Button></fieldset>)}</section>;
}

function ExternalResourceEditor({resources,change}:{resources:ExternalResource[];change:(resources:ExternalResource[])=>void}) {
  const update=(index:number,patch:Partial<ExternalResource>)=>change(resources.map((resource,i)=>i===index?{...resource,...patch}:resource));
  return <section className="my-5 space-y-3"><div className="flex items-center justify-between"><h3>外部链接导览</h3><Button type="button" variant="outline" onClick={()=>change([...resources,{title:'',url:'',description:''}])}>添加外部链接</Button></div><p className="text-sm text-muted-foreground">只提供前往原网站的链接。链接内容不会显示为已解析的教材、视频或转写。</p>{resources.map((resource,index)=><fieldset className="space-y-2 rounded-md border p-3" key={index}><legend>外部链接 {index+1}</legend><Field label="链接名称"><Input name={`external_title_${index}`} value={resource.title} required onChange={e=>update(index,{title:e.target.value})}/></Field><Field label="网址"><Input name={`external_url_${index}`} type="url" value={resource.url} required onChange={e=>update(index,{url:e.target.value})}/></Field><Field label="导览说明"><textarea name={`external_description_${index}`} className={textareaClass} value={resource.description} onChange={e=>update(index,{description:e.target.value})}/></Field><Button type="button" variant="ghost" onClick={()=>change(resources.filter((_,i)=>i!==index))}>移除外部链接</Button></fieldset>)}</section>;
}

<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount } from 'vue';
import { preparations, type Preparation } from '../personal-preparations';
import { allowancePercent, allowanceTime } from '../account-allowance';
import type { LearningWorkspace, PublishedBookRef } from '../network-context';
const props=defineProps<{workspace:LearningWorkspace}>();
const emit=defineEmits<{ready:[];open:[reference:PublishedBookRef];allowance:[]}>();
const workspaceId=props.workspace.learning_workspace_id;
const items=ref<Preparation[]>([]),error=ref(''),busy=ref(false),uploadProgress=ref<number|null>(null),budgets=ref<Record<string,string>>({});
const fileInput=ref<HTMLInputElement|null>(null),dragging=ref(false),uploadFile=ref<{name:string;size:number}|null>(null),uploaded=ref(false);
let timer:ReturnType<typeof setInterval>|undefined,loading=false,alive=true;
const knownReady=new Set<string>();
const states:Record<string,string>={draft:'等待准备',queued_prepare:'等待生成方案',preparing:'正在生成方案',awaiting_confirmation:'请确认额度上限',queued:'等待开始',running:'正在准备',interrupted:'已暂停',ready:'已加入原工作区'};
async function refresh(){
  if(loading)return;loading=true;
  try{
    const result=await preparations.list(workspaceId);if(!alive)return;
    items.value=result.items;
    for(const task of result.items){
      if(task.state==='ready'&&!knownReady.has(task.task_id)){knownReady.add(task.task_id);emit('ready');}
      if(budgets.value[task.task_id]===undefined)budgets.value[task.task_id]=task.allowance?.remaining ? task.allowance.maximum.value : '5';
    }
  }catch(failure){if(alive)error.value=failure instanceof Error?failure.message:String(failure);}
  finally{loading=false;}
}
function fileSize(size:number){return size<1024?`${size} B`:size<1024*1024?`${(size/1024).toFixed(1)} KB`:`${(size/1024/1024).toFixed(1)} MB`;}
async function upload(file:File){
  if(busy.value||props.workspace.archived)return;
  error.value='';uploaded.value=false;
  if(!/\.(epub|md|txt|mp4)$/i.test(file.name)){error.value='请选择 EPUB、Markdown、文本或 MP4 文件。';return;}
  if(file.size===0){error.value='这个文件是空的，请重新选择资料。';return;}
  uploadFile.value={name:file.name,size:file.size};busy.value=true;uploadProgress.value=0;
  try{await preparations.upload(workspaceId,file,bytes=>{if(alive)uploadProgress.value=Math.round(bytes/file.size*100);});uploaded.value=true;await refresh();}
  catch(failure){error.value=failure instanceof Error?failure.message:String(failure);}
  finally{busy.value=false;uploadProgress.value=null;}
}
function chooseFile(event:Event){const field=event.target as HTMLInputElement,file=field.files?.[0];field.value='';if(file)void upload(file);}
function dropFile(event:DragEvent){dragging.value=false;if(busy.value)return;const files=event.dataTransfer?.files;if(!files?.length)return;if(files.length>1){error.value='每次添加一份资料，完成后可以继续添加。';return;}void upload(files[0]);}
async function act(task:Preparation,action:string){
  busy.value=true;error.value='';
  try{
    const budget=budgets.value[task.task_id]??'';
    if(['start','continue'].includes(action)&&(!/^\d+(\.\d{1,3})?$/.test(budget)||Number(budget)<=0))throw new Error('请输入大于 0、最多三位小数的额度百分比。');
    await preparations.act(workspaceId,{task_id:task.task_id,action,expected_revision:task.revision,
      ...(['start','continue'].includes(action)?{period_id:task.allowance?.period_id,max_milli_percent:Math.round(Number(budget)*1000)}:{})});
    await refresh();
  }catch(failure){error.value=failure instanceof Error?failure.message:String(failure);await refresh();}
  finally{busy.value=false;}
}
function canStart(task:Preparation){return task.state==='awaiting_confirmation'||task.state==='draft'&&task.materials.every(m=>m.kind==='video');}
onMounted(()=>{void refresh();timer=setInterval(()=>{void refresh();},2000);});
onBeforeUnmount(()=>{alive=false;clearInterval(timer);});
</script>
<template>
  <section class="personal-preparations" aria-label="个人资料准备">
    <div class="preparation-heading"><div><h3>加入自己的资料</h3><p>把书籍、笔记或视频放进这个工作区，接着一起学。</p></div><span class="private-label">个人资料</span></div>
    <div v-if="!workspace.archived" class="upload-zone" :class="{'is-dragging':dragging,'is-uploading':uploadProgress!==null}" role="group" aria-label="资料上传区" @dragover.prevent="dragging=!busy" @dragleave.self="dragging=false" @drop.prevent="dropFile">
      <input ref="fileInput" class="file-input" aria-label="上传个人资料" type="file" accept=".epub,.md,.txt,.mp4" tabindex="-1" :disabled="busy" @change="chooseFile">
      <div class="upload-icon" aria-hidden="true"><svg viewBox="0 0 32 32" fill="none"><path d="M18 5H9a2 2 0 0 0-2 2v18a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V12L18 5Z"/><path d="M18 5v7h7M16 23V15m-4 4 4-4 4 4"/></svg></div>
      <div class="upload-copy"><strong>{{dragging?'松开即可添加资料':uploadProgress!==null?'正在上传资料':'选择一份资料，或拖到这里'}}</strong><span>EPUB · Markdown · TXT · MP4</span></div>
      <button class="upload-button" :disabled="busy" @click="fileInput?.click()">{{uploadProgress!==null?'正在上传…':'选择文件'}}</button>
      <div v-if="uploadFile" class="upload-file" aria-live="polite"><div><span class="upload-filename">{{uploadFile.name}}</span><span>{{fileSize(uploadFile.size)}}</span></div><span v-if="uploadProgress!==null">{{uploadProgress}}%<template v-if="uploadProgress===100"> · 正在保存</template></span><span v-else-if="uploaded" class="uploaded-label">已添加到准备列表</span></div>
      <progress v-if="uploadProgress!==null" class="upload-progress" aria-label="资料上传进度" :value="uploadProgress" max="100" />
    </div>
    <p class="preparation-note">上传后先查看准备方案，再确认额度。视频先准备原文；完成后会回到这个工作区。</p>
    <p v-if="error" role="alert">{{error}}</p>
    <article v-for="task in items" :key="task.task_id">
      <div class="task-heading"><strong>{{task.name}}</strong><span class="task-status" :class="{'is-ready':task.state==='ready'}" role="status">{{states[task.state]??'等待更新'}}</span></div>
      <p>{{task.estimate.message}}</p>
      <p v-if="task.estimate.status==='no_model_cost'">预计使用额度：0%</p>
      <p v-else-if="task.estimate.status==='estimated'">{{task.estimate.partial?'已能估算部分':'预计'}}：{{allowancePercent(task.estimate.lower)}} ～ {{allowancePercent(task.estimate.upper)}}<span v-if="task.estimate.partial">；其余步骤尚无法估算，确认上限仍约束总扣减。</span></p>
      <p v-else>预计额度：方案生成后计算；没有可靠估算的步骤会保留说明。</p>
      <template v-for="(material,index) in task.materials" :key="index">
        <p v-if="material.progress">{{material.progress.message}} · {{material.progress.completed}}<template v-if="material.progress.total>0"> / {{material.progress.total}}</template></p>
        <progress v-if="material.progress&&material.progress.total>0" :value="material.progress.completed" :max="material.progress.total" />
        <button v-if="task.state==='ready'&&material.published_book_ref" @click="emit('open',material.published_book_ref)">继续学习</button>
      </template>
      <template v-if="task.allowance">
        <p>当前可用 {{allowancePercent(task.allowance.available)}} · 有效至 {{allowanceTime(task.allowance.expires_at)}}</p>
        <p v-if="task.allowance.remaining">任务上限 {{allowancePercent(task.allowance.maximum)}} · 剩余上限 {{allowancePercent(task.allowance.remaining)}}</p>
      </template>
      <p v-else>当前没有可用的额度期。</p>
      <p v-for="period in task.usage_periods" :key="period.period_id">截至 {{allowanceTime(period.expires_at)}} 的额度期：已用 {{allowancePercent(period.used)}} · 预计占用 {{allowancePercent(period.reserved)}} · 待核算 {{allowancePercent(period.pending)}}</p>
      <p v-if="task.error">准备已停在保存的位置。请检查可用额度、任务上限，或联系资料准备服务；继续会复用已完成成果。</p>
      <div v-if="!workspace.archived" class="actions">
        <button v-if="task.state==='draft'&&!canStart(task)||task.state==='interrupted'&&!task.materials.every(m=>m.plan_ready||m.kind==='video')" :disabled="busy" @click="act(task,'prepare')">生成准备方案</button>
        <template v-if="canStart(task)||task.state==='interrupted'&&task.materials.every(m=>m.plan_ready||m.kind==='video')">
          <label>本次总上限 <input v-model="budgets[task.task_id]" :aria-label="`${task.name}额度上限`" type="number" min="0.001" step="0.001"> %</label>
          <button :disabled="busy||!task.allowance" @click="act(task,task.state==='interrupted'?'continue':'start')">{{task.state==='interrupted'?'确认当前额度并继续':'确认上限并开始'}}</button>
          <button @click="emit('allowance')">补充额度</button>
        </template>
        <button v-if="['queued_prepare','preparing','queued','running'].includes(task.state)" :disabled="busy||task.pause_requested" @click="act(task,'pause')">{{task.pause_requested?'正在暂停':'暂停准备'}}</button>
      </div>
    </article>
  </section>
</template>
<style scoped>
.personal-preparations{margin:1.2rem 0;padding:1.3rem;border:1px solid var(--line,#ddd8ce);border-radius:16px;background:var(--paper,#fcfaf6);color:var(--ink,#29382e)}h3{margin:0;font-size:1.08rem}p{line-height:1.65;color:var(--ink-muted,#686d65);font-size:.88rem}.preparation-heading{display:flex;justify-content:space-between;align-items:flex-start;gap:1rem}.preparation-heading p{margin:.45rem 0 1.1rem}.private-label{white-space:nowrap;font-size:.73rem;color:var(--ink-muted,#686d65);border:1px solid var(--line,#ddd8ce);border-radius:99px;padding:.25rem .6rem}.upload-zone{position:relative;display:grid;grid-template-columns:auto 1fr auto;align-items:center;gap:1rem;padding:1.35rem;border:1.5px dashed var(--brand-accent-line,#a9bba9);border-radius:12px;background:var(--brand-accent-soft,#f0f5ed);transition:background .15s,border-color .15s}.upload-zone.is-dragging{background:var(--brand-accent-soft-hover,#ecd5c1);border-color:var(--brand-accent,#b65a17)}.file-input{position:absolute;width:1px;height:1px;padding:0;margin:-1px;overflow:hidden;clip:rect(0,0,0,0);white-space:nowrap;border:0}.upload-icon{display:grid;place-items:center;width:48px;height:48px;border-radius:12px;background:var(--paper,#fffdf8);color:var(--brand-accent-deep,#91420f)}.upload-icon svg{width:30px;height:30px;stroke:currentColor;stroke-width:1.6;stroke-linecap:round;stroke-linejoin:round}.upload-copy{min-width:0;display:grid;gap:.35rem}.upload-copy strong{font-size:.93rem;font-weight:600}.upload-copy span{font-size:.77rem;color:var(--ink-muted,#686d65)}button{padding:.55rem .9rem;border:1px solid var(--line,#d5d8ce);border-radius:8px;background:var(--paper,#fffdf8);color:inherit;font:inherit;font-size:.85rem;cursor:pointer}button:focus-visible{outline:2px solid var(--brand-accent,#b65a17);outline-offset:3px}button:disabled{opacity:.5;cursor:default}button.upload-button{background:var(--brand-accent-deep,#91420f);border-color:var(--brand-accent-deep,#91420f);color:#fff;white-space:nowrap;padding:.65rem 1.1rem}.upload-file{grid-column:1/-1;display:flex;justify-content:space-between;gap:.7rem;font-size:.79rem;color:var(--ink-muted,#686d65);border-top:1px solid var(--brand-accent-line,#d0ddcb);padding-top:.8rem}.upload-file>div{display:flex;gap:.7rem;min-width:0;align-items:baseline}.upload-filename{overflow-wrap:anywhere;color:var(--ink,#29382e)}.upload-file>div>span:last-child{white-space:nowrap}.uploaded-label{color:var(--brand-accent-deep,#91420f)}progress{width:100%;max-width:26rem;accent-color:var(--brand-accent,#b65a17)}progress.upload-progress{grid-column:1/-1;max-width:none;height:7px}.preparation-note{margin:.7rem 0 0;font-size:.78rem}article{padding:1.1rem 0 .2rem;border-top:1px solid var(--line,#ddd8ce);margin-top:1.1rem}.task-heading,.actions{display:flex;align-items:center;gap:.75rem;flex-wrap:wrap}.task-heading{justify-content:space-between}strong{overflow-wrap:anywhere}.task-status{font-size:.75rem;padding:.25rem .55rem;border-radius:6px;background:var(--canvas,#f1eee6);color:var(--ink-muted,#686d65)}.task-status.is-ready{background:var(--brand-accent-soft,#edf3e8);color:var(--brand-accent-deep,#91420f)}.actions input{width:5rem}p[role=alert]{color:#a33825}
@media(max-width:560px){.personal-preparations{padding:1rem}.private-label{display:none}.upload-zone{grid-template-columns:auto 1fr;padding:1rem;gap:.75rem}.upload-button{grid-column:1/-1;width:100%}.upload-file,.upload-file>div{flex-wrap:wrap}.upload-copy strong{font-size:.85rem}.upload-copy span{font-size:.7rem}.upload-icon{width:40px;height:44px}.upload-file{gap:.3rem}.task-heading{align-items:flex-start}.task-status{max-width:100%}}
</style>


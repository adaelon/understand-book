<script setup lang="ts">
import { t as uiText } from '../i18n';

import { computed, ref, shallowRef, onMounted, onBeforeUnmount } from 'vue';
import { accountStorage, preparations, preparationUpload, type AccountStorage, type Preparation, type PreparationUpload, type StorageItem } from '../personal-preparations';
import { allowancePercent, allowanceTime } from '../account-allowance';
import type { LearningWorkspace, PublishedBookRef } from '../network-context';
const props=defineProps<{workspace:LearningWorkspace}>();
const emit=defineEmits<{ready:[];open:[reference:PublishedBookRef];allowance:[]}>();
const workspaceId=props.workspace.learning_workspace_id;
const items=ref<Preparation[]>([]),error=ref(''),busy=ref(false),uploadProgress=ref<number|null>(null),budgets=ref<Record<string,string>>({});
const fileInput=ref<HTMLInputElement|null>(null),dragging=ref(false),uploadFile=ref<{name:string;size:number}|null>(null),uploaded=ref(false);
const storage=ref<AccountStorage|null>(null),storageError=ref(''),showStorage=ref(false),storageBusy=ref(false);
const pendingFile=shallowRef<File|null>(null),uploadSession=ref<PreparationUpload|null>(null),resumeItem=ref<StorageItem|null>(null),cancelBusy=ref(false);
const blocked=computed(()=>busy.value||storageBusy.value||cancelBusy.value);
const canChoose=computed(()=>!blocked.value&&!props.workspace.archived&&!!storage.value?.can_upload&&!uploadSession.value);
let activeUpload:Promise<void>|null=null;
let timer:ReturnType<typeof setInterval>|undefined,loading=false,alive=true;
const knownReady=new Set<string>();
const supportedFormats=ref(['epub','md','txt','mp4','pdf']);
const fileAccept=computed(()=>supportedFormats.value.map(format=>`.${format}`).join(','));
const formatLabels=computed(()=>supportedFormats.value.map(format=>format==='md'?'Markdown':format.toUpperCase()).join(' · '));
const states:Record<string,string>={draft:uiText('等待准备'),queued_prepare:uiText('等待生成方案'),preparing:uiText('正在生成方案'),awaiting_confirmation:uiText('请确认额度上限'),queued:uiText('等待开始'),running:uiText('正在准备'),interrupted:uiText('已暂停'),ready:uiText('已加入原工作区')};
async function refresh(){
  if(loading)return;loading=true;
  try{
    const result=await preparations.list(workspaceId);if(!alive)return;
    items.value=result.items;
    supportedFormats.value=result.supported_formats??['epub','md','txt','mp4'];
    for(const task of result.items){
      if(task.state==='ready'&&!knownReady.has(task.task_id)){knownReady.add(task.task_id);emit('ready');}
      if(budgets.value[task.task_id]===undefined)budgets.value[task.task_id]=task.allowance?.remaining ? task.allowance.maximum.value : '5';
    }
  }catch(failure){if(alive)error.value=failure instanceof Error?failure.message:String(failure);}
  finally{loading=false;}
}
async function refreshStorage(){
  try{const result=await accountStorage.read();if(alive){storage.value=result;storageError.value='';}}
  catch(failure){if(alive)storageError.value=failure instanceof Error?failure.message:String(failure);}
}
function fileSize(size:number){return size<1024?`${size} B`:size<1024*1024?`${(size/1024).toFixed(1)} KB`:size<1024*1024*1024?`${(size/1024/1024).toFixed(1)} MB`:`${(size/1024/1024/1024).toFixed(2)} GB`;}
async function upload(file:File){
  if(blocked.value||props.workspace.archived)return;
  error.value='';uploaded.value=false;
  if(!supportedFormats.value.includes(file.name.split('.').pop()?.toLowerCase()??'')){error.value=uiText('当前支持 {0}。课件可先导出为带文字层的 PDF。', [formatLabels.value]);return;}
  if(file.size===0){error.value=uiText('这个文件是空的，请重新选择资料。');return;}
  if(resumeItem.value){
    if(file.name!==resumeItem.value.filename||file.size!==resumeItem.value.bytes){error.value=uiText('请选择这份上传原来的同名、同大小文件。');return;}
    uploadSession.value=preparationUpload(resumeItem.value.storage_id);resumeItem.value=null;
  }else{
    if(uploadSession.value){error.value=uiText('请先继续或取消当前上传。');return;}
    if(!storage.value?.can_upload){error.value=uiText('当前无法新增上传，请查看套餐有效期和空间。');return;}
    if(storage.value.max_file_bytes!==null&&file.size>storage.value.max_file_bytes){error.value=uiText('单份资料不能超过 {0}。', [fileSize(storage.value.max_file_bytes)]);return;}
    if(file.size>storage.value.available_bytes){error.value=uiText('剩余空间不足，请先整理资料或选择更高容量套餐。');return;}
    uploadSession.value=preparationUpload();
  }
  pendingFile.value=file;uploadFile.value={name:file.name,size:file.size};
  await transmit();
}
async function transmit(){
  const session=uploadSession.value,file=pendingFile.value;if(!session||!file||blocked.value)return;
  busy.value=true;error.value='';uploadProgress.value=Math.round(session.received/file.size*100);
  activeUpload=(async()=>{
    try{
      await preparations.upload(workspaceId,file,bytes=>{if(alive)uploadProgress.value=Math.round(bytes/file.size*100);},session);
      if(alive){uploaded.value=true;uploadSession.value=null;pendingFile.value=null;await refresh();}
    }catch(failure){if(alive&&!session.cancelled)error.value=failure instanceof Error?failure.message:String(failure);}
    finally{busy.value=false;uploadProgress.value=null;await refreshStorage();}
  })();
  await activeUpload;activeUpload=null;
}
async function cancelUpload(){
  const session=uploadSession.value;if(!session||session.saving||cancelBusy.value)return;
  cancelBusy.value=true;session.cancelled=true;error.value='';
  try{
    if(activeUpload)await activeUpload;
    const current=await accountStorage.read();
    const item=current.items.find(item=>item.storage_id===session.upload_id);
    if(item?.task_id)throw new Error(uiText('资料已加入准备列表，请在空间明细中整理。'));
    storage.value=item?await accountStorage.act('cancel',item):current;
    uploadSession.value=null;pendingFile.value=null;uploadFile.value=null;
  }catch(failure){error.value=failure instanceof Error?failure.message:String(failure);}
  finally{cancelBusy.value=false;}
}
function chooseFile(event:Event){const field=event.target as HTMLInputElement,file=field.files?.[0];field.value='';if(file)void upload(file);}
function chooseNewFile(){resumeItem.value=null;fileInput.value?.click();}
function dropFile(event:DragEvent){dragging.value=false;if(!canChoose.value)return;const files=event.dataTransfer?.files;if(!files?.length)return;if(files.length>1){error.value=uiText('每次添加一份资料，完成后可以继续添加。');return;}resumeItem.value=null;void upload(files[0]);}
function resumeUpload(item:StorageItem){if(blocked.value||uploadSession.value)return;resumeItem.value=item;fileInput.value?.click();}
function storageTaskActive(item:StorageItem){return !!item.task_id&&items.value.some(task=>task.task_id===item.task_id&&['queued_prepare','preparing','queued','running'].includes(task.state));}
async function removeStorage(item:StorageItem){
  if(blocked.value)return;
  const action=!item.task_id?'cancel':'delete';
  if(!window.confirm(action==='cancel'?uiText('取消“{0}”的上传并释放空间？', [item.filename]):uiText('删除“{0}”的私人文件并释放空间？文件删除后无法恢复，请先下载需要保留的内容。', [item.filename])))return;
  storageBusy.value=true;error.value='';
  try{
    storage.value=await accountStorage.act(action,item);
    if(uploadSession.value?.upload_id===item.storage_id){uploadSession.value=null;pendingFile.value=null;uploadFile.value=null;}
    await refresh();emit('ready');
  }catch(failure){error.value=failure instanceof Error?failure.message:String(failure);}
  finally{storageBusy.value=false;}
}
async function act(task:Preparation,action:string){
  busy.value=true;error.value='';
  try{
    const budget=budgets.value[task.task_id]??'';
    if(['start','continue'].includes(action)&&(!/^\d+(\.\d{1,3})?$/.test(budget)||Number(budget)<=0))throw new Error(uiText('请输入大于 0、最多三位小数的额度百分比。'));
    await preparations.act(workspaceId,{task_id:task.task_id,action,expected_revision:task.revision,
      ...(['start','continue'].includes(action)?{period_id:task.allowance?.period_id,max_milli_percent:Math.round(Number(budget)*1000)}:{})});
    await refresh();
  }catch(failure){error.value=failure instanceof Error?failure.message:String(failure);await refresh();}
  finally{busy.value=false;await refreshStorage();}
}
function canStart(task:Preparation){return task.state==='awaiting_confirmation'||task.state==='draft'&&task.materials.every(m=>m.kind==='video');}
onMounted(()=>{void refresh();void refreshStorage();timer=setInterval(()=>{void refresh();},2000);});
onBeforeUnmount(()=>{alive=false;if(uploadSession.value)uploadSession.value.cancelled=true;clearInterval(timer);});
</script>
<template>
  <section class="personal-preparations" :aria-label="uiText('个人资料准备')">
    <div class="preparation-heading"><div><h3>{{ uiText('云端资料与准备') }}</h3><p>{{ uiText('上传到服务器，在网站和其他设备接着学习。') }}</p></div><span class="private-label">{{ uiText('云端副本') }}</span></div>
    <div class="storage-summary" :aria-label="uiText('账号私人空间')">
      <template v-if="storage">
        <div class="storage-heading"><strong>{{ uiText('私人云端空间') }}</strong><span>{{fileSize(storage.total_bytes)}} / {{storage.limit_bytes===null?uiText('待开通套餐'):fileSize(storage.limit_bytes)}}</span></div>
        <p>{{ uiText('仅计算上传的书籍、视频和课件原件；演示页、笔记等学习成果不占上传空间。') }}</p>
        <progress v-if="storage.limit_bytes!==null&&storage.limit_bytes>0" :aria-label="uiText('私人空间占用')" :value="Math.min(storage.total_bytes,storage.limit_bytes)" :max="storage.limit_bytes" />
        <p>{{ uiText('已保存') }} {{fileSize(storage.used_bytes)}} {{ uiText('· 上传预留') }} {{fileSize(storage.reserved_bytes)}}<template v-if="storage.max_file_bytes!==null"> {{ uiText('· 单份最多') }} {{fileSize(storage.max_file_bytes)}}</template></p>
        <p v-if="storage.over_limit" class="storage-warning">{{ uiText('已超过当前套餐空间，请整理资料或选择更高容量套餐后再上传。现有资料仍可在保留期内查看和导出。') }}</p>
        <p v-else-if="!storage.can_upload">{{ uiText('当前不能新增上传，请查看套餐有效期与剩余空间。') }}</p>
        <p v-if="storage.retained_until!==null">{{ uiText('现有私人资料保留至') }} {{allowanceTime(storage.retained_until)}}。</p>
        <div class="actions"><button :aria-expanded="showStorage" @click="showStorage=!showStorage">{{showStorage?uiText('收起空间明细'):uiText('整理空间')}}</button><button :disabled="blocked" @click="refreshStorage">{{ uiText('刷新空间') }}</button><button v-if="!storage.can_upload" @click="emit('allowance')">{{ uiText('查看套餐') }}</button></div>
      </template>
      <p v-else-if="!storageError">{{ uiText('正在读取账号空间…') }}</p>
      <p v-if="storageError" role="alert">{{storageError}} <button :disabled="blocked" @click="refreshStorage">{{ uiText('重新读取空间') }}</button></p>
      <div v-if="showStorage&&storage" class="storage-items">
        <p class="preparation-note">{{ uiText('这里汇总账号所有工作区的私人文件。公共课程不占私人空间；移出或归档工作区不会删除文件。删除前请先暂停相关准备，并将资料移出所有工作区。') }}</p>
        <p v-if="!storage.items.length">{{ uiText('还没有占用私人空间的资料。') }}</p>
        <div v-for="item in storage.items" :key="item.storage_id" class="storage-item">
          <div><strong>{{item.filename}}</strong><span>{{fileSize(item.bytes)}} · {{item.state==='reserved'?uiText('上传预留'):uiText('已保存原件')}}<template v-if="item.workspace_id!==workspaceId"> {{ uiText('· 其他工作区') }}</template></span></div>
          <div class="actions">
            <a v-if="item.download_url" :href="item.download_url" download>{{ uiText('下载') }}</a>
            <button v-if="!item.task_id&&item.workspace_id===workspaceId&&!workspace.archived" :disabled="blocked||!!uploadSession" @click="resumeUpload(item)">{{ uiText('继续上传') }}</button>
            <button :disabled="blocked||storageTaskActive(item)" @click="removeStorage(item)">{{!item.task_id?uiText('取消上传'):uiText('删除文件')}}</button>
            <span v-if="storageTaskActive(item)" class="preparation-note">{{ uiText('请先暂停准备') }}</span>
          </div>
        </div>
      </div>
    </div>
    <div v-if="!workspace.archived" class="upload-zone" :class="{'is-dragging':dragging,'is-uploading':uploadProgress!==null}" role="group" :aria-label="uiText('资料上传区')" @dragover.prevent="dragging=canChoose" @dragleave.self="dragging=false" @drop.prevent="dropFile">
      <input ref="fileInput" class="file-input" :aria-label="uiText('上传个人资料')" type="file" :accept="fileAccept" tabindex="-1" :disabled="blocked" @change="chooseFile">
      <div class="upload-icon" aria-hidden="true"><svg viewBox="0 0 32 32" fill="none"><path d="M18 5H9a2 2 0 0 0-2 2v18a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V12L18 5Z"/><path d="M18 5v7h7M16 23V15m-4 4 4-4 4 4"/></svg></div>
      <div class="upload-copy"><strong>{{dragging?uiText('松开即可添加资料'):uploadProgress!==null?uiText('正在上传资料'):uiText('选择一份资料，或拖到这里')}}</strong><span>{{formatLabels}}</span></div>
      <button class="upload-button" :disabled="!canChoose" @click="chooseNewFile">{{uploadProgress!==null?uiText('正在上传…'):uiText('选择文件')}}</button>
      <p class="preparation-note document-format-note">{{ uiText('PDF 需有文字层；公式、表格和图示可回看来源页。扫描页和纯图片页暂需先补充文字层。课件的 PDF 页对应幻灯片。') }}</p>
      <div v-if="uploadFile" class="upload-file" aria-live="polite"><div><span class="upload-filename">{{uploadFile.name}}</span><span>{{fileSize(uploadFile.size)}}</span></div><span v-if="uploadProgress!==null">{{uploadProgress}}%<template v-if="uploadProgress===100"> {{ uiText('· 正在保存') }}</template></span><span v-else-if="uploaded" class="uploaded-label">{{ uiText('已添加到准备列表') }}</span></div>
      <progress v-if="uploadProgress!==null" class="upload-progress" :aria-label="uiText('资料上传进度')" :value="uploadProgress" max="100" />
      <div v-if="uploadSession" class="upload-actions actions">
        <button v-if="uploadProgress===null" :disabled="blocked" @click="transmit">{{ uiText('继续这次上传') }}</button>
        <button :disabled="cancelBusy||uploadSession.saving" @click="cancelUpload">{{cancelBusy?uiText('正在取消…'):uiText('取消并释放空间')}}</button>
        <span v-if="uploadProgress===null" class="preparation-note">{{ uiText('已确认的上传进度会继续复用，也可以取消后重新选择资料。') }}</span>
      </div>
    </div>
    <p class="preparation-note">{{ uiText('上传后先查看准备方案，再确认额度。视频先准备原文；完成后会回到这个工作区。') }}</p>
    <p v-if="error" role="alert">{{error}}</p>
    <article v-for="task in items" :key="task.task_id">
      <div class="task-heading"><strong>{{task.name}}</strong><span class="task-status" :class="{'is-ready':task.state==='ready'}" role="status">{{states[task.state]??uiText('等待更新')}}</span></div>
      <p>{{task.estimate.message}}</p>
      <p v-if="task.estimate.status==='no_model_cost'">{{ uiText('预计使用额度：0%') }}</p>
      <p v-else-if="task.estimate.status==='estimated'">{{task.estimate.partial?uiText('已能估算部分'):uiText('预计')}}：{{allowancePercent(task.estimate.lower)}} ～ {{allowancePercent(task.estimate.upper)}}<span v-if="task.estimate.partial">{{ uiText('；其余步骤尚无法估算，确认上限仍约束总扣减。') }}</span></p>
      <p v-else>{{ uiText('预计额度：方案生成后计算；没有可靠估算的步骤会保留说明。') }}</p>
      <template v-for="(material,index) in task.materials" :key="index">
        <p v-if="material.progress">{{material.progress.message}} · {{material.progress.completed}}<template v-if="material.progress.total>0"> / {{material.progress.total}}</template></p>
        <progress v-if="material.progress&&material.progress.total>0" :value="material.progress.completed" :max="material.progress.total" />
        <button v-if="task.state==='ready'&&material.published_book_ref" @click="emit('open',material.published_book_ref)">{{ uiText('继续学习') }}</button>
      </template>
      <template v-if="task.allowance">
        <p v-if="task.allowance.unlimited">{{ uiText('内测期间 AI 使用不限额度') }}</p>
        <p v-else>{{ uiText('当前可用') }} {{allowancePercent(task.allowance.available)}} {{ uiText('· 有效至') }} {{allowanceTime(task.allowance.expires_at)}}</p>
        <p v-if="task.allowance.remaining">{{ uiText('任务上限') }} {{allowancePercent(task.allowance.maximum)}} {{ uiText('· 剩余上限') }} {{allowancePercent(task.allowance.remaining)}}</p>
      </template>
      <p v-else>{{ uiText('当前没有可用的额度期。') }}</p>
      <p v-for="period in task.usage_periods" :key="period.period_id">{{ uiText('截至') }} {{allowanceTime(period.expires_at)}} {{ uiText('的额度期：已用') }} {{allowancePercent(period.used)}} {{ uiText('· 预计占用') }} {{allowancePercent(period.reserved)}} {{ uiText('· 待核算') }} {{allowancePercent(period.pending)}}</p>
      <p v-if="task.error">{{ uiText('准备已停在保存的位置。请检查可用额度、任务上限，或联系资料准备服务；继续会复用已完成成果。') }}</p>
      <div v-if="!workspace.archived" class="actions">
        <button v-if="task.state==='draft'&&!canStart(task)||task.state==='interrupted'&&!task.materials.every(m=>m.plan_ready||m.kind==='video')" :disabled="busy" @click="act(task,'prepare')">{{ uiText('生成准备方案') }}</button>
        <template v-if="canStart(task)||task.state==='interrupted'&&task.materials.every(m=>m.plan_ready||m.kind==='video')">
          <label>{{ uiText('本次总上限') }} <input v-model="budgets[task.task_id]" :aria-label="uiText('{0}额度上限', [task.name])" type="number" min="0.001" step="0.001"> %</label>
          <button :disabled="busy||!task.allowance" @click="act(task,task.state==='interrupted'?'continue':'start')">{{task.state==='interrupted'?uiText('确认当前额度并继续'):uiText('确认上限并开始')}}</button>
          <button @click="emit('allowance')">{{ uiText('补充额度') }}</button>
        </template>
        <button v-if="['queued_prepare','preparing','queued','running'].includes(task.state)" :disabled="busy||task.pause_requested" @click="act(task,'pause')">{{task.pause_requested?uiText('正在暂停'):uiText('暂停准备')}}</button>
      </div>
    </article>
  </section>
</template>
<style scoped>
.document-format-note{grid-column:1/-1}
.storage-summary{padding:1rem;margin-bottom:1rem;border:1px solid var(--line,#ddd8ce);border-radius:12px}.storage-heading{display:flex;justify-content:space-between;gap:.75rem;flex-wrap:wrap;font-size:.87rem}.storage-summary>p{margin:.5rem 0;font-size:.8rem}.storage-summary>progress{margin-top:.65rem;max-width:none}.storage-warning{color:#a33825}.storage-items{margin-top:.85rem}.storage-item{display:flex;justify-content:space-between;gap:.8rem;padding:.85rem 0;border-top:1px solid var(--line,#ddd8ce);flex-wrap:wrap}.storage-item>div:first-child{min-width:0;display:grid;gap:.3rem;flex:1 1 14rem;font-size:.82rem}.storage-item>div:first-child>span{font-size:.76rem;color:var(--ink-muted,#686d65)}.storage-item .actions{font-size:.8rem}.storage-item a{color:var(--brand-accent-deep,#91420f)}.upload-actions{grid-column:1/-1}
.personal-preparations{margin:1.2rem 0;padding:1.3rem;border:1px solid var(--line,#ddd8ce);border-radius:16px;background:var(--paper,#fcfaf6);color:var(--ink,#29382e)}h3{margin:0;font-size:1.08rem}p{line-height:1.65;color:var(--ink-muted,#686d65);font-size:.88rem}.preparation-heading{display:flex;justify-content:space-between;align-items:flex-start;gap:1rem}.preparation-heading p{margin:.45rem 0 1.1rem}.private-label{white-space:nowrap;font-size:.73rem;color:var(--ink-muted,#686d65);border:1px solid var(--line,#ddd8ce);border-radius:99px;padding:.25rem .6rem}.upload-zone{position:relative;display:grid;grid-template-columns:auto 1fr auto;align-items:center;gap:1rem;padding:1.35rem;border:1.5px dashed var(--brand-accent-line,#a9bba9);border-radius:12px;background:var(--brand-accent-soft,#f0f5ed);transition:background .15s,border-color .15s}.upload-zone.is-dragging{background:var(--brand-accent-soft-hover,#ecd5c1);border-color:var(--brand-accent,#b65a17)}.file-input{position:absolute;width:1px;height:1px;padding:0;margin:-1px;overflow:hidden;clip:rect(0,0,0,0);white-space:nowrap;border:0}.upload-icon{display:grid;place-items:center;width:48px;height:48px;border-radius:12px;background:var(--paper,#fffdf8);color:var(--brand-accent-deep,#91420f)}.upload-icon svg{width:30px;height:30px;stroke:currentColor;stroke-width:1.6;stroke-linecap:round;stroke-linejoin:round}.upload-copy{min-width:0;display:grid;gap:.35rem}.upload-copy strong{font-size:.93rem;font-weight:600}.upload-copy span{font-size:.77rem;color:var(--ink-muted,#686d65)}button{padding:.55rem .9rem;border:1px solid var(--line,#d5d8ce);border-radius:8px;background:var(--paper,#fffdf8);color:inherit;font:inherit;font-size:.85rem;cursor:pointer}button:focus-visible{outline:2px solid var(--brand-accent,#b65a17);outline-offset:3px}button:disabled{opacity:.5;cursor:default}button.upload-button{background:var(--brand-accent-deep,#91420f);border-color:var(--brand-accent-deep,#91420f);color:#fff;white-space:nowrap;padding:.65rem 1.1rem}.upload-file{grid-column:1/-1;display:flex;justify-content:space-between;gap:.7rem;font-size:.79rem;color:var(--ink-muted,#686d65);border-top:1px solid var(--brand-accent-line,#d0ddcb);padding-top:.8rem}.upload-file>div{display:flex;gap:.7rem;min-width:0;align-items:baseline}.upload-filename{overflow-wrap:anywhere;color:var(--ink,#29382e)}.upload-file>div>span:last-child{white-space:nowrap}.uploaded-label{color:var(--brand-accent-deep,#91420f)}progress{width:100%;max-width:26rem;accent-color:var(--brand-accent,#b65a17)}progress.upload-progress{grid-column:1/-1;max-width:none;height:7px}.preparation-note{margin:.7rem 0 0;font-size:.78rem}article{padding:1.1rem 0 .2rem;border-top:1px solid var(--line,#ddd8ce);margin-top:1.1rem}.task-heading,.actions{display:flex;align-items:center;gap:.75rem;flex-wrap:wrap}.task-heading{justify-content:space-between}strong{overflow-wrap:anywhere}.task-status{font-size:.75rem;padding:.25rem .55rem;border-radius:6px;background:var(--canvas,#f1eee6);color:var(--ink-muted,#686d65)}.task-status.is-ready{background:var(--brand-accent-soft,#edf3e8);color:var(--brand-accent-deep,#91420f)}.actions input{width:5rem}p[role=alert]{color:#a33825}
@media(max-width:560px){.personal-preparations{padding:1rem}.private-label{display:none}.upload-zone{grid-template-columns:auto 1fr;padding:1rem;gap:.75rem}.upload-button{grid-column:1/-1;width:100%}.upload-file,.upload-file>div{flex-wrap:wrap}.upload-copy strong{font-size:.85rem}.upload-copy span{font-size:.7rem}.upload-icon{width:40px;height:44px}.upload-file{gap:.3rem}.task-heading{align-items:flex-start}.task-status{max-width:100%}}
</style>


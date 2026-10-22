import assert from 'node:assert/strict';
import {test} from 'node:test';
import {mkdtempSync,writeFileSync,existsSync} from 'node:fs';
import {tmpdir} from 'node:os';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {createOperatorBuildHost} from '../src/operator-build-host.ts';
import {operatorHarness} from './operator-build-fixture.ts';
import {costFixture} from './external-cost-fixture.ts';

test('two video tasks sharing an output root and material name keep separate frozen workspaces',async()=>{
  const root=mkdtempSync(path.join(tmpdir(),'ub-video-plan-'));
  const h=await operatorHarness({onFirstCommit:async()=>{}});
  const tasks=['first-video','second-video'].map(task_id=>({task_id,action:'prepare',max_micro_cny:100,
    reserved_per_execution_micro_cny:100,materials:[{material_id:'missing-0',label:'Video',material_kind:'video',target_input:path.join(root,task_id+'.mp4'),root_dir:root}]}));
  const plans:any[]=[];
  const fetcher:typeof fetch=async(url,init)=>{
    if(String(url).endsWith('/next'))return Response.json({task:tasks.shift()??null});
    const body=JSON.parse(String(init?.body));if(body.plan)plans.push(body.plan);return Response.json({});
  };
  const host=createOperatorBuildHost(h.ctx,{executable:process.execPath,driverRoot:path.join(root,'registry'),maxOutputTokens:4096,safetyMarginTokens:4096,handoffsPerCall:8},
    'understand-book-executor',{rootSessionId:h.parent.id,connection:{base_url:'http://fixture',origin:'http://fixture',cookie:'fixture',csrf_token:'fixture'},
      video:{codexExecutable:process.execPath,publisherExecutable:process.execPath}},fetcher);
  try{
    await host.processNext();await host.processNext();assert.equal(plans.length,2);
    assert.equal(plans[0].workspace_dir,path.join(root,'first-video','video-missing-0'));
    assert.equal(plans[1].workspace_dir,path.join(root,'second-video','video-missing-0'));
    assert.ok(plans.every(plan=>plan.build_tutor===false));assert.equal(h.model.requests.length,0);
  }finally{await host.dispose();await h.dispose();}
});

test('curated music prepares in the same operator queue without a speech model configuration',async()=>{
  const root=mkdtempSync(path.join(tmpdir(),'ub-music-plan-'));
  const h=await operatorHarness({onFirstCommit:async()=>{}});
  const task={task_id:'music-task',action:'prepare',max_micro_cny:100,reserved_per_execution_micro_cny:100,
    materials:[{material_id:'schubert',label:'舒伯特',material_kind:'music',target_input:path.join(root,'curation.json'),root_dir:root}]};
  const events:any[]=[];let claimed=false;
  const fetcher:typeof fetch=async(url,init)=>{
    if(String(url).endsWith('/next')){if(claimed)return Response.json({task:null});claimed=true;return Response.json({task});}
    const body=JSON.parse(String(init?.body));events.push(body);return Response.json({});
  };
  const host=createOperatorBuildHost(h.ctx,{executable:process.execPath,driverRoot:path.join(root,'registry'),maxOutputTokens:4096,safetyMarginTokens:4096,handoffsPerCall:8},
    'understand-book-executor',{rootSessionId:h.parent.id,connection:{base_url:'http://fixture',origin:'http://fixture',cookie:'fixture',csrf_token:'fixture'}},fetcher);
  try{
    await host.processNext();
    assert.equal(events.at(-1).state,'awaiting_confirmation');
    const plan=events.find(event=>event.plan).plan;
    assert.equal(plan.version,'operator_music_plan.v1');
    assert.equal(plan.workspace_dir,path.join(root,'music-task','music-schubert'));
    assert.equal(plan.target_input,task.materials[0].target_input);
    assert.equal(plan.build_tutor,false);assert.equal(h.model.requests.length,0);
  }finally{await host.dispose();await h.dispose();}
});

test('B1 operator queue prepares without models, resumes the existing Engine and reuses one completed book', {timeout:360000}, async()=>{
  const root=mkdtempSync(path.join(tmpdir(),'ub-b1-operator-'));
  const source=path.join(root,'book.md');writeFileSync(source,'# Transactions\n\nTransactions group operations into one atomic change.\n');
  const costs=await costFixture();
  let claims=0,imports=0,lostProgress=true,lostInvocation='';
  const h=await operatorHarness({assertAdmitted:()=>assert.ok(costs.admissions().length>0,'admission precedes every actual host launch'),onFirstCommit:async()=>{task.pause_requested=true;}});
  const reused={book_id:'existing-book',publication_id:'existing-exact-publication'};
  const task:any={task_id:'operator-two-books',state:'queued_prepare',action:'prepare',confirmation:null,confirmed_at:null,
    max_micro_cny:10000,invocation_max_micro_cny:10000,reserved_per_execution_micro_cny:100,materials:[
      {material_id:'first',label:'Existing book',published_book_ref:reused,state:'reused'},
      {material_id:'second',label:'New book',target_input:source,root_dir:root,pass2:'disabled',max_parallel:3,state:'pending'},
    ]};
  const events:{url:string;body:any}[]=[];
  const fetcher:typeof fetch=async(url,init)=>{
    const route=new URL(String(url)).pathname;const body=init?.body?JSON.parse(String(init.body)):undefined;events.push({url:route,body});
    if(route.endsWith('/next')){claims++;if(!task.state.startsWith('queued'))return Response.json({task:null});task.state=task.action==='prepare'?'preparing':'running';return Response.json({task});}
    if(route.endsWith('/progress')){assert.equal(body.host_session_id,h.parent.id);if(body.state==='running'&&lostProgress){lostProgress=false;lostInvocation=body.invocation_ref;throw new Error('progress response lost');}Object.assign(task.materials[1],body);return Response.json(task);}
    if(route.endsWith('/import')){assert.equal(task.materials[1].state,'complete');assert.equal(body.invocation_ref,task.materials[1].invocation_ref);assert.ok(existsSync(path.join(body.workspace_dir,'book_structure.json')));imports++;task.materials[1].published_book_ref={book_id:'book',publication_id:'new-exact-publication'};return Response.json(task);}
    if(route.endsWith('/finish')){task.state=body.state;task.error=body.error;return Response.json(task);}
    return Response.json(task);
  };
  const config={executable:process.env.UNDERSTAND_BOOK_TEST_NODE!,prefixArgs:['--import','tsx',fileURLToPath(new URL('engine-source-cli.ts',import.meta.url))],driverRoot:path.join(root,'registry'),maxOutputTokens:4096,safetyMarginTokens:4096,handoffsPerCall:24};
  let host=createOperatorBuildHost(h.ctx,config,'understand-book-executor',{connection:costs.config.connection,rootSessionId:h.parent.id},fetcher);
  try{
    assert.equal(await host.processNext(),true);assert.equal(task.state,'awaiting_confirmation');assert.equal(h.model.requests.length,0);assert.equal(costs.admissions().length,0);
    assert.ok(task.materials[1].plan.review_markdown.includes('本次构建计划'));assert.deepEqual(task.materials[0].published_book_ref,reused);
    assert.deepEqual(task.materials[1].plan.plan.public_stage_closure,['pass1','profile_sidecar','book_structure']);
    assert.ok(task.materials[1].plan.plan.excluded.some((item:any)=>item.artifact==='public.teaching_publish'));
    task.state='queued';task.action='run';task.confirmation='operator-explicit-confirmation';task.confirmed_at=1792281600;
    await host.processNext();assert.equal(task.state,'interrupted');assert.equal(task.materials[1].invocation_ref,undefined);assert.equal(h.model.requests.length,0);
    task.state='queued';task.max_micro_cny=20000;costs.budgetLimit(20000);
    await host.processNext();assert.equal(task.state,'interrupted',JSON.stringify(task.error));const invocation=task.materials[1].invocation_ref;assert.ok(invocation);assert.equal(imports,0);
    assert.equal(invocation,lostInvocation,'a lost invocation response plus a larger budget keeps the original creation identity');
    await host.dispose();host=createOperatorBuildHost(h.ctx,config,'understand-book-executor',{connection:costs.config.connection,rootSessionId:h.parent.id},fetcher);
    task.state='queued';task.pause_requested=false;
    await host.processNext();assert.equal(task.state,'ready',JSON.stringify(task.error));assert.equal(task.materials[1].invocation_ref,invocation);assert.equal(imports,1);assert.equal(h.pass1Generations(),1);
    assert.deepEqual(task.materials[0].published_book_ref,reused);assert.equal(claims,4);
    assert.ok(costs.calls.filter(c=>c.path.endsWith('/admit')).every(c=>c.path.includes('/operator-two-books/')));
    assert.equal(costs.admissions().length,costs.receipts().length);assert.ok(costs.receipts().every(r=>r.status==='partial'));
    assert.equal(events.filter(e=>e.url.endsWith('/progress')&&e.body.state==='planned').length,1);
    console.log(JSON.stringify({root,pass1Generations:h.pass1Generations(),actualExecutions:costs.admissions().length,materials:2,reused:1,imports,final:task.state}));
  }finally{await host.dispose();await h.dispose();await costs.close();}
});

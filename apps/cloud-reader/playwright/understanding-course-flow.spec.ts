import { expect, test } from '@playwright/test';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const authored=JSON.parse(readFileSync(resolve('../../docs/examples/inquiry-course/ai-infra-resource-bounds.course.json'),'utf8')).lessons[0];
for(const width of [1366,390])test(`short support and learner questions retain the original activity at ${width}px`,async({page},testInfo)=>{
  const errors:string[]=[];page.on('pageerror',e=>errors.push(e.stack??e.message));
  const workspace={learning_workspace_id:'flow',goal:{text:'理解计算与读取的时间关系',constraints:[],revision:1,created_at:1},goal_history:[],members:[],archived:false,created_at:1,updated_at:1};
  const expression={fact_ref:'reflection',job_ref:'review',activity:'reflection',response:'读取和计算重叠，我还是不明白为什么不能忽略读取。',round_index:0,round_title:'找出真正的瓶颈'};
  const branch={focus:'同时开始，与全部完成',explanation:'同时进行表示两个过程可以重叠，不表示其中一个过程消失。要完成这次任务，计算和读取都必须结束；较早完成的一项，需要等待另一项。',example:'计算要 5 ms，读取要 8 ms。如果它们同时开始，5 ms 时计算已完成，读取还需要 3 ms；两项都完成时已过去 8 ms。',return_prompt:'回到原来的问题：读取比计算更慢时，为什么不能把它忽略？'};
  const feedback={response_quote:expression.response,acknowledgement:'你已经在区分两项时间，卡在“同时进行”的含义上。',guidance:'先补一下开始与完成的区别，再回到眼前这组数据。',follow_up:'哪一项更晚完成？',support_branch:branch};
  let lesson:any={...authored,progress:{run_id:'run',release_id:'release',release_version:1,lesson_id:authored.summary.id,revision:4,goal_revision:1,stage:'explanation',status:'active'},prediction_response:{choice:authored.prediction.options[0].id,reason:'重叠之后，也许只需要看计算。'},preset:'baseline',observed:['baseline','double_compute','double_bandwidth'],answer:null,hint:null,solution:null,assessment_status:null,support_delivered:[],source_excerpt:null,exploration:authored.lab,expressions:[expression],reviews:[{expression,status:'ready',displayed:true,feedback,understanding:[],error:null}],round_index:0,round_titles:['找出真正的瓶颈'],round_history:[],decision:null,activity_response:null,inquiry_kind:null,resolved_questions:[]};
  await page.route('**/api/learning-workspaces/flow/shared-release/course**',async route=>{
    if(route.request().method()==='POST'){
      const action=route.request().postDataJSON().action;
      if(action.kind==='open_support')lesson.support_visit={job_ref:'review',original_response:expression.response,branch};
      if(action.kind==='return_from_support')delete lesson.support_visit;
      if(action.kind==='ask_question'){
        const question={fact_ref:'q',job_ref:'q-review',activity:'question',intent:'question',response:action.response,round_index:0};
        lesson.expressions.push(question);lesson.reviews.push({expression:question,status:'ready',displayed:true,understanding:[],error:null,feedback:{response_quote:action.response,acknowledgement:'这个问题可以通过比较两个完成时刻来探索。',guidance:'画两条从零开始的时间线，看看什么时候两条都结束。',follow_up:null,question_guide:{suggested_question:'只缩短较快的一项，全部完成的时刻会变吗？',why:'固定另一项，就能看出是谁限制了结束时刻。',try_next:'保持读取 8 ms，把计算从 5 ms 调整为 3 ms，再比较总时间。'}}});
      }
      lesson.progress.revision++;return route.fulfill({json:lesson});
    }
    return route.fulfill({json:new URL(route.request().url()).searchParams.has('run_id')?lesson:{release_id:'release',release_version:1,course:{language:'zh',units:[],routes:[],lessons:[authored.summary]},progress:[lesson.progress]}});
  });
  await page.route('**/study-flow-preview',route=>route.fulfill({contentType:'text/html',body:`<!doctype html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"></head><body><div id="app"></div><script type="module">
    import {createApp,h,ref,onMounted} from '/node_modules/.vite/deps/vue.js';import Player from '/reader-src/components/InquiryCoursePlayer.vue';import Entry from '/reader-src/components/UnderstandingEntry.vue';import {installIdentity} from '/src/network-context.ts';import '/src/network-client.ts';import '/reader-src/style.css';
    installIdentity({user_id:'flow-reader',csrf_token:'test'});
    createApp({setup(){const player=ref();onMounted(()=>player.value.openLesson(${JSON.stringify(authored.summary.id)}));return()=>h('main',{style:'height:100%;overflow:auto;max-width:1200px;margin:auto;padding:24px 20px;background:var(--canvas)'},[h(Entry,{workspaceId:'flow'}),h(Player,{ref:player,workspace:${JSON.stringify(workspace)},embedded:true})]);}}).mount('#app');</script></body></html>`}));
  await page.setViewportSize({width,height:width>1000?960:844});await page.goto('/study-flow-preview');
  await expect(page.locator('#activity-reply')).toBeVisible();
  await page.locator('.understanding-entry').screenshot({path:testInfo.outputPath(`entry-${width}.png`)});
  await page.locator('#activity-reply').fill('我的原回答草稿：两项都完成才算结束');
  await page.locator('.support-link').click();
  await expect(page.getByRole('region',{name:'短补学'})).toBeVisible();
  await page.locator('.support-branch').screenshot({path:testInfo.outputPath(`support-${width}.png`)});
  await page.getByRole('button',{name:'回到原问题，接着想 →',exact:true}).click();
  await expect(page.locator('#activity-reply')).toHaveValue('我的原回答草稿：两项都完成才算结束');
  await page.getByRole('button',{name:'＋ 记下一个疑问',exact:true}).click();
  await page.getByRole('button',{name:'不知道怎么问？试着补三句话',exact:true}).click();
  await page.getByPlaceholder('某段解释、某组数据或某个现象').fill('计算和读取同时进行');
  await page.getByPlaceholder('预期与实际有什么不同').fill('为什么取较大值');
  await page.getByPlaceholder('原因、条件，或者改一个量会怎样').fill('只缩短计算会发生什么');
  await page.getByRole('button',{name:'放进问题框，再改一改',exact:true}).click();
  await page.getByRole('button',{name:'带着这个问题探索 →',exact:true}).click();
  await expect(page.locator('.question-guide')).toBeVisible();
  await page.locator('.activity-tutor').screenshot({path:testInfo.outputPath(`question-${width}.png`)});
  await page.getByRole('button',{name:'用这个问法继续，可以修改 →',exact:true}).click();
  await expect(page.locator('#activity-reply')).toHaveValue('只缩短较快的一项，全部完成的时刻会变吗？');
  await expect(page.locator('.activity-tutor').getByRole('button',{name:'提出我的疑问',exact:true})).toBeVisible();
  await expect(page.locator('.course-questions li')).toContainText('为什么取较大值');
  expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);expect(errors).toEqual([]);
});

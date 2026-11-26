import { expect, test } from '@playwright/test';
import { mkdirSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';

// Actual isolated Rust HTTP service, formal AI Infra course, real private storage.
// Start inquiry_course_browser_host with COURSE_BOOK_DIR and the formal two-lesson
// ai-infra-resource-objects.unit.course.json COURSE_FILE before running this spec.
const output=resolve(process.env.LEARNING_ENTRY_OUTPUT ?? '../../docs/evidence/learning-collections-20261116/browser');
test('a book contains its units and preserves the chosen lesson across reading, home and refresh', async({page,context})=>{
  page.setDefaultTimeout(25_000); mkdirSync(output,{recursive:true});
  const fixture=await page.request.get('http://127.0.0.1:4261/fixture').then(r=>r.json());
  const headers={origin:fixture.origin,cookie:fixture.new_reader_cookie,'X-CSRF-Token':fixture.new_reader_csrf};
  const api=async(path:string)=>{
    const response=await page.request.get(`http://127.0.0.1:4260/api${path}`,{headers});
    expect(response.ok(),await response.text()).toBe(true); return response.json();
  };
  const spacesBefore=await api('/learning-workspaces');
  expect(spacesBefore.workspaces).toHaveLength(0);
  const errors:string[]=[]; page.on('pageerror',e=>errors.push(e.message));
  const requests:{method:string;path:string;duration_ms?:number;status?:number}[]=[];
  let rejectOpen=true;
  let selectionResponse:Promise<void>|null=null;
  let finishSelection:(()=>void)|null=null;
  let selectionWaiting=false;
  await page.addInitScript(()=>{
    for(const surface of ['library','reader'])localStorage.setItem(`understand-book.product-tour.v1.B.${surface}`,'skipped');
  });
  await context.route(/\/api\//,async route=>{
    const request=route.request(),url=new URL(request.url());
    const timing={method:request.method(),path:url.pathname,duration_ms:0,status:0};
    requests.push(timing); const started=performance.now();
    if(url.pathname==='/api/workspaces' && request.method()==='POST' && rejectOpen){
      rejectOpen=false;
      await route.fulfill({status:503,json:{error_code:'WORKSPACE_CAPACITY',category:'unavailable',message:'The operation could not be completed'}});
      timing.status=503;timing.duration_ms=Math.round(performance.now()-started);return;
    }
    const response=await route.fetch({url:`http://127.0.0.1:4260${url.pathname}${url.search}`,headers:{...await request.allHeaders(),host:'127.0.0.1:4260',...headers}});
    timing.status=response.status();timing.duration_ms=Math.round(performance.now()-started);
    if(url.pathname.endsWith('/learning-task') && request.method()==='POST' && selectionResponse){
      selectionWaiting=true;await selectionResponse;selectionWaiting=false;
    }
    await route.fulfill({response});
  });
  const course=page.locator('.course-reader .inquiry-course');
  const mainNav=()=>page.getByRole('navigation',{name:'主导航',exact:true}).filter({visible:true});
  const act=async(name:string)=>{await course.getByRole('button',{name,exact:true}).click();await expect(course).toHaveAttribute('aria-busy','false');};
  try{
    await page.goto('/?lang=zh-CN&view=learning');
    await expect(page.locator('.learning-home-task')).toContainText('选一本书，或开始一套课程');
    const bookCard=page.locator('.collection-card[data-kind="book"]').filter({hasText:'已提供'}).first();
    await expect(bookCard).toContainText('2 节课');
    const bookTitle=await bookCard.locator('h3').innerText();
    await expect(page.locator('.learning-create')).toBeHidden();
    await page.screenshot({path:resolve(output,'desktop-learning-home.png'),fullPage:true});
    await page.setViewportSize({width:390,height:844});
    await page.screenshot({path:resolve(output,'mobile-learning-home.png'),fullPage:true});
    expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
    await page.setViewportSize({width:1440,height:1050});
    await bookCard.getByRole('button',{name:'查看这本书',exact:true}).click();
    await expect(page.locator('.shared-catalog h2')).toHaveText(bookTitle);
    await expect(page.locator('.course-unit .lesson-row')).toHaveCount(2);
    await expect(page.locator('.accepted-goal')).not.toBeEmpty();
    const bookUrl=page.url();
    expect(new URL(bookUrl).searchParams.get('collection_book')).toBe(fixture.reference.book_id);
    await page.reload();
    await expect(page.locator('.shared-catalog h2')).toHaveText(bookTitle);
    await expect(page.locator('.course-unit .lesson-row')).toHaveCount(2);
    expect((await api('/learning-workspaces')).workspaces).toHaveLength(0);
    expect(requests.filter(r=>r.path.endsWith('/use'))).toHaveLength(0);
    expect(requests.filter(r=>r.path==='/api/workspaces'&&r.method==='POST')).toHaveLength(0);
    await page.screenshot({path:resolve(output,'desktop-book-home.png'),fullPage:true});
    await page.setViewportSize({width:390,height:844});
    await page.screenshot({path:resolve(output,'mobile-book-home.png'),fullPage:true});
    expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
    await page.locator('.book-home-intro').getByRole('button',{name:'跟着课程学',exact:true}).click();
    await expect(page.locator('.course-unit > header')).toBeInViewport();
    const duration=page.locator('.lesson-row').first().locator('details');
    await expect(duration).not.toHaveAttribute('open');
    await duration.getByText('课时说明',{exact:true}).click();
    await expect(duration).toHaveAttribute('open');
    await expect(duration.locator('p')).toBeVisible();
    await duration.getByText('课时说明',{exact:true}).click();
    await page.locator('.course-unit > header').evaluate(element=>element.scrollIntoView({block:'start'}));
    await page.screenshot({path:resolve(output,'mobile-book-units.png')});
    await page.setViewportSize({width:1440,height:1050});
    await page.locator('.course-unit > header').evaluate(element=>element.scrollIntoView({block:'start'}));
    await page.screenshot({path:resolve(output,'desktop-book-units.png')});
    await page.locator('.lesson-row[data-lesson="batch-reuse-and-wait"]').getByRole('button',{name:'开始本课',exact:true}).click();
    await expect(page.locator('.book-opening')).toContainText('当前阅读窗口已达到上限');
    await page.getByRole('button',{name:'重试打开',exact:true}).click();
    await expect(course.getByRole('heading',{name:'先认识这个问题',exact:true})).toBeVisible();
    await expect(course.locator('.collection-breadcrumb')).toContainText('一批生成八个 token');
    await expect(page.locator('.book-opening')).toHaveCount(0);
    const chosen=(await api('/me/learning-tasks')).tasks.find((task:any)=>task.lesson_id==='batch-reuse-and-wait');
    expect(chosen.course_context.lesson_number).toBe(2);
    expect(chosen.course_context.lesson_count).toBe(2);
    await course.getByRole('button',{name:'返回书籍学习首页',exact:true}).click();
    await expect(page.locator('.shared-catalog h2')).toHaveText(bookTitle);
    expect((await api('/me/learning-tasks')).tasks.find((task:any)=>task.lesson_id==='batch-reuse-and-wait').status).toBe('paused');
    await page.locator('.lesson-row[data-lesson="compute-or-bandwidth"]').getByRole('button',{name:'开始本课',exact:true}).click();
    await expect(course.getByRole('heading',{name:'先认识这个问题',exact:true})).toBeVisible();
    await expect(course.locator('.collection-breadcrumb')).toContainText(bookTitle);
    await expect(course.locator('.collection-breadcrumb')).toContainText('生成任务要在 9 ms 内完成');
    await expect(course.locator('.current-task')).toContainText('本课目标');
    await expect(course.getByRole('button',{name:'我来解释一下',exact:true})).toHaveCount(0);
    await expect(page.locator('.book-opening')).toHaveCount(0);
    await page.screenshot({path:resolve(output,'desktop-current-task.png')});
    await act('做出我的预测 →');
    const reason='我先比较计算与读取的时间，再判断只增加算力是否有效。';
    await course.getByLabel('未必，要先比较计算与读取各自需要的时间。',{exact:true}).check();
    await course.locator('#course-reason').fill(reason);
    await act('暂停，稍后继续');
    await mainNav().getByRole('button',{name:'学习',exact:true}).click();
    await expect(course).toBeHidden();
    await expect(page.locator('.network-shell > .app')).toBeHidden();
    const saved=(await api('/me/learning-tasks')).tasks.find((task:any)=>task.lesson_id==='compute-or-bandwidth');
    expect(saved.status).toBe('paused'); expect(saved.stage).toBe('prediction');
    expect(saved.chat_session_id).toBeTruthy();expect(saved.course_run_id).toBeTruthy();
    await expect(page.locator('.learning-home-task')).toContainText('已暂停');
    await expect(page.locator('.learning-home-task .task-collection')).toHaveText(bookTitle);
    await expect(page.locator('.learning-home-task .task-course-position')).toContainText('第 1 课 · 共 2 课');
    await page.screenshot({path:resolve(output,'desktop-continue.png')});
    await page.reload();
    await page.setViewportSize({width:390,height:844});
    await expect(page.locator('.learning-home-task')).toContainText(saved.title);
    await page.screenshot({path:resolve(output,'mobile-continue.png'),fullPage:true});
    // Keep the final persistence response pending while the restored lesson has rendered.
    // It must not accept course actions that the shell would discard as still opening.
    selectionResponse=new Promise<void>(resolve=>{finishSelection=resolve;});
    await page.locator('.learning-home-task .task-primary').click();
    await expect(course.locator('#course-reason')).toBeVisible();
    await expect(course.locator('#course-reason')).toHaveValue(reason);
    await expect(course.locator('.pause-card')).toHaveCount(0);
    await expect.poll(()=>selectionWaiting).toBe(true);
    const reader=page.locator('.network-shell > .app');
    await expect(reader).toHaveAttribute('inert','');
    await expect(reader).toHaveAttribute('aria-busy','true');
    const cancelOpening=page.locator('.book-opening').getByRole('button',{name:'取消',exact:true});
    await expect(cancelOpening).toBeEnabled();
    await cancelOpening.focus();await expect(cancelOpening).toBeFocused();
    expect(await page.locator('.book-opening').evaluate(opening=>{
      const launcher=document.querySelector('.thought-toolbar');if(!launcher)return true;
      const a=opening.getBoundingClientRect(),b=launcher.getBoundingClientRect();
      const left=Math.max(a.left,b.left),right=Math.min(a.right,b.right),top=Math.max(a.top,b.top),bottom=Math.min(a.bottom,b.bottom);
      return left>=right||top>=bottom||opening.contains(document.elementFromPoint((left+right)/2,(top+bottom)/2));
    })).toBe(true);
    await page.screenshot({path:resolve(output,'mobile-restoring-task.png')});
    finishSelection!();selectionResponse=null;
    await expect(page.locator('.book-opening')).toHaveCount(0);
    await expect(reader).not.toHaveAttribute('inert');
    const returnToBook=course.getByRole('button',{name:'返回书籍学习首页',exact:true});
    await returnToBook.focus();await expect(returnToBook).toBeFocused();
    const resumed=(await api('/me/learning-tasks')).tasks.find((task:any)=>task.lesson_id==='compute-or-bandwidth');
    expect(resumed.course_run_id).toBe(saved.course_run_id);expect(resumed.chat_session_id).toBe(saved.chat_session_id);
    await page.screenshot({path:resolve(output,'mobile-resumed-prediction.png')});
    await course.getByRole('button',{name:'返回书籍学习首页',exact:true}).click();
    await expect(page.locator('.shared-catalog h2')).toHaveText(bookTitle);
    await expect(page.locator('.lesson-row[data-lesson="compute-or-bandwidth"]')).toContainText('继续本课');
    await page.locator('.book-home-intro').getByRole('button',{name:'阅读原文',exact:true}).click();
    await expect(page.locator('.book-opening')).toHaveCount(0);
    await expect(page.locator('.network-shell > .app')).toBeVisible();
    await expect(course).toBeHidden();
    await expect(page.locator('main.reader-pane').first()).toBeVisible();
    await expect(page.getByRole('navigation',{name:'书内学习',exact:true})).toHaveCount(0);
    await page.screenshot({path:resolve(output,'mobile-book-reading.png')});
    await page.getByRole('button',{name:'更多阅读工具',exact:true}).click();
    await expect(page.getByRole('button',{name:'书籍学习首页',exact:true})).toBeVisible();
    await expect(page.getByRole('button',{name:'本书课程 →',exact:true})).toBeVisible();
    await page.screenshot({path:resolve(output,'mobile-book-reading-tools.png')});
    await page.getByRole('button',{name:'书籍学习首页',exact:true}).click();
    await expect(page.locator('.shared-catalog h2')).toHaveText(bookTitle);
    await page.setViewportSize({width:1440,height:1050});
    await page.locator('.book-home-intro').getByRole('button',{name:'阅读原文',exact:true}).click();
    await expect(page.locator('main.reader-pane').first()).toBeVisible();
    await expect(page.locator('.book-opening')).toHaveCount(0);
    await expect(page.getByRole('navigation',{name:'书内学习',exact:true})).toHaveCount(0);
    await page.screenshot({path:resolve(output,'desktop-book-reading.png')});
    await page.getByRole('button',{name:'更多阅读工具',exact:true}).click();
    await expect(page.getByRole('button',{name:'书籍学习首页',exact:true})).toBeVisible();
    await expect(page.getByRole('button',{name:'本书课程 →',exact:true})).toBeVisible();
    await page.screenshot({path:resolve(output,'desktop-book-reading-tools.png')});
    await page.keyboard.press('Escape');
    const afterReading=(await api('/me/learning-tasks')).tasks.find((task:any)=>task.course_run_id===saved.course_run_id);
    expect(afterReading.chat_session_id).toBe(saved.chat_session_id);
    expect(afterReading.status).toBe('paused');
    await mainNav().getByRole('button',{name:'学习',exact:true}).click();
    await expect(page.locator('.learning-home-task .task-collection')).toHaveText(bookTitle);
    await page.locator('.learning-home-task .task-collection').click();
    await expect(page.locator('.shared-catalog h2')).toHaveText(bookTitle);
    await page.reload();
    await expect(page.locator('.lesson-row[data-lesson="compute-or-bandwidth"]')).toContainText('继续本课');
    await page.setViewportSize({width:320,height:780});
    expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
    await page.screenshot({path:resolve(output,'small-book-home.png'),fullPage:true});
    await page.goto('/?view=learning&lang=en');
    await expect(page.locator('.learning-home-task')).toContainText('Continue');
    await expect(page.locator('.learning-home-task')).toContainText('Lesson 1 of 2');
    await page.screenshot({path:resolve(output,'small-english-home.png'),fullPage:true});
    await page.setViewportSize({width:1440,height:1050});
    await page.screenshot({path:resolve(output,'desktop-english-home.png'),fullPage:true});
    await page.locator('.learning-home-task .task-collection').click();
    await expect(page.locator('.book-home-intro').getByRole('button',{name:'Read source text',exact:true})).toBeVisible();
    await page.screenshot({path:resolve(output,'desktop-english-book-home.png'),fullPage:true});
    await page.goto('/?view=learning&lang=en');
    await page.locator('.learning-home-task .task-primary').click();
    await expect(course.locator('.current-task')).toBeVisible();
    await expect(course.locator('#course-reason')).toHaveValue(reason);
    await expect(page.locator('.book-opening')).toHaveCount(0);
    const after=(await api('/me/learning-tasks')).tasks.find((task:any)=>task.lesson_id==='compute-or-bandwidth');
    expect(after.course_run_id).toBe(saved.course_run_id);
    expect((await api('/learning-workspaces')).workspaces).toHaveLength(1);
    expect(requests.filter(r=>r.path.endsWith('/use'))).toHaveLength(1);
    expect(requests.filter(r=>r.path==='/api/agent/runs'&&r.method==='POST')).toHaveLength(0);
    expect(errors).toEqual([]);
    writeFileSync(resolve(output,'acceptance.json'),JSON.stringify({book_title:bookTitle,readonly_book_browsing:true,book_deep_link_refresh:true,selected_second_lesson:true,opening_retry_same_lesson_and_instance:true,restoring_task_blocks_reader_actions:true,opening_cancel_focusable:true,reader_focus_available_after_opening:true,opening_status_unobscured:true,return_to_book_pauses:true,reading_original_not_saved_course:true,original_run:saved.course_run_id,original_chat:saved.chat_session_id,prediction_draft_restored:true,existing_instance_reused:true,course_navigation_model_requests:0,widths:[1440,390,320],languages:['zh-CN','en'],errors},null,2));
  }catch(error){
    await page.screenshot({path:resolve(output,'failure.png'),fullPage:true});
    writeFileSync(resolve(output,'failure-context.json'),JSON.stringify({errors,requests},null,2));
    writeFileSync(resolve(output,'failure.txt'),String(error)+'\n'+await page.locator('body').innerText());throw error;
  }finally{
    finishSelection?.();
    writeFileSync(resolve(output,'request-log.json'),JSON.stringify(requests,null,2));
    await context.unrouteAll({behavior:'wait'});
  }
});

test('representative complete-course overview keeps units and related books readable', async({page,context})=>{
  // This is a layout fixture, not evidence of cross-book backend execution.
  // Use formal lesson summaries, with a second source and unit for the overview.
  mkdirSync(output,{recursive:true});
  const fixture=await page.request.get('http://127.0.0.1:4261/fixture').then(r=>r.json());
  const headers={origin:fixture.origin,cookie:fixture.new_reader_cookie,'X-CSRF-Token':fixture.new_reader_csrf};
  const read=async(path:string)=>page.request.get(`http://127.0.0.1:4260/api${path}`,{headers}).then(r=>r.json());
  const release=await read('/workspace-releases/exam-course/versions/1');
  const library=await read('/library');
  const book=library.books.find((book:any)=>book.published_book_ref.book_id===fixture.reference.book_id);
  expect(book).toBeTruthy();
  const other={book_id:'visual-service-design',publication_id:'visual-service-design-v1'};
  library.books.push({...book,title:'模型服务设计：从批处理到请求延迟',published_book_ref:other});
  release.release_id='visual-cross-book-course';
  release.manifest.name='从资源下界到服务性能：一套跨书学习课程';
  release.manifest.materials.push({published_book_ref:other,required_capabilities:['text']});
  release.manifest.inquiry_course.units.push({id:'service-design',title:'第 2 单元 · 批处理与请求等待'});
  release.manifest.inquiry_course.lessons[1].unit_id='service-design';
  release.manifest.inquiry_course.lessons[1].source.publication=other;
  const writes:string[]=[];
  await page.addInitScript(()=>{
    for(const surface of ['library','reader'])localStorage.setItem(`understand-book.product-tour.v1.B.${surface}`,'skipped');
  });
  await context.route(/\/api\//,async route=>{
    const request=route.request(),url=new URL(request.url());
    if(request.method()!=='GET'&&url.pathname!=='/api/account/presence')writes.push(url.pathname);
    if(url.pathname==='/api/library'){await route.fulfill({json:library});return;}
    if(url.pathname==='/api/workspace-releases/visual-cross-book-course/versions/1'){await route.fulfill({json:release});return;}
    const response=await route.fetch({url:`http://127.0.0.1:4260${url.pathname}${url.search}`,headers:{...await request.allHeaders(),host:'127.0.0.1:4260',...headers}});
    await route.fulfill({response});
  });
  try{
    for(const lang of ['zh-CN','en']){
      await page.goto(`/?view=learning&collection_release=visual-cross-book-course&collection_version=1&lang=${lang}`);
      await expect(page.locator('.shared-catalog h2')).toHaveText(release.manifest.name);
      await expect(page.locator('.course-unit')).toHaveCount(2);
      await expect(page.locator('.associated-books button')).toHaveCount(2);
      await expect(page.locator('.book-home-intro')).toHaveCount(0);
      for(const width of [1440,390,320]){
        await page.setViewportSize({width,height:width===1440?1050:844});
        expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
        await page.screenshot({path:resolve(output,`course-overview-${lang}-${width}.png`),fullPage:true});
      }
      await page.getByRole('button',{name:lang==='en'?'Choose a lesson to start':'选择一课开始',exact:true}).click();
      await expect(page.locator('.course-unit > header').first()).toBeInViewport();
      await page.screenshot({path:resolve(output,`course-units-${lang}-320.png`)});
    }
    expect(writes).toEqual([]);
  }finally{await context.unrouteAll({behavior:'wait'});}
});

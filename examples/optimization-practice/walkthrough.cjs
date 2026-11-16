// Run against a fresh, isolated local reader seeded by seed_optimization_question_walkthrough.
const { chromium, expect } = require('@playwright/test');
const { mkdir, writeFile } = require('node:fs/promises');
const { join } = require('node:path');
const assert = require('node:assert/strict');

async function main() {
  const [url, output, mode] = process.argv.slice(2);
  assert(url && output, 'Pass the URL of the isolated reader and an output directory.');
  await mkdir(output, { recursive: true });
  const browser = await chromium.launch();
  const mobile = mode === 'mobile';
  const page = await browser.newPage(mobile ? {viewport:{width:390,height:844},isMobile:true,hasTouch:true} : { viewport: { width: 1440, height: 960 } });
  page.setDefaultTimeout(120000);
  const card = id => page.locator('article[data-delivery-ref$=":' + id + '"]');
  const report = { questions: [], steps: [], grades: [] };
  async function action(button, endpoint) {
    const [response] = await Promise.all([
      page.waitForResponse(r => r.request().method() === 'POST' && new URL(r.url()).pathname === '/api/tutor/' + endpoint),
      button.click(),
    ]);
    const value = await response.json();
    assert(response.ok(), JSON.stringify(value));
    return { request: response.request().postDataJSON(), value };
  }
  async function choose(id, index, revised, status) {
    const q = card(id); await q.locator('.options label').nth(index).click();
    const { request, value } = await action(q.getByRole('button', { name: revised ? '提交改答' : '提交选项' }), 'action');
    assert.equal(value.assessment, status); report.questions.push({ id, submitted: request.response, status });
    await expect(q.locator('.attempts summary')).toContainText(revised ? '2' : '1', { timeout: 120000 });
  }
  async function ask(id, thought, selected) {
    const q = card(id), editor = q.locator('.question-work textarea');
    const attemptsBefore = await q.locator('.attempts summary').allTextContents();
    await editor.fill(thought);
    if (selected) await editor.evaluate((el, line) => {
      const offset = el.value.indexOf(line); el.setSelectionRange(offset, offset + line.length);
      el.dispatchEvent(new Event('select', { bubbles: true }));
    }, selected);
    const { request, value } = await action(q.locator('.ask'), 'ask-step');
    assert.equal(request.text, selected ?? thought);
    assert(!Object.hasOwn(request, 'draft'));
    const reply = q.locator('[data-step-help]').filter({ hasText: value.text.split('$')[0].slice(0,8) }).last();
    await expect(reply).toBeVisible({ timeout: 120000 });
    await reply.scrollIntoViewIfNeeded();
    await expect(editor).toHaveValue(thought);
    assert.deepEqual(await q.locator('.attempts summary').allTextContents(), attemptsBefore);
    report.steps.push({ id, text: request.text, reply: value.text });
  }
  async function grade(text, revised, status) {
    const q = card('boundary'); await q.locator('textarea').fill(text);
    const { value } = await action(q.getByRole('button', { name: revised ? '提交改答' : '提交解答' }), 'grade');
    assert.equal(value.assessment, status);
    const feedback = q.locator('ul[data-feedback-ref="' + value.action_ref + '"] li');
    await expect(feedback).toHaveCount(3, { timeout: 120000 });
    const reasons = await feedback.allTextContents();
    if (status === 'partial') assert(!reasons.join(' ').match(/x\s*=\s*3/), 'A missing optimizer was disclosed');
    report.grades.push({ answer: text, status, feedback: reasons });
  }
  try {
    await page.goto(url, { waitUntil: 'domcontentloaded' });
    await card('boundary').waitFor({state:'attached'});
    const tour = page.getByRole('button', { name: '跳过引导' });
    if (await tour.isVisible()) await tour.click();
    const ready = card('bounds').getByRole('button', { name: '提交选项' });
    await expect(ready).toBeDisabled();
    if (mobile) {
      const calc = card('boundary');
      const draft = '我知道 x≥3，但不确定从约束的哪一端开始看。';
      await ask('boundary', draft);
      await calc.locator('.question-work').screenshot({path:join(output,'mobile.png')});
      await page.reload({waitUntil:'domcontentloaded'});
      await expect(calc.locator('textarea')).toHaveValue(draft,{timeout:120000});
      await grade('4', false, 'partial');
      await grade('可行域 x≥3 上，x−1≥2，所以 (x−1)²≥4。x=3 时取到等号，因此最小值是 4，取得点是 x=3。', true, 'correct');
      assert(await page.evaluate(()=>document.documentElement.scrollWidth <= innerWidth + 1));
      await writeFile(join(output,'mobile.json'),JSON.stringify(report,null,2)); return;
    }
    await choose('bounds', 0, false, 'incorrect');
    await choose('bounds', 1, true, 'correct');
    await ask('certificates', '两个 8 来自不同的地方，我怎么判断它们是否足以说明最优？');
    await choose('certificates', 0, false, 'partial');
    await choose('certificates', 1, true, 'correct');
    const first = '我令导数为零得到 x=1，所以最小值就是 0 吗？';
    const draft = '先写下可行条件 x≥3。\n' + first + '\n这一行只是我的草稿。';
    await ask('boundary', draft, first);
    await card('boundary').locator('.question-work').screenshot({ path:join(output,'desktop.png') });
    await page.reload({ waitUntil:'domcontentloaded' });
    const calc = card('boundary'), editor = calc.locator('.question-work textarea');
    await expect(editor).toHaveValue(draft, {timeout:120000});
    await expect(calc.locator('.exchange')).toHaveCount(1, { timeout:120000 });
    await ask('boundary', '我算出了 3 和 4，但不清楚哪个叫最小值，哪个是取得点。');
    assert.notEqual(report.steps[1].reply, report.steps[2].reply);
    await grade('4', false, 'partial');
    await grade('因为 x≥3，所以 x−1≥2，两边都非负，故 (x−1)²≥4。等号恰好在 x=3 时成立。因此最小值为 4，取得点是 x=3。', true, 'correct');
    await grade('f′(x)=2(x−1)，在 x≥3 上它恒为正，所以 f 在可行区间严格递增。左端点 x=3 取得最小值 f(3)=4。', true, 'correct');
    await calc.locator('.attempts summary').click();
    await expect(calc.locator('.attempts b')).toHaveCount(3);
    await writeFile(join(output,'walkthrough.json'), JSON.stringify(report,null,2));
  } finally { await browser.close(); }
}
main().catch(error => { console.error(error); process.exitCode = 1; });

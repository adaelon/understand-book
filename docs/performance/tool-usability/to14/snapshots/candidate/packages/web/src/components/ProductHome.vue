<script setup lang="ts">
import { computed, nextTick, ref } from 'vue';
import { ArrowDown, ArrowRight, ArrowUpRight, BookOpen, Check, ChevronDown, CornerDownRight, Maximize2, Quote, Sparkles, X } from '@lucide/vue';
import readerScreenshot from '../assets/home/reading-workspace.png';
import presentationScreenshot from '../assets/home/interactive-explanation.png';

const screenshots = [
  {
    src: readerScreenshot,
    width: 1913,
    height: 877,
    label: '阅读现场',
    title: '原文在眼前，问题就在旁边。',
    description: '沿着目录读到当前章节，把想弄明白的问题交给身边的阅读助手。学习目标和历史对话，也留在同一个阅读空间。',
    alt: '实际阅读界面：左侧是章节目录，中间是 AI Infra 原文，右侧阅读助手显示学习目标和历史对话入口。',
    details: ['章节目录', '原文阅读', '目标与对话'],
  },
  {
    src: presentationScreenshot,
    width: 1908,
    height: 876,
    label: '动手理解',
    title: '把解释变成可以试的东西。',
    description: '切换参数，比较结果，再围绕眼前的变化继续追问。助手的回答带着来源，也接着你正在看的演示现场。',
    alt: '实际讲解界面：左侧用请求数量按钮和分摊图解释模型执行耗时，右侧围绕当前演示追问，并在回答中引用原文来源。',
    details: ['调整参数', '对照结果', '围绕现场追问'],
  },
];
const imageDialog = ref<HTMLDialogElement | null>(null);
const activeScreenshot = ref(screenshots[0]!);
const originalSize = ref(false);
function openScreenshot(screenshot: typeof screenshots[number]) {
  activeScreenshot.value = screenshot;
  originalSize.value = false;
  imageDialog.value?.showModal();
}

const stage = ref(0);
const understood = ref(true);
const learningRate = ref(0.5);
const storyPanel = ref<HTMLElement | null>(null);
async function continueTo(nextStage: number) {
  stage.value = nextStage;
  await nextTick();
  storyPanel.value?.focus({ preventScroll: true });
  storyPanel.value?.scrollIntoView({ block: 'start' });
}
const stages = [
  { label: '第一次学习', detail: '从一个真实的困惑开始' },
  { label: '说说你的理解', detail: '你的反馈，改变后面的帮助' },
  { label: '下一次，接着学', detail: '看看讲解的起点如何变化' },
];
// An illustrative gradient-descent experiment for f(x) = x² / 2.
// This page does not create learning records or call the teaching service.
const descent = computed(() => {
  let x = 1.8;
  return Array.from({ length: 7 }, (_, index) => {
    const point = { x: 220 + x * 88, y: 145 - x * x * 29, index };
    x *= 1 - learningRate.value;
    return point;
  });
});
const descentLine = computed(() => descent.value.map(point => `${point.x},${point.y}`).join(' '));
const questions = [
  {
    title: '已经会用 AI 讲解，为什么还需要 Understand Book？',
    answer: '当你花几周读一本书，前后章节、问过的问题和自己的理解会不断积累。Understand Book 把原文、阅读现场和这些反馈组织在一起，让后续帮助能够接着前面的学习展开。你可以把精力留给正在理解的问题。',
  },
  {
    title: '它怎么知道我理解到了哪里？',
    answer: '它结合你的提问、复述、回应和学习中的实际表现，形成有依据、可修正的理解判断。你可以查看依据，也可以纠正它；新的反馈会继续影响讲法和下一步。读过一页本身并不代表已经掌握。',
  },
  {
    title: '我可以只读书、自由提问吗？',
    answer: '可以。你可以围绕原文自由阅读和提问，也可以开启持续教学，让它结合你的目标带着往下学。你仍然可以直接要求解释、补充例子，或改变当前的学习方向。',
  },
  {
    title: '现在怎样开始使用？',
    answer: '目前采用邀请制内测。获得内测码后，使用邮箱注册并完成验证；可阅读的材料与使用额度由管理员开通。你可以从一份已开通的材料、一节内容和一个想弄明白的问题开始。',
  },
];
</script>

<template>
  <div class="product-home" id="top">
    <a class="home-skip" href="#learning-story">跳到连续学习示意</a>
    <header class="home-nav home-width">
      <a href="#top" class="home-brand" aria-label="Understand Book 首页">
        <span class="home-brand-symbol"><BookOpen :size="23" :stroke-width="1.5" /></span>
        <span>Understand <i>Book</i></span>
      </a>
      <nav class="home-nav-links" aria-label="首页导航">
        <a href="#experience">实际界面</a>
        <a href="#learning-story">连续学习</a>
        <a href="#approach">我们的方式</a>
        <a href="#questions">常见问题</a>
      </nav>
      <div class="home-nav-actions">
        <a href="./?account=login" class="home-login">登录</a>
        <a href="#begin" class="home-nav-join">参与内测 <ArrowUpRight :size="15" /></a>
      </div>
    </header>

    <main>
      <section class="home-hero home-width" aria-labelledby="home-title">
        <div class="home-hero-copy">
          <p class="home-eyebrow"><span class="home-status-dot"></span> 给认真读书、想弄明白的人</p>
          <h1 id="home-title">接着你的理解，<br /><span>往下学。</span></h1>
          <p class="home-hero-description">一本书，常常要读很多天。<br />让你问过的问题、说过的困惑，<br class="home-desktop-break" />都成为下一次学习的起点。</p>
          <p class="home-hero-label">伴随原文的 AI 阅读与持续学习</p>
          <div class="home-hero-actions">
            <a class="home-button home-button-dark" href="#learning-story">看看两次学习 <ArrowDown :size="17" /></a>
            <a class="home-text-link" href="#begin">从一节内容开始 <ArrowUpRight :size="16" /></a>
          </div>
          <div class="home-hero-notes"><span>原文相伴</span><span>理解可修订</span><span>学习能接续</span></div>
        </div>

        <figure class="home-hero-illustration" aria-label="两次学习情境示意：从理解学习率的震荡，接着学习动量">
          <div class="home-illustration-orbit" aria-hidden="true"></div>
          <div class="home-note home-note-first">
            <div class="home-note-meta"><span><span class="home-tiny-dot"></span> 今天 · 第一次学习</span><span>01</span></div>
            <p class="home-note-chapter">梯度下降 / 学习率</p>
            <h2>“步子越大，<br />是不是就学得越快？”</h2>
            <svg class="home-bowl" viewBox="0 0 360 112" fill="none" aria-hidden="true">
              <path d="M18 93H342" stroke="#d8d7ca" stroke-dasharray="3 5" />
              <path d="M45 10Q180 177 315 10" stroke="#858e7a" stroke-width="2" />
              <path d="M68 37L284 42L98 64L260 66L122 79L230 84L162 91" stroke="#ba7453" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" />
              <circle cx="68" cy="37" r="4" fill="#ba7453" /><circle cx="162" cy="91" r="4" fill="#465744" />
              <path d="M162 100V108" stroke="#465744" /><text x="238" y="106" fill="#8c8b7f" font-size="10">试着改变步长</text>
            </svg>
            <div class="home-note-insight"><Sparkles :size="14" /><span>从“为什么会震荡”开始理解</span></div>
          </div>
          <div class="home-note home-note-next">
            <div class="home-note-meta"><span><span class="home-tiny-dot"></span> 下一次 · 接着往下学</span><span>02</span></div>
            <p class="home-note-chapter">从学习率，走到动量</p>
            <p class="home-next-explanation">上次你已经解释了<br /><em>为什么会来回震荡。</em><br />今天，我们从“怎样减小震荡”接着看。</p>
            <div class="home-note-thread"><CornerDownRight :size="15" /><span>前一次的理解，成为这一次的起点</span></div>
          </div>
          <figcaption class="home-illustration-caption"><span>一段学习，接上另一段。</span><small>情境示意</small></figcaption>
        </figure>
      </section>

      <section class="home-intro home-width">
        <p class="home-section-index">THE THREAD OF UNDERSTANDING</p>
        <div class="home-intro-body">
          <h2>一段解释之后，<br />学习还在继续。</h2>
          <div><p>今天似乎听懂了，过几天读到下一章，又遇到了新的问题。前面理解了什么，哪里还没想通，都会影响这一次该从哪里开始。</p><p>我们想让这些连接留在学习里。让每一次讲解，都接得住前一次的思考。</p></div>
        </div>
      </section>

      <section class="home-experience home-width" id="experience" aria-labelledby="experience-title">
        <div class="home-section-heading">
          <div><p class="home-section-index">01 / 实际使用界面</p><h2 id="experience-title">你会在这样的页面里，<br />读下去。</h2></div>
          <p>从原文中的一个问题，<br />走到能看、能试、能继续追问的解释。</p>
        </div>
        <figure v-for="(screenshot, index) in screenshots" :key="screenshot.src" class="home-screenshot">
          <figcaption class="home-screenshot-caption">
            <div class="home-screenshot-label"><span>0{{ index + 1 }}</span>{{ screenshot.label }}</div>
            <div><h3>{{ screenshot.title }}</h3><p>{{ screenshot.description }}</p></div>
          </figcaption>
          <button class="home-screenshot-preview" type="button" :aria-label="`放大查看：${screenshot.label}`" aria-haspopup="dialog" @click="openScreenshot(screenshot)">
            <img :src="screenshot.src" :alt="screenshot.alt" :width="screenshot.width" :height="screenshot.height" loading="lazy" decoding="async" />
            <span class="home-screenshot-zoom"><Maximize2 :size="15" /> 放大查看</span>
          </button>
          <div class="home-screenshot-notes"><div><span v-for="detail in screenshot.details" :key="detail">{{ detail }}</span></div><span>实际使用截图</span></div>
        </figure>
      </section>

      <section class="home-story-section" id="learning-story" aria-labelledby="story-title">
        <div class="home-width">
          <div class="home-section-heading">
            <div><p class="home-section-index">02 / 一次连续学习</p><h2 id="story-title">看看前一次，<br class="home-mobile-break" />怎样改变下一次。</h2></div>
            <p>同一位读者，两个学习时刻。<br />你可以改变反馈，看看讲解如何接续。</p>
          </div>
          <div class="home-story">
            <div class="home-story-tabs" role="tablist" aria-label="学习时刻">
              <button v-for="(item, index) in stages" :key="item.label" :id="`learning-tab-${index}`" role="tab" :aria-selected="stage === index" aria-controls="learning-panel" :tabindex="stage === index ? 0 : -1" :class="{ active: stage === index }" @click="stage = index" @keydown.right.prevent="stage = (index + 1) % stages.length; ($event.currentTarget as HTMLElement).parentElement?.querySelectorAll<HTMLButtonElement>('[role=tab]')[stage]?.focus()" @keydown.left.prevent="stage = (index + stages.length - 1) % stages.length; ($event.currentTarget as HTMLElement).parentElement?.querySelectorAll<HTMLButtonElement>('[role=tab]')[stage]?.focus()">
                <span class="home-step-number">0{{ index + 1 }}</span><span><strong>{{ item.label }}</strong><small>{{ item.detail }}</small></span><ArrowRight :size="17" />
              </button>
            </div>

            <div ref="storyPanel" class="home-story-panel" id="learning-panel" role="tabpanel" :aria-labelledby="`learning-tab-${stage}`" tabindex="0">
              <div class="home-story-toolbar"><span><BookOpen :size="15" /> 一起读懂梯度下降</span><span>交互情境示意</span></div>
              <div v-if="stage === 0" class="home-scene">
                <div class="home-reader-quote"><span class="home-speaker">你</span><p>“学习率越大，是不是收敛就越快？”</p></div>
                <div class="home-tutor-response"><span class="home-tutor-mark"><Sparkles :size="17" /></span><div><h3>我们先看一步，会走到哪里。</h3><p>在这个简单例子里，步长变大后，一步可能跨过最低点。拖动下面的滑块，看看路径怎样变化。</p></div></div>
                <div class="home-experiment">
                  <div class="home-experiment-caption"><span>一个可以动手试的解释</span><span>f(x) = ½x²</span></div>
                  <svg viewBox="0 0 440 170" role="img" :aria-label="learningRate > 1 ? '路径交替越过最低点，并逐渐靠近它' : '路径从一侧逐渐靠近最低点'">
                    <path d="M32 145H408" stroke="#d5d9ce" /><path d="M220 12V150" stroke="#e1e3db" stroke-dasharray="3 5" />
                    <path d="M44 29Q220 261 396 29" fill="none" stroke="#99a18e" stroke-width="2" />
                    <polyline :points="descentLine" fill="none" stroke="#b16c4e" stroke-width="1.7" />
                    <circle v-for="point in descent" :key="point.index" :cx="point.x" :cy="point.y" :r="point.index === 0 ? 5 : 3" :fill="point.index === 0 ? '#b16c4e' : '#576c51'" />
                    <text x="230" y="163" font-size="10" fill="#737a6c">最低点</text>
                  </svg>
                  <label class="home-slider-label" for="learning-rate"><span>试着调大步长</span><output>{{ Number(learningRate).toFixed(1) }}</output></label>
                  <input id="learning-rate" class="home-slider" v-model.number="learningRate" type="range" min="0.2" max="1.8" step="0.1" />
                  <p class="home-experiment-result" aria-live="polite">{{ learningRate > 1 ? '现在，路径在最低点两侧来回震荡。' : learningRate === 1 ? '这个例子里，一步恰好走到最低点。' : '现在，路径从一侧逐渐靠近最低点。' }}</p>
                </div>
                <button class="home-panel-next" @click="continueTo(1)">接下来，说说你的理解 <ArrowRight :size="16" /></button>
              </div>

              <div v-else-if="stage === 1" class="home-scene home-feedback-scene">
                <div class="home-tutor-response"><span class="home-tutor-mark"><Sparkles :size="17" /></span><div><h3>你现在是怎样理解这件事的？</h3><p>你的回应，会帮助下一次讲解找到合适的起点。选一种反馈，继续看看。</p></div></div>
                <fieldset class="home-feedback-choices"><legend>选择你的反馈</legend>
                  <label :class="{ selected: understood }"><input type="radio" name="understanding" :value="true" v-model="understood" /><span><strong>我能解释为什么会震荡了</strong><small>步子太大，跨过最低点，又从另一侧跨回来。</small></span><Check v-if="understood" :size="18" /></label>
                  <label :class="{ selected: !understood }"><input type="radio" name="understanding" :value="false" v-model="understood" /><span><strong>我还需要一个更直观的例子</strong><small>看到了路径的变化，但还没想通为什么会这样。</small></span><Check v-if="!understood" :size="18" /></label>
                </fieldset>
                <div class="home-context-note"><CornerDownRight :size="18" /><p>{{ understood ? '下一次，可以从已有的理解连接新概念。' : '下一次，先换一个例子，把这里讲清楚。' }}<small>你也可以随时补充或纠正这个判断。</small></p></div>
                <button class="home-panel-next" @click="continueTo(2)">看看下一次怎样讲 <ArrowRight :size="16" /></button>
              </div>

              <div v-else class="home-scene home-continuation-scene">
                <div class="home-reader-quote"><span class="home-speaker">你</span><p>“今天读到了动量，它为什么有帮助？”</p></div>
                <div class="home-remembered"><span>接着上次的反馈</span><p>{{ understood ? '“我能解释为什么会震荡了。”' : '“我还需要一个更直观的例子。”' }}</p></div>
                <div class="home-tutor-response"><span class="home-tutor-mark"><Sparkles :size="17" /></span><div><h3>{{ understood ? '从你理解的震荡，接着看动量。' : '我们先用一个例子，把震荡想清楚。' }}</h3><p>{{ understood ? '上次，你已经解释了步长过大时为什么会越过最低点。这次我们比较两种更新方式：只看当前梯度，以及让前几步的方向也参与这一步。' : '想象你在谷底两侧来回走，每一步都迈得过大，就会一次次越过最低处。我们先对照路径把这个过程走一遍，再看动量怎样影响更新方向。' }}</p></div></div>
                <div class="home-next-route"><span>{{ understood ? '已有理解' : '仍在处理的困惑' }}</span><ArrowRight :size="16" /><strong>{{ understood ? '连接新概念' : '换一个例子' }}</strong><ArrowRight :size="16" /><span>继续读下去</span></div>
                <p class="home-change-reason">{{ understood ? '因为你已经说明了原因，这次从两个概念的联系开始。' : '因为你提出还没想通，这次先补上理解，再进入新概念。' }}</p>
                <button class="home-panel-next" @click="continueTo(1)">换一种反馈，再看看 <ArrowRight :size="16" /></button>
              </div>
            </div>
          </div>
          <p class="home-story-footnote">以上为可操作的情境示意，展示反馈如何影响后续讲解；内容为预设示例。</p>
        </div>
      </section>

      <section class="home-approach home-width" id="approach" aria-labelledby="approach-title">
        <div class="home-section-heading"><div><p class="home-section-index">03 / 我们的方式</p><h2 id="approach-title">把注意力，留给理解本身。</h2></div><p>从正在读的这一页开始，<br />把前后的思考连接起来。</p></div>
        <div class="home-principles">
          <article><span class="home-principle-number">01 — 原文</span><h3>沿着原文，问下去。</h3><p>选中卡住的段落，围绕上下文提问。解释中的来源可以回到书里核对，让理解有一个清楚的落点。</p><div class="home-margin-note"><Quote :size="15" /><span>“这里的‘方向’，具体指什么？”</span></div></article>
          <article><span class="home-principle-number">02 — 反馈</span><h3>让反馈，影响讲法。</h3><p>说出你已经明白的，也说出仍然困惑的。系统结合这些实际回应修订判断，选择接下来怎样帮助你。</p><div class="home-margin-note"><CornerDownRight :size="16" /><span>“公式懂了，但我还想看一个例子。”</span></div></article>
          <article><span class="home-principle-number">03 — 接续</span><h3>把前后，连起来。</h3><p>再打开书时，相关的提问和反馈可以成为后续教学的依据。学习的起点，随着你的理解一起向前。</p><div class="home-margin-note"><BookOpen :size="15" /><span>“接着上次的问题，我们往下看。”</span></div></article>
        </div>
      </section>

      <section class="home-questions home-width" id="questions" aria-labelledby="questions-title">
        <div><p class="home-section-index">04 / 你可能还想知道</p><h2 id="questions-title">开始之前，<br />聊清楚几件事。</h2></div>
        <div class="home-faq-list"><details v-for="question in questions" :key="question.title"><summary>{{ question.title }}<ChevronDown :size="18" /></summary><p>{{ question.answer }}</p></details></div>
      </section>

      <section class="home-begin home-width" id="begin" aria-labelledby="begin-title">
        <div class="home-begin-inner"><div><p class="home-section-index"><span class="home-status-dot"></span> 邀请制内测进行中</p><h2 id="begin-title">从一节内容，<br />一个想弄明白的问题开始。</h2><p>带着你的学习目标，连续读几次。<br />看看下一次的帮助，是否接住了前面的理解。</p></div><div class="home-begin-actions"><a class="home-button home-button-light" href="./?account=register">已有内测码，开始注册 <ArrowUpRight :size="18" /></a><a class="home-begin-login" href="./?account=login">已经有账号？继续阅读 <ArrowRight :size="16" /></a><p>内测码由管理员邀请发放。<br />注册后，阅读材料与使用额度由管理员开通。</p></div></div>
      </section>
    </main>

    <footer class="home-footer home-width"><a href="#top" class="home-brand"><BookOpen :size="20" :stroke-width="1.5" /><span>Understand <i>Book</i></span></a><p>让每一次阅读，都接着上一次的理解。</p><a href="#top">回到开头 <ArrowUpRight :size="14" /></a></footer>
    <dialog ref="imageDialog" class="home-image-dialog" aria-labelledby="home-image-title" @click="($event.target === $event.currentTarget) && imageDialog?.close()">
      <div class="home-image-dialog-content">
        <header class="home-image-toolbar">
          <div><span>实际使用截图</span><h2 id="home-image-title">{{ activeScreenshot.label }}</h2></div>
          <div class="home-image-actions">
            <button type="button" :aria-pressed="originalSize" @click="originalSize = !originalSize">{{ originalSize ? '适应窗口' : '原始尺寸' }}</button>
            <button type="button" aria-label="关闭大图" autofocus @click="imageDialog?.close()"><X :size="20" /></button>
          </div>
        </header>
        <div class="home-image-viewport" :class="{ 'is-original': originalSize }" tabindex="0" aria-label="截图查看区域，可滚动查看原始尺寸">
          <img :src="activeScreenshot.src" :alt="activeScreenshot.alt" :width="activeScreenshot.width" :height="activeScreenshot.height" />
        </div>
        <p class="home-image-hint">{{ originalSize ? '滚动查看图片细节。' : '切换原始尺寸，可以看清页面里的文字与细节。' }}</p>
      </div>
    </dialog>
  </div>
</template>

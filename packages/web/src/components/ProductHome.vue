<script setup lang="ts">
import { ref } from 'vue';
import { ArrowDown, ArrowRight, ArrowUpRight, BookOpen, ChevronDown, CornerDownRight, Maximize2, Quote, Sparkles, X } from '@lucide/vue';
import HomeLearningDemo from './HomeLearningDemo.vue';
import YeyeMascot from './YeyeMascot.vue';
import brandLogo from '../assets/brand/understand-book-logo.svg';
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
    title: '适合和孩子一起用吗？',
    answer: '可以从孩子已经能独立阅读、愿意提问的一小节内容开始，一起提出猜想、尝试解释、对照原文。家长可以陪着讨论，把表达和尝试的空间留给孩子。内测可读材料由管理员开通，开始前先确认内容与孩子的阅读能力相符。',
  },
  {
    title: '怎样避免只是接受 AI 给出的说法？',
    answer: '把解释放回原文中核对，追问它成立的条件，再用例子或实际结果检验。你可以要求反例，也可以指出不同意见；这些新的回应会影响后续帮助。理解的判断需要具体表现作依据，并且可以修正。',
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
    <a class="home-skip" href="#learning-story">跳到动手体验</a>
    <header class="home-nav home-width">
      <a href="#top" class="home-brand" aria-label="Understand Book 首页">
        <span class="home-brand-symbol"><img :src="brandLogo" class="brand-logo" width="37" height="37" alt="" /></span>
        <span>Understand <i>Book</i></span>
      </a>
      <nav class="home-nav-links" aria-label="首页导航">
        <a href="#learning-story">动手体验</a>
        <a href="#for-you">为谁而做</a>
        <a href="#experience">实际界面</a>
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
          <p class="home-eyebrow"><span class="home-status-dot"></span> 伴随原文的 AI 阅读与持续学习</p>
          <h1 id="home-title">把答案，<br /><span>学成自己的理解。</span></h1>
          <p class="home-hero-description">沿着一本书，追问为什么，动手试一试。<br />把自己的想法与原文和结果对照，<br />今天没有想通的问题，下次接着学。</p>
          <p class="home-hero-label">持续学习，不止于一次回答。</p>
          <div class="home-hero-actions">
            <a class="home-button home-button-dark" href="#learning-story">试着想明白一个问题 <ArrowDown :size="17" /></a>
            <a class="home-text-link" href="#begin">参与内测 <ArrowUpRight :size="16" /></a>
          </div>
          <div class="home-hero-notes"><span>自己尝试</span><span>对照验证</span><span>接着往下学</span></div>
        </div>

        <figure class="home-hero-illustration" aria-label="两次学习情境示意：尝试用围栏围花园，再探索借用一面墙会怎样">
          <div class="home-illustration-orbit" aria-hidden="true"></div>
          <div class="home-hero-companion">
            <YeyeMascot :size="240" motion="idle" />
            <p><strong>页页</strong><span>陪你把一本书读懂</span></p>
          </div>
          <div class="home-note home-note-first">
            <div class="home-note-meta"><span><span class="home-tiny-dot"></span> 今天 · 第一次学习</span><span>01</span></div>
            <p class="home-note-chapter">一个猜想 / 一次尝试</p>
            <h2>“同样长的围栏，<br />怎么围出更大的花园？”</h2>
            <svg class="home-bowl" viewBox="0 0 360 112" fill="none" aria-hidden="true">
              <rect x="25" y="35" width="126" height="54" fill="#edf0e3" stroke="#8c9b77" stroke-width="1.5" />
              <path d="M43 35V89M61 35V89M79 35V89M97 35V89M115 35V89M133 35V89M25 53H151M25 71H151" stroke="#d4dcc5" />
              <path d="M175 60H204M198 55L204 60L198 65" stroke="#b78362" stroke-width="1.5" />
              <rect x="228" y="17" width="90" height="90" fill="#edf0e3" stroke="#8c9b77" stroke-width="1.5" />
              <path d="M246 17V107M264 17V107M282 17V107M300 17V107M228 35H318M228 53H318M228 71H318M228 89H318" stroke="#d4dcc5" />
            </svg>
            <div class="home-note-insight"><Sparkles :size="14" /><span>先猜一猜，再看看形状改变了什么</span></div>
          </div>
          <div class="home-note home-note-next">
            <div class="home-note-meta"><span><span class="home-tiny-dot"></span> 下一次 · 接着往下学</span><span>02</span></div>
            <p class="home-note-chapter">沿着上一次的尝试，再问一步</p>
            <p class="home-next-explanation">上次，你比较了几种围法。<br /><em>如果有一面墙可以借用，</em><br />同样的围栏，应该怎样围？</p>
            <div class="home-note-thread"><CornerDownRight :size="15" /><span>前一次的思考，成为这一次的起点</span></div>
          </div>
          <figcaption class="home-illustration-caption"><span>一段学习，接上另一段。</span><small>情境示意</small></figcaption>
        </figure>
      </section>

      <section class="home-intro home-width">
        <p class="home-section-index">THE THREAD OF UNDERSTANDING</p>
        <div class="home-intro-body">
          <h2>听懂一个答案之后，<br />你还想自己想明白。</h2>
          <div><p>看解释时觉得懂了，合上书却说不清为什么；换个条件，又不知道从哪一步开始。你想把知识学进去，让它成为自己的判断。</p><p>从一个猜想出发，得到一点帮助，再亲手检验。让这次想过的、试过的、还没弄明白的，都能成为下一次学习的起点。</p></div>
        </div>
      </section>

      <section class="home-audiences home-width" id="for-you" aria-label="为自己学习，也陪孩子学习">
        <article><p class="home-section-index">为自己学习</p><h2>从“好像懂了”，<br />走到说得清、用得上。</h2><p>读教材、读论文，或认真学习一个新领域。围绕正在读的内容追问，把前后的知识连起来，再用自己的解释和尝试检验理解。</p><a class="home-text-link" href="#experience">看看实际阅读空间 <ArrowRight :size="15" /></a></article>
        <article><p class="home-section-index">陪孩子学习</p><h2>给孩子保留<br />“我先试试”的空间。</h2><p>从孩子能读懂的一小节开始，一起提出猜想、尝试、验证。家长陪着讨论，让孩子有机会说出自己的想法，也得到继续探索的帮助。</p><a class="home-text-link" href="#learning-story">一起试一个小问题 <ArrowRight :size="15" /></a></article>
      </section>

      <HomeLearningDemo />

      <section class="home-experience home-width" id="experience" aria-labelledby="experience-title">
        <div class="home-section-heading">
          <div><p class="home-section-index">02 / 实际使用界面</p><h2 id="experience-title">沿着原文，<br />把问题想深一点。</h2></div>
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

      <section class="home-approach home-width" id="approach" aria-labelledby="approach-title">
        <div class="home-section-heading"><div><p class="home-section-index">03 / 我们的方式</p><h2 id="approach-title">持续学习，<br />不止于一次回答。</h2></div><p>让每一次求助，<br />都能帮助你接着往下想。</p></div>
        <div class="home-principles">
          <article><span class="home-principle-number">01 — 原文</span><h3>沿着原文，问下去。</h3><p>选中卡住的段落，围绕上下文提问。解释中的来源可以回到书里核对，让理解有一个清楚的落点。</p><div class="home-margin-note"><Quote :size="15" /><span>“这里的‘方向’，具体指什么？”</span></div></article>
          <article><span class="home-principle-number">02 — 尝试</span><h3>让想法，经得起检验。</h3><p>说出自己的解释，动手试一个例子，看看条件改变后会怎样。你的实际回应，帮助后续讲解找到合适的起点。</p><div class="home-margin-note"><CornerDownRight :size="16" /><span>“如果把这个条件改掉，还成立吗？”</span></div></article>
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

    <footer class="home-footer home-width"><a href="#top" class="home-brand"><img :src="brandLogo" class="brand-logo" width="30" height="30" alt="" /><span>Understand <i>Book</i></span></a><p>让每一次阅读，都接着上一次的理解。</p><a href="#top">回到开头 <ArrowUpRight :size="14" /></a></footer>
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

<script setup lang="ts">
import { t as uiText } from '../i18n';

import { computed, nextTick, ref } from 'vue';
import LanguageSwitcher from './LanguageSwitcher.vue';
import { ArrowDown, ArrowRight, ArrowUpRight, BookOpen, ChevronDown, CornerDownRight, FolderOpen, Maximize2, Music2, Play, Quote, Sparkles, X } from '@lucide/vue';
import HomeLearningDemo from './HomeLearningDemo.vue';
import YeyeMascot from './YeyeMascot.vue';
import brandLogo from '../assets/brand/understand-book-logo.svg';
import readerScreenshot from '../assets/home/reading-workspace-20261024.jpg';
import videoScreenshot from '../assets/home/video-understanding-20261024.jpg';
import musicScreenshot from '../assets/home/music-workspace-20261023.png';
import workspaceScreenshot from '../assets/home/course-workspace-20261021.png';
import presentationScreenshot from '../assets/home/interactive-explanation.png';

const screenshots = [
  {
    id: 'reading',
    icon: BookOpen,
    src: readerScreenshot,
    width: 1914,
    height: 877,
    label: uiText('阅读界面'),
    title: uiText('原文在眼前，问题就在旁边。'),
    description: uiText('读教材、论文或一本想认真读懂的书。选中卡住的段落，沿着上下文追问，也可以带着目标开始学习。'),
    alt: uiText('AI Infra 阅读界面：左侧章节目录，中间原文，右侧可选择先了解重点、带我读懂、复习备考或探索一个问题。'),
    details: [uiText('章节目录'), uiText('原文提问'), uiText('学习目标')],
  },
  {
    id: 'video',
    icon: Play,
    src: videoScreenshot,
    width: 1280,
    height: 720,
    label: uiText('理解视频'),
    title: uiText('停在没看懂的那一刻，接着问。'),
    description: uiText('看课程录像，对照双语字幕。框选画面里的代码、公式，或选取一段时间，把问题和正在看的内容一起交给助手。'),
    alt: uiText('CS336 视频理解界面：暂停在 FP16 代码示例，框选画面，对照中英字幕，右侧附上选区询问数值为什么变成了零。'),
    details: [uiText('双语字幕'), uiText('画面框选'), uiText('片段提问')],
  },
  {
    id: 'music',
    icon: Music2,
    src: musicScreenshot,
    width: 1500,
    height: 1000,
    label: uiText('理解音乐'),
    title: uiText('把听到的，和乐谱里的细节连起来。'),
    description: uiText('从精选作品开始，边听原声、边看乐谱。框住不认识的音符或符号提问，再回到片段反复听，让每一遍都有新的发现。'),
    alt: uiText('舒伯特《音乐瞬间》第 3 首音乐学习界面：乐谱、原声播放条与带谱面截图的问题输入。'),
    details: [uiText('看谱听音乐'), uiText('谱面框选'), uiText('片段反复听')],
  },
  {
    id: 'explore',
    icon: Sparkles,
    src: presentationScreenshot,
    width: 1908,
    height: 876,
    label: uiText('动手理解'),
    title: uiText('把解释变成可以试的东西。'),
    description: uiText('切换参数，比较结果，再围绕眼前的变化继续追问。助手的回答带着来源，也接着你正在看的演示。'),
    alt: uiText('实际讲解界面：左侧用请求数量按钮和分摊图解释模型执行耗时，右侧围绕当前演示追问，并在回答中引用原文来源。'),
    details: [uiText('调整参数'), uiText('对照结果'), uiText('围绕演示追问')],
  },
];
const workspace = {
  src: workspaceScreenshot, width: 978, height: 725,
  label: uiText('学习工作区'),
  alt: uiText('CS336 第 2 讲工作区：公开学习目标、适用范围、课程录像与中英转写等预构建资料，以及三步建议学习路线。'),
};
const selectedScene = ref(0);
const scene = computed(() => screenshots[selectedScene.value]!);
const galleryTabs = ref<HTMLElement | null>(null);
async function navigateScene(event: KeyboardEvent, index: number) {
  const last = screenshots.length - 1;
  const next = event.key === 'ArrowRight' ? (index + 1) % screenshots.length
    : event.key === 'ArrowLeft' ? (index + last) % screenshots.length
    : event.key === 'Home' ? 0 : event.key === 'End' ? last : null;
  if (next === null) return;
  event.preventDefault();
  selectedScene.value = next;
  await nextTick();
  galleryTabs.value?.querySelectorAll<HTMLButtonElement>('button')[next]?.focus();
}
const imageDialog = ref<HTMLDialogElement | null>(null);
type Screenshot = Pick<typeof workspace, 'src' | 'width' | 'height' | 'label' | 'alt'>;
const activeScreenshot = ref<Screenshot>(screenshots[0]!);
const originalSize = ref(false);
function openScreenshot(screenshot: Screenshot) {
  activeScreenshot.value = screenshot;
  originalSize.value = false;
  imageDialog.value?.showModal();
}

const copyrightYear = new Date().getFullYear();
const siteInfo = {
  terms: {
    title: uiText('服务条款'),
    intro: uiText('欢迎使用 Understand Book。这里说明当前邀请制内测的使用方式。'),
    sections: [
      { title: uiText('账号与内测资格'), text: uiText('使用内测码和邮箱注册，完成邮箱验证后即可登录。内测期间 AI Token 免费使用，不限额度。已有账号可直接登录，继续之前的阅读与学习。') },
      { title: uiText('阅读材料与使用范围'), text: uiText('请仅提交和使用你有权使用的内容，尊重书籍、论文及其他材料的作者与权利人。获得阅读权限不代表获得转载、分发或商业使用的授权。') },
      { title: uiText('AI 回答与学习判断'), text: uiText('AI 生成的解释、引用和对理解程度的判断可能存在错误或遗漏。请结合原文和实际结果核对；你可以提出异议、纠正判断或调整学习方向。') },
      { title: uiText('内测期间的服务'), text: uiText('功能、可用材料与额度安排会随内测调整，维护时可能暂时无法使用。有关当前安排、使用问题与反馈，请联系邀请你参与内测的管理员。') },
    ],
  },
  privacy: {
    title: uiText('隐私说明'),
    intro: uiText('以下说明当前网站处理哪些信息，以及这些信息如何参与阅读与学习。'),
    sections: [
      { title: uiText('账号与使用信息'), text: uiText('注册和登录会处理邮箱、账号验证信息与登录状态，用于身份验证、邮件验证和找回密码。服务还会记录材料权限、使用额度及调用记录，用于提供和管理内测服务。') },
      { title: uiText('阅读与学习记录'), text: uiText('阅读位置、笔记、提问、对话、学习目标及相关学习反馈会保存在服务端，用于恢复阅读进度、展示历史，并帮助后续讲解接着已有学习展开。') },
      { title: uiText('AI 与第三方服务'), text: uiText('生成回答时，你的问题、相关原文及必要的对话和学习上下文会发送给网站配置的 AI 服务。网站还使用第三方云服务提供网络访问和邮件发送；相关服务会处理完成这些功能所需的信息。请避免在提问中提交密码、验证码或无关的个人敏感信息。') },
      { title: uiText('浏览器存储与运行记录'), text: uiText('网站使用 Cookie 保持登录，并在浏览器中保存字号、阅读方式等偏好和待处理操作。服务端会保存运行与调用记录，以支持用量统计和问题排查。') },
      { title: uiText('管理与联系'), text: uiText('你可以在账号设置中查看已绑定邮箱，并在阅读界面查看自己的历史记录。有关账号、个人信息的更正或删除请求，以及本说明的问题，请联系邀请你参与内测的管理员，确认具体处理范围。') },
    ],
  },
  about: {
    title: uiText('关于我们'),
    intro: uiText('Understand Book 是一个围绕目标组织阅读、视频与音乐学习的工作区。'),
    sections: [
      { title: uiText('让理解接着发生'), text: uiText('我们希望把材料、问题、尝试和学习反馈留在同一个工作区。读书、看课程、听音乐时形成的理解，都能成为下一次学习的起点。') },
      { title: uiText('从一本书、一个问题开始'), text: uiText('你可以自由阅读和提问，也可以围绕自己的目标持续学习。通过对照原文、解释自己的想法和动手验证，把听到的答案逐渐变成自己的理解。') },
      { title: uiText('一起把产品做好'), text: uiText('目前产品采用邀请制内测。你可以向邀请人领取内测码，并在使用过程中向管理员交流感受、反馈问题。') },
    ],
  },
} as const;
const siteInfoDialog = ref<HTMLDialogElement | null>(null);
const siteInfoContent = ref<HTMLElement | null>(null);
const activeSiteInfo = ref<keyof typeof siteInfo>('terms');
function openSiteInfo(section: keyof typeof siteInfo) {
  activeSiteInfo.value = section;
  siteInfoDialog.value?.showModal();
  siteInfoContent.value?.scrollTo(0, 0);
}

const questions = [
  {
    title: uiText('工作区和一份阅读材料有什么区别？'),
    answer: uiText('材料是你正在读的书、看的视频或听的作品；工作区围绕一个学习目标，把相关材料、聊天和私人学习记录组织起来。你可以使用已准备好的公开工作区，也可以围绕自己的目标组织学习。'),
  },
  {
    title: uiText('视频和音乐可以怎样学？'),
    answer: uiText('视频可以对照字幕、跳转片段、框选画面提问；音乐可以边听原声边看谱，框选谱面，围绕片段继续问。目前请从已开放的视频资料与精选音乐作品开始，具体材料由管理员开通。'),
  },
  {
    title: uiText('已经会用 AI 讲解，为什么还需要 Understand Book？'),
    answer: uiText('当你花几周读一本书、跟一门课或学习一部作品，问过的问题和自己的理解会不断积累。Understand Book 把相关材料、学习目标与这些反馈组织在一起，让后续帮助接着前面的学习展开。'),
  },
  {
    title: uiText('它怎么知道我理解到了哪里？'),
    answer: uiText('它结合你的提问、复述、回应和学习中的实际表现，形成有依据、可修正的理解判断。你可以查看依据，也可以纠正它；新的反馈会继续影响讲法和下一步。读过一页本身并不代表已经掌握。'),
  },
  {
    title: uiText('适合和孩子一起用吗？'),
    answer: uiText('可以从孩子已经能独立阅读、愿意提问的一小节内容开始，一起提出猜想、尝试解释、对照原文。家长可以陪着讨论，把表达和尝试的空间留给孩子。内测可读材料由管理员开通，开始前先确认内容与孩子的阅读能力相符。'),
  },
  {
    title: uiText('怎样避免只是接受 AI 给出的说法？'),
    answer: uiText('把解释放回原文中核对，追问它成立的条件，再用例子或实际结果检验。你可以要求反例，也可以指出不同意见；这些新的回应会影响后续帮助。理解的判断需要具体表现作依据，并且可以修正。'),
  },
  {
    title: uiText('我可以只读书、自由提问吗？'),
    answer: uiText('可以。你可以围绕原文自由阅读和提问，也可以开启持续教学，让它结合你的目标带着往下学。你仍然可以直接要求解释、补充例子，或改变当前的学习方向。'),
  },
  {
    title: uiText('现在怎样开始使用？'),
    answer: uiText('向邀请人领取内测码后，使用邮箱注册并完成验证。内测期间 AI Token 免费使用，不限额度。从已开放的材料和一个想弄明白的问题开始，下次用同一账号登录即可继续。'),
  },
];
</script>

<template>
  <div class="product-home" id="top">
    <a class="home-skip" href="#learning-story">{{ uiText('跳到动手体验') }}</a>
    <header class="home-nav home-width">
      <a href="#top" class="home-brand" :aria-label="uiText('Understand Book 首页')">
        <span class="home-brand-symbol"><img :src="brandLogo" class="brand-logo" width="37" height="37" alt="" /></span>
        <span>Understand <i>Book</i></span>
      </a>
      <nav class="home-nav-links" :aria-label="uiText('首页导航')">
        <a href="#workspace">{{ uiText('学习工作区') }}</a>
        <a href="#experience">{{ uiText('阅读、视频与音乐') }}</a>
        <a href="#learning-story">{{ uiText('动手体验') }}</a>
        <a href="#questions">{{ uiText('常见问题') }}</a>
      </nav>
      <div class="home-nav-actions">
        <LanguageSwitcher />
        <a href="./?account=login" class="home-login">{{ uiText('登录') }}</a>
        <a href="/downloads/UnderstandBookSetup.exe" class="home-nav-join" download>{{ uiText('Windows 版') }} <ArrowDown :size="15" /></a>
      </div>
    </header>

    <main>
      <section class="home-hero home-width" aria-labelledby="home-title">
        <div class="home-hero-copy">
          <p class="home-eyebrow"><span class="home-status-dot"></span> {{ uiText('阅读 · 视频 · 音乐 · 学习工作区') }}</p>
          <h1 id="home-title">{{ uiText('把想学的，') }}<br /><span>{{ uiText('学成自己的理解。') }}</span></h1>
          <p class="home-hero-description"><span>{{ uiText('从一本书、一节视频课，到一段音乐。') }}</span><span>{{ uiText('把材料、问题与笔记放进同一个工作区，') }}</span><span>{{ uiText('让每次探索，都能接着上次继续。') }}</span></p>
          <p class="home-hero-label">{{ uiText('一个目标，多种材料，一路积累的理解。') }}</p>
          <div class="home-hero-actions">
            <a class="home-button home-button-dark" href="#experience">{{ uiText('看看怎样学') }} <ArrowDown :size="17" /></a>
            <a class="home-text-link" href="#begin">{{ uiText('参与内测') }} <ArrowUpRight :size="16" /></a>
          </div>
          <div class="home-hero-notes"><span>{{ uiText('对照材料') }}</span><span>{{ uiText('动手探索') }}</span><span>{{ uiText('接着往下学') }}</span></div>
        </div>

        <div class="home-hero-workspace">
          <div class="home-workspace-cover">
            <div><p class="home-section-index">A SPACE FOR CURIOSITY</p><h2>{{ uiText('给想学的东西，') }}<br />{{ uiText('一个可以回来的地方。') }}</h2></div>
            <div class="home-hero-companion"><YeyeMascot :size="112" motion="idle" /><span>{{ uiText('页页，陪你一起学') }}</span></div>
          </div>
          <button class="home-screenshot-preview home-hero-preview" type="button" :aria-label="uiText('放大查看：{0}', [screenshots[0]!.label])" aria-haspopup="dialog" @click="openScreenshot(screenshots[0]!)">
            <img :src="readerScreenshot" :alt="screenshots[0]!.alt" width="1914" height="877" fetchpriority="high" />
            <span class="home-screenshot-zoom"><Maximize2 :size="14" /> {{ uiText('实际阅读界面') }}</span>
          </button>
          <div class="home-workspace-sources"><span><BookOpen :size="16" />{{ uiText('一本书') }}</span><span><Play :size="16" />{{ uiText('一门课') }}</span><span><Music2 :size="16" />{{ uiText('一段音乐') }}</span></div>
        </div>
      </section>

      <section class="home-workspace home-width" id="workspace" aria-labelledby="workspace-title">
        <div class="home-workspace-copy">
          <p class="home-section-index">{{ uiText('01 / 围绕目标，组织学习') }}</p>
          <h2 id="workspace-title">{{ uiText('材料可以有很多份，') }}<br />{{ uiText('学习有一条自己的线。') }}</h2>
          <p>{{ uiText('想学 AI Infra，想跟完一门课，或想认真听懂一部作品。工作区围绕你的目标，把相关材料和学习过程放在一起。') }}</p>
          <dl class="home-workspace-points">
            <div><dt><FolderOpen :size="18" />{{ uiText('把材料放在一起') }}</dt><dd>{{ uiText('书籍、课程录像与配套资料，围绕同一个目标组织。') }}</dd></div>
            <div><dt><CornerDownRight :size="18" />{{ uiText('知道下一步学什么') }}</dt><dd>{{ uiText('从准备好的资料和建议路线开始，也可以按自己的问题探索。') }}</dd></div>
            <div><dt><BookOpen :size="18" />{{ uiText('让理解留下来') }}</dt><dd>{{ uiText('目标、笔记、对话和学习记录持续积累，下次回来接着学。') }}</dd></div>
          </dl>
        </div>
        <figure class="home-workspace-example">
          <figcaption><span>{{ uiText('一个真实的课程工作区') }}</span><strong>{{ uiText('CS336 · 第 2 讲') }}</strong><p>{{ uiText('PyTorch 与资源核算：从课程录像到动手复习。') }}</p></figcaption>
          <button class="home-screenshot-preview" type="button" :aria-label="uiText('放大查看：{0}', [workspace.label])" aria-haspopup="dialog" @click="openScreenshot(workspace)"><img :src="workspace.src" :alt="workspace.alt" :width="workspace.width" :height="workspace.height" loading="lazy" decoding="async" /><span class="home-screenshot-zoom"><Maximize2 :size="14" /> {{ uiText('放大查看') }}</span></button>
          <p class="home-workspace-caption">{{ uiText('学习目标、预构建资料与建议学习路线。实际使用截图。') }}</p>
        </figure>
      </section>

      <section class="home-experience home-width" id="experience" aria-labelledby="experience-title">
        <div class="home-section-heading">
          <div><p class="home-section-index">{{ uiText('02 / 阅读、视频与音乐') }}</p><h2 id="experience-title">{{ uiText('换一种材料，') }}<br />{{ uiText('继续同一份好奇。') }}</h2></div>
          <p>{{ uiText('从原文、画面、声音出发，') }}<br />{{ uiText('让问题有依据，让解释可以回看。') }}</p>
        </div>
        <div ref="galleryTabs" class="home-gallery-tabs" role="tablist" :aria-label="uiText('选择学习场景')">
          <button v-for="(item, index) in screenshots" :key="item.id" :id="'scene-tab-' + item.id" type="button" role="tab" :aria-selected="selectedScene === index" aria-controls="scene-panel" :tabindex="selectedScene === index ? 0 : -1" @click="selectedScene = index" @keydown="navigateScene($event, index)"><component :is="item.icon" :size="18" />{{ item.label }}</button>
        </div>
        <figure id="scene-panel" class="home-screenshot home-gallery-panel" role="tabpanel" :aria-labelledby="'scene-tab-' + scene.id" tabindex="0">
          <figcaption class="home-screenshot-caption">
            <div class="home-screenshot-label"><span>0{{ selectedScene + 1 }}</span>{{ scene.label }}</div>
            <div><h3>{{ scene.title }}</h3><p>{{ scene.description }}</p></div>
          </figcaption>
          <button class="home-screenshot-preview home-gallery-preview" type="button" :aria-label="uiText('放大查看：{0}', [scene.label])" aria-haspopup="dialog" @click="openScreenshot(scene)">
            <img :src="scene.src" :alt="scene.alt" :width="scene.width" :height="scene.height" loading="lazy" decoding="async" />
            <span class="home-screenshot-zoom"><Maximize2 :size="15" /> {{ uiText('放大查看') }}</span>
          </button>
          <div class="home-screenshot-notes"><div><span v-for="detail in scene.details" :key="detail">{{ detail }}</span></div><span>{{ uiText('实际使用截图') }}</span></div>
        </figure>
      </section>

      <div class="home-demo-intro home-width"><p>{{ uiText('理解，也可以从一次亲手尝试开始。') }}</p><a class="home-text-link" href="#learning-story">{{ uiText('试着想明白一个问题') }} <ArrowDown :size="16" /></a></div>
      <HomeLearningDemo />

      <section class="home-approach home-width" id="approach" aria-labelledby="approach-title">
        <div class="home-section-heading"><div><p class="home-section-index">{{ uiText('04 / 让学习接着发生') }}</p><h2 id="approach-title">{{ uiText('持续学习，') }}<br />{{ uiText('不止于一次回答。') }}</h2></div><p>{{ uiText('让每一次求助，') }}<br />{{ uiText('都能帮助你接着往下想。') }}</p></div>
        <div class="home-principles">
          <article><span class="home-principle-number">{{ uiText('01 — 依据') }}</span><h3>{{ uiText('回到材料，问下去。') }}</h3><p>{{ uiText('选中一段原文、一帧画面或一处谱面，围绕眼前的内容提问。再回到原文、视频片段和乐谱里核对。') }}</p><div class="home-margin-note"><Quote :size="15" /><span>{{ uiText('“这里的‘方向’，具体指什么？”') }}</span></div></article>
          <article><span class="home-principle-number">{{ uiText('02 — 尝试') }}</span><h3>{{ uiText('让想法，经得起检验。') }}</h3><p>{{ uiText('说出自己的解释，动手试一个例子，看看条件改变后会怎样。你的实际回应，帮助后续讲解找到合适的起点。') }}</p><div class="home-margin-note"><CornerDownRight :size="16" /><span>{{ uiText('“如果把这个条件改掉，还成立吗？”') }}</span></div></article>
          <article><span class="home-principle-number">{{ uiText('03 — 接续') }}</span><h3>{{ uiText('把前后，连起来。') }}</h3><p>{{ uiText('再回到工作区时，相关的提问和反馈可以成为后续学习的依据。在“我的理解”里查看、补充或纠正，让学习接着向前。') }}</p><div class="home-margin-note"><BookOpen :size="15" /><span>{{ uiText('“接着上次的问题，我们往下看。”') }}</span></div></article>
        </div>
      </section>

      <section class="home-questions home-width" id="questions" aria-labelledby="questions-title">
        <div><p class="home-section-index">{{ uiText('05 / 你可能还想知道') }}</p><h2 id="questions-title">{{ uiText('开始之前，') }}<br />{{ uiText('聊清楚几件事。') }}</h2></div>
        <div class="home-faq-list"><details v-for="question in questions" :key="question.title"><summary>{{ question.title }}<ChevronDown :size="18" /></summary><p>{{ question.answer }}</p></details></div>
      </section>

      <section class="home-begin home-width" id="begin" aria-labelledby="begin-title">
        <div class="home-begin-inner">
          <div>
            <p class="home-section-index"><span class="home-status-dot"></span> {{ uiText('邀请制内测进行中') }}</p>
            <h2 id="begin-title">{{ uiText('从一个想学的目标，') }}<br />{{ uiText('开始自己的工作区。') }}</h2>
            <p>{{ uiText('带上想读的书、想学的课、想听懂的音乐。') }}<br />{{ uiText('从已开放的材料开始，一点点积累自己的理解。') }}</p>
          </div>
          <div class="home-begin-actions">
            <p class="home-beta-notice">{{ uiText('内测期间 AI Token 免费使用，不限额度。') }}</p>
            <a class="home-button home-button-light" href="./?account=register">{{ uiText('已有内测码，开始注册') }} <ArrowUpRight :size="18" /></a>
            <a class="home-begin-login" href="./?account=login">{{ uiText('已经有账号？继续阅读') }} <ArrowRight :size="16" /></a>
            <a class="home-begin-login" href="/downloads/UnderstandBookSetup.exe" download>{{ uiText('下载 Windows 版 · 使用同一账号登录') }} <ArrowRight :size="16" /></a>
            <p>{{ uiText('使用内测码注册，验证邮箱后即可开始。') }}</p>
          </div>
        </div>
      </section>
    </main>

    <footer class="home-footer home-width">
      <p class="home-footer-tagline">{{ uiText('让想学的东西，有地方继续。') }}</p>
      <div class="home-footer-bottom">
        <p class="home-copyright">© {{ copyrightYear }} {{ uiText('Understand Book · 保留所有权利') }}</p>
        <nav class="home-footer-links" :aria-label="uiText('网站信息')">
          <button type="button" aria-haspopup="dialog" @click="openSiteInfo('terms')">{{ uiText('服务条款') }}</button>
          <button type="button" aria-haspopup="dialog" @click="openSiteInfo('privacy')">{{ uiText('隐私说明') }}</button>
          <a href="#begin">{{ uiText('参与内测') }}</a>
          <button type="button" aria-haspopup="dialog" @click="openSiteInfo('about')">{{ uiText('关于我们') }}</button>
        </nav>
      </div>
    </footer>
    <dialog ref="siteInfoDialog" class="home-info-dialog" aria-labelledby="home-info-title" @click="($event.target === $event.currentTarget) && siteInfoDialog?.close()">
      <div class="home-info-panel">
        <header class="home-info-header">
          <div><p>Understand Book</p><h2 id="home-info-title">{{ siteInfo[activeSiteInfo].title }}</h2></div>
          <button type="button" :aria-label="uiText('关闭说明')" autofocus @click="siteInfoDialog?.close()"><X :size="20" /></button>
        </header>
        <div ref="siteInfoContent" class="home-info-content" tabindex="0" :aria-label="uiText('说明正文')">
          <p class="home-info-date">{{ uiText('更新日期：2026 年 10 月 24 日') }}</p>
          <p class="home-info-intro">{{ siteInfo[activeSiteInfo].intro }}</p>
          <section v-for="section in siteInfo[activeSiteInfo].sections" :key="section.title">
            <h3>{{ section.title }}</h3><p>{{ section.text }}</p>
          </section>
          <a class="home-info-contact" href="#begin" @click="siteInfoDialog?.close()">{{ uiText('查看内测参与方式') }} <ArrowUpRight :size="15" /></a>
        </div>
      </div>
    </dialog>
    <dialog ref="imageDialog" class="home-image-dialog" aria-labelledby="home-image-title" @click="($event.target === $event.currentTarget) && imageDialog?.close()">
      <div class="home-image-dialog-content">
        <header class="home-image-toolbar">
          <div><span>{{ uiText('实际使用截图') }}</span><h2 id="home-image-title">{{ activeScreenshot.label }}</h2></div>
          <div class="home-image-actions">
            <button type="button" :aria-pressed="originalSize" @click="originalSize = !originalSize">{{ originalSize ? uiText('适应窗口') : uiText('原始尺寸') }}</button>
            <button type="button" :aria-label="uiText('关闭大图')" autofocus @click="imageDialog?.close()"><X :size="20" /></button>
          </div>
        </header>
        <div class="home-image-viewport" :class="{ 'is-original': originalSize }" tabindex="0" :aria-label="uiText('截图查看区域，可滚动查看原始尺寸')">
          <img :src="activeScreenshot.src" :alt="activeScreenshot.alt" :width="activeScreenshot.width" :height="activeScreenshot.height" />
        </div>
        <p class="home-image-hint">{{ originalSize ? uiText('滚动查看图片细节。') : uiText('切换原始尺寸，可以看清页面里的文字与细节。') }}</p>
      </div>
    </dialog>
  </div>
</template>

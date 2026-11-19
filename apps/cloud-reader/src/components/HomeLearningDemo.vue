<script setup lang="ts">
import { t as uiText } from '@understand-book/web/reader/i18n';

import { computed, nextTick, ref } from 'vue';
import { ArrowRight, BookOpen, CornerDownRight, Sparkles } from '@lucide/vue';

// This browser-local illustration never creates a teaching session or learning record.
const stage = ref(0);
const panel = ref<HTMLElement | null>(null);
const stepsNav = ref<HTMLElement | null>(null);
const stages = [uiText('先猜一猜'), uiText('动手试试'), uiText('说说理由'), uiText('换个条件'), uiText('下次接着学')];
const prediction = ref('');
const predictions = [
  { value: 'long', label: uiText('把一边拉得越长越好') },
  { value: 'square', label: uiText('长和宽一样时，面积最大') },
  { value: 'same', label: uiText('围栏一样长，面积应该一样') },
];
const length = ref(2);
const width = computed(() => 10 - length.value);
const area = computed(() => length.value * width.value);
const observations = ref<number[]>([]);
const reflection = ref('');
const explanationOpen = ref(false);
const explanationSeen = ref(false);
const transferAnswer = ref<number | null>(null);
const checkedAnswer = ref<number | null>(null);
const predictionLabel = computed(() => predictions.find(item => item.value === prediction.value)?.label);
const continuation = computed(() => {
  if (checkedAnswer.value !== null) return checkedAnswer.value === 100
    ? uiText('你已经对照过围栏翻倍后的计算。下次，可以继续试试另一个条件。')
    : uiText('你对围栏翻倍后的面积有一个猜想。下次，先把新边长和新面积一起算一遍。');
  if (reflection.value.trim()) return uiText('你留下了自己的解释。下次，可以从这句话出发，换个条件检验它。');
  if (observations.value.length) return uiText('你比较过的围法可以留下来。下次，从这些具体结果继续寻找规律。');
  if (explanationSeen.value) return uiText('你看过了这里的解释。下次，可以自己选两种围法，看看计算是否支持它。');
  if (prediction.value) return uiText('你已经提出一个猜想。下次，可以从一次实际比较开始检验它。');
  return uiText('还没有留下尝试。可以先选一个猜想，或者直接动手围一围。');
});

async function continueTo(nextStage: number) {
  stage.value = nextStage;
  await nextTick();
  stepsNav.value?.querySelector<HTMLButtonElement>('[aria-current="step"]')?.scrollIntoView({ behavior: 'instant', block: 'nearest', inline: 'nearest' });
  panel.value?.focus({ preventScroll: true });
  panel.value?.scrollIntoView({ block: 'start' });
}

function rememberShape() {
  if (!observations.value.includes(length.value)) observations.value.push(length.value);
}

function chooseTransfer(answer: number) {
  transferAnswer.value = answer;
  checkedAnswer.value = null;
}

function showExplanation() {
  explanationOpen.value = true;
  void continueTo(2);
}

function recordExplanation(event: Event) {
  explanationOpen.value = (event.target as HTMLDetailsElement).open;
  if (explanationOpen.value) explanationSeen.value = true;
}
</script>

<template>
  <section class="home-story-section" id="learning-story" aria-labelledby="story-title">
    <div class="home-width">
      <div class="home-section-heading">
        <div><p class="home-section-index">{{ uiText('03 / 亲手想明白一件事') }}</p><h2 id="story-title">{{ uiText('先有自己的想法，') }}<br />{{ uiText('再看看它是否成立。') }}</h2></div>
        <p>{{ uiText('用一个小问题，走过一段思考。') }}<br />{{ uiText('随时可以看解释，也可以回去再试。') }}</p>
      </div>
      <div class="home-story">
        <nav ref="stepsNav" class="home-story-tabs" :aria-label="uiText('体验步骤')">
          <button v-for="(label, index) in stages" :key="label" type="button" :aria-current="stage === index ? 'step' : undefined" :class="{ active: stage === index }" aria-controls="learning-panel" @click="stage = index">
            <span class="home-step-number">0{{ index + 1 }}</span><span><strong>{{ label }}</strong></span><ArrowRight :size="17" />
          </button>
          <p class="home-step-note">{{ uiText('思考可以有来回。') }}<br />{{ uiText('选任何一步，按自己的节奏试。') }}</p>
        </nav>

        <div ref="panel" class="home-story-panel" id="learning-panel" role="region" :aria-label="stages[stage]" tabindex="-1">
          <div class="home-story-toolbar"><span><BookOpen :size="15" /> {{ uiText('同样长的围栏，更大的花园') }}</span><span>{{ uiText('可操作示例') }}</span></div>

          <div v-if="stage === 0" class="home-scene">
            <div class="home-reader-quote"><span class="home-speaker">{{ uiText('想') }}</span><p>{{ uiText('“用 20 米围栏围出一个长方形，怎样让里面的面积最大？”') }}</p></div>
            <div class="home-tutor-response"><span class="home-tutor-mark"><Sparkles :size="17" /></span><div><h3>{{ uiText('先留下一个猜想。') }}</h3><p>{{ uiText('四条边都用围栏。还没把握也没关系，等会儿可以亲手试。') }}</p></div></div>
            <fieldset class="home-feedback-choices"><legend>{{ uiText('你觉得哪一种更可能？') }}</legend>
              <label v-for="item in predictions" :key="item.value" :class="{ selected: prediction === item.value }"><input v-model="prediction" type="radio" name="garden-prediction" :value="item.value" /><span><strong>{{ item.label }}</strong></span></label>
            </fieldset>
            <div class="home-demo-actions"><button class="home-inline-action" type="button" @click="showExplanation">{{ uiText('我想先看解释') }}</button><button class="home-panel-next" type="button" @click="continueTo(1)">{{ prediction ? uiText('带着猜想，动手试试') : uiText('还没想好，先动手看看') }} <ArrowRight :size="16" /></button></div>
          </div>

          <div v-else-if="stage === 1" class="home-scene">
            <div class="home-tutor-response"><span class="home-tutor-mark"><Sparkles :size="17" /></span><div><h3>{{ uiText('一边变长，另一边会怎样？') }}</h3><p>{{ uiText('拖动滑块，改变花园的形状。留意：围栏总长一直是 20 米。') }}</p></div></div>
            <div class="home-experiment home-garden">
              <div class="home-experiment-caption"><span>{{ uiText('每个小方格是 1 平方米') }}</span><span>{{ uiText('围栏 20 米') }}</span></div>
              <svg viewBox="0 0 400 260" role="img" :aria-label="uiText('长 {0} 米，宽 {1} 米，面积 {2} 平方米的长方形花园', [length, width, area])">
                <defs><pattern id="garden-grid" x="0" y="0" width="23" height="23" patternUnits="userSpaceOnUse"><path d="M23 0H0V23" fill="none" stroke="#b8c7a3" stroke-width="1" /></pattern></defs>
                <g :transform="`translate(${200 - length * 11.5}, ${120 - width * 11.5})`">
                  <rect :width="length * 23" :height="width * 23" fill="#e5edd9" />
                  <rect :width="length * 23" :height="width * 23" fill="url(#garden-grid)" stroke="#607b50" stroke-width="2" />
                  <text :x="length * 11.5" y="-10" text-anchor="middle" fill="#526746" font-size="13">{{ length }} {{ uiText('米') }}</text>
                  <text :x="length * 23 + 12" :y="width * 11.5 + 4" fill="#526746" font-size="13">{{ width }} {{ uiText('米') }}</text>
                </g>
                <text x="200" y="253" text-anchor="middle" fill="#69785a" font-size="12">{{ uiText('两条长边 + 两条宽边 = 20 米') }}</text>
              </svg>
              <div class="home-garden-result" aria-live="polite"><span>{{ length }} × {{ width }}</span><strong>{{ area }} <small>{{ uiText('平方米') }}</small></strong></div>
              <label class="home-slider-label" for="garden-length"><span>{{ uiText('调整一条边的长度') }}</span><output>{{ length }} {{ uiText('米') }}</output></label>
              <input id="garden-length" v-model.number="length" class="home-slider" type="range" min="1" max="9" step="1" />
              <button class="home-inline-action home-record-shape" type="button" :disabled="observations.includes(length)" @click="rememberShape">{{ observations.includes(length) ? uiText('这一种已记下') : uiText('记下这一种围法') }}</button>
            </div>
            <div v-if="observations.length" class="home-observations" aria-live="polite"><span>{{ uiText('你记下的结果') }}</span><ul><li v-for="side in observations" :key="side">{{ side }} × {{ 10 - side }} = {{ side * (10 - side) }} m²</li></ul></div>
            <button class="home-panel-next" type="button" @click="continueTo(2)">{{ uiText('说说你发现了什么') }} <ArrowRight :size="16" /></button>
          </div>

          <div v-else-if="stage === 2" class="home-scene">
            <div class="home-tutor-response"><span class="home-tutor-mark"><Sparkles :size="17" /></span><div><h3>{{ uiText('围栏一样长，面积为什么会变？') }}</h3><p>{{ uiText('试着把观察到的结果连成一个解释。也可以记下暂时没想通的地方。') }}</p></div></div>
            <div class="home-reflection"><label for="garden-reflection">{{ uiText('用自己的话说一句') }} <span>{{ uiText('（可选）') }}</span></label><textarea id="garden-reflection" v-model="reflection" rows="3" maxlength="1200" :placeholder="uiText('我原来以为……试过之后，我发现……')" /><p>{{ uiText('这句话会留在本页最后的回顾里。') }}</p></div>
            <details class="home-demo-explanation" :open="explanationOpen" @toggle="recordExplanation"><summary>{{ uiText('看看一种解释') }} <ArrowRight :size="15" /></summary><div><p>{{ uiText('四条边总共 20 米，所以一条长边加一条宽边总是 10 米。面积则是两者相乘。') }}</p><p>{{ uiText('从 5 × 5 出发，一边加 2，另一边就要减 2：7 × 3 = 21，比原来的 25 小了 4。') }}</p><p>{{ uiText('把这个变化记作 d：') }}<strong>(5 + d) × (5 − d) = 25 − d²</strong>{{ uiText('。偏离得越多，减去的面积越大；长宽都为 5 米时，面积最大。') }}</p></div></details>
            <div class="home-demo-actions"><button class="home-inline-action" type="button" @click="continueTo(1)">{{ uiText('回去再试试') }}</button><button class="home-panel-next" type="button" @click="continueTo(3)">{{ uiText('换个条件，再想一次') }} <ArrowRight :size="16" /></button></div>
          </div>

          <div v-else-if="stage === 3" class="home-scene">
            <div class="home-reader-quote"><span class="home-speaker">{{ uiText('想') }}</span><p>{{ uiText('“围栏增加到 40 米，最大的面积也会翻倍吗？”') }}</p></div>
            <div class="home-tutor-response"><span class="home-tutor-mark"><Sparkles :size="17" /></span><div><h3>{{ uiText('把刚才的想法，用到新条件里。') }}</h3><p>{{ uiText('仍然围成长方形，四条边都用围栏。最大的面积会是多少？') }}</p></div></div>
            <fieldset class="home-feedback-choices home-transfer-choices"><legend>{{ uiText('先想一想，再对照计算') }}</legend><label v-for="answer in [25, 50, 100]" :key="answer" :class="{ selected: transferAnswer === answer }"><input type="radio" name="garden-transfer" :value="answer" :checked="transferAnswer === answer" @change="chooseTransfer(answer)" /><span><strong>{{ answer }} {{ uiText('平方米') }}</strong></span></label></fieldset>
            <button class="home-verify" type="button" :disabled="transferAnswer === null" @click="checkedAnswer = transferAnswer">{{ uiText('用计算验证') }}</button>
            <div v-if="checkedAnswer !== null" class="home-transfer-result" role="status"><strong>{{ checkedAnswer === 100 ? uiText('这次判断与计算一致。') : uiText('把两条边一起算进去，再看一次。') }}</strong><p>{{ uiText('40 ÷ 4 = 10 米。长宽相等时，面积为 10 × 10 = 100 平方米。两条边都翻倍，面积变成原来的 4 倍。') }}</p><p v-if="checkedAnswer !== 100">{{ uiText('你刚才选了') }} {{ checkedAnswer }} {{ uiText('平方米。可以回头看看，是在哪一步少算了一个变化。') }}</p></div>
            <button class="home-panel-next" type="button" @click="continueTo(4)">{{ uiText('看看下次从哪里继续') }} <ArrowRight :size="16" /></button>
          </div>

          <div v-else class="home-scene">
            <div class="home-tutor-response"><span class="home-tutor-mark"><Sparkles :size="17" /></span><div><h3>{{ uiText('这次想过的，下次接得上。') }}</h3><p>{{ continuation }}</p></div></div>
            <dl class="home-learning-note">
              <template v-if="predictionLabel"><dt>{{ uiText('最初的猜想') }}</dt><dd>{{ predictionLabel }}</dd></template>
              <template v-if="observations.length"><dt>{{ uiText('记下的围法') }}</dt><dd>{{ observations.map(side => `${side} × ${10 - side} = ${side * (10 - side)} m²`).join('；') }}</dd></template>
              <template v-if="reflection.trim()"><dt>{{ uiText('你自己的解释或疑问') }}</dt><dd class="home-reflection-saved">{{ reflection }}</dd></template>
              <template v-if="explanationSeen"><dt>{{ uiText('看过的帮助') }}</dt><dd>{{ uiText('长宽之和不变时，面积怎样随着形状变化。') }}</dd></template>
              <template v-if="checkedAnswer !== null"><dt>{{ uiText('换条件后的尝试') }}</dt><dd>{{ uiText('你选了') }} {{ checkedAnswer }} {{ uiText('平方米，计算结果是 100 平方米。') }}</dd></template>
              <template v-else><dt>{{ uiText('还可以继续') }}</dt><dd>{{ transferAnswer !== null ? uiText('你选了 {0} 平方米，还没有对照计算。', [transferAnswer]) : uiText('试试围栏翻倍后，最大的面积会怎样变化。') }}</dd></template>
            </dl>
            <div class="home-context-note"><CornerDownRight :size="18" /><p>{{ uiText('下次可以继续的问题') }}<small>{{ uiText('如果有一面墙可以借用，20 米围栏只需要围三条边，怎样围更合适？') }}</small></p></div>
            <div class="home-demo-actions"><button class="home-inline-action" type="button" @click="continueTo(1)">{{ uiText('回去再试一次') }}</button><a class="home-panel-next" href="#begin">{{ uiText('从一节原文开始') }} <ArrowRight :size="16" /></a></div>
          </div>
        </div>
      </div>
      <p class="home-story-footnote">{{ uiText('这是一个独立的学习示例，计算与解释均已预设。你的输入只留在当前页面，刷新后清空。') }}</p>
    </div>
  </section>
</template>

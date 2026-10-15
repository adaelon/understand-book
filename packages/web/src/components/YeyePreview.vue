<script setup lang="ts">
import YeyeMascot from './YeyeMascot.vue';
</script>

<template>
  <main>
    <header>
      <span class="eyebrow">页页 · 阅读伙伴</span>
      <h1>陪你把一本书读懂</h1>
      <p>安静陪伴，也带着一点好奇。</p>
    </header>

    <section class="portraits" aria-label="完整形象">
      <figure v-for="variant in (['idle', 'reading'] as const)" :key="variant">
        <YeyeMascot :variant="variant" :size="300" :data-sample="variant" />
        <figcaption>{{ variant === 'idle' ? '接着往下读' : '一起看清这一页' }}</figcaption>
      </figure>
    </section>

    <section aria-label="紧凑身份" class="identities">
      <div v-for="surface in ['paper', 'white', 'tint']" :key="surface" class="identity" :class="surface">
        <div v-for="size in [24, 32]" :key="size" class="identity-row">
          <YeyeMascot variant="avatar" :size="size" :data-sample="`avatar-${surface}-${size}`" />
          <span>问这本书 <small>{{ size }}px</small></span>
        </div>
      </div>
    </section>

    <section class="sizes" aria-label="插画尺寸">
      <figure v-for="size in [120, 128, 160, 220]" :key="size">
        <YeyeMascot :size="size" :data-sample="`body-${size}`" />
        <figcaption>{{ size }}px</figcaption>
      </figure>
    </section>

    <footer>
      <YeyeMascot variant="avatar" :decorative="false" data-sample="named-avatar" />
      <span>这一节，想先弄明白什么？</span>
    </footer>
  </main>
</template>

<style>
@import '../brand.css';
* { box-sizing: border-box; }
body { margin: 0; background: #fbfaf7; color: #332b25; font-family: system-ui, sans-serif; }
main { max-width: 1060px; padding: 48px 24px; margin: auto; }
header { margin-bottom: 28px; }
.eyebrow { color: var(--brand-accent-deep); font-size: 13px; letter-spacing: .12em; }
h1 { font-size: clamp(26px, 4vw, 38px); margin: 12px 0; letter-spacing: -.04em; }
p { color: #74675a; margin: 0; }
section { margin-top: 24px; }
figure { margin: 0; text-align: center; }
figcaption { color: #74675a; font-size: 13px; margin-top: 10px; }
.portraits { display: grid; grid-template-columns: repeat(auto-fit, minmax(300px, 1fr)); gap: 24px; }
.portraits figure { padding: 20px 0; background: #f4ede2; border-radius: 24px; }
.identities { display: grid; grid-template-columns: repeat(auto-fit, minmax(220px, 1fr)); gap: 16px; }
.identity { border-radius: 18px; border: 1px solid #e4d9cc; padding: 18px 24px; }
.identity-row { display: flex; align-items: center; gap: 12px; min-height: 44px; }
.white { background: white; }
.tint { background: var(--brand-accent-soft); }
small { color: #74675a; margin-left: 12px; font-size: 11px; }
.sizes { display: flex; align-items: end; justify-content: space-around; flex-wrap: wrap; gap: 20px; padding: 20px 0; }
footer { display: flex; align-items: center; gap: 14px; border-top: 1px solid #e4d9cc; margin-top: 16px; padding-top: 24px; }
@media (max-width: 400px) { main { padding: 28px 16px; } }
</style>

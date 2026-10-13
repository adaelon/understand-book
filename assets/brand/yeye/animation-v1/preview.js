const reduced = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
const query = new URLSearchParams(location.search);
const pinnedScene = query.get('scene');
const pinnedFrame = Number(query.get('frame'));
for (const card of document.querySelectorAll('[data-scene]')) {
  const id = card.dataset.scene;
  const loop = id !== 'success';
  const button = card.querySelector('.toggle');
  const slider = card.querySelector('.scrub');
  const clock = card.querySelector('.clock');
  const animation = lottie.loadAnimation({
    container: card.querySelector('.stage'), renderer: 'svg', loop,
    autoplay: false, path: `${id}/lottie.json`,
    rendererSettings: { preserveAspectRatio: 'xMidYMid meet', progressiveLoad: false },
  });
  let done = false;
  const label = () => { button.textContent = done ? '重播' : animation.isPaused ? '播放' : '暂停'; };
  animation.addEventListener('DOMLoaded', () => {
    card.dataset.ready = 'true';
    button.disabled = false;
    slider.max = animation.totalFrames - 1;
    if (pinnedScene) {
      animation.goToAndStop(id === pinnedScene ? Math.max(0, Math.min(pinnedFrame, animation.totalFrames - 1)) : 0, true);
    } else if (!reduced) animation.play();
    label();
  });
  animation.addEventListener('enterFrame', event => {
    slider.value = Math.floor(event.currentTime);
    clock.value = `${(event.currentTime / animation.frameRate).toFixed(1)}s`;
  });
  animation.addEventListener('complete', () => { done = true; label(); });
  animation.addEventListener('data_failed', () => { button.textContent = '加载失败'; });
  button.addEventListener('click', () => {
    if (done) { done = false; animation.goToAndPlay(0, true); }
    else if (animation.isPaused) animation.play();
    else animation.pause();
    label();
  });
  slider.addEventListener('input', () => {
    done = !loop && Number(slider.value) === animation.totalFrames - 1;
    animation.goToAndStop(Number(slider.value), true);
    label();
  });
}
for (const button of document.querySelectorAll('[data-bg]')) {
  button.addEventListener('click', () => {
    document.body.dataset.background = button.dataset.bg;
    for (const item of document.querySelectorAll('[data-bg]')) item.setAttribute('aria-pressed', String(item === button));
  });
}

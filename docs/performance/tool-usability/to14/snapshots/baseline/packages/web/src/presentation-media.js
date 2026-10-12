// Shared Reader/preview lifecycle. Returning never resumes a demo implicitly.
(() => {
  let hostVisible = true;
  const pauseAll = () => document.querySelectorAll("video").forEach(video => video.pause());
  window.addEventListener("message", event => {
    if (event.source !== parent || event.data?.channel !== "agent-presentation" || event.data.kind !== "visibility") return;
    hostVisible = event.data.visible;
    if (!hostVisible) pauseAll();
  });
  document.addEventListener("visibilitychange", () => { if (document.hidden) pauseAll(); });
  window.addEventListener("pagehide", pauseAll);
  document.addEventListener("DOMContentLoaded", () => {
    const visibility = new WeakMap();
    const installed = new WeakSet();
    const observer = new IntersectionObserver(entries => entries.forEach(entry => {
      visibility.set(entry.target, entry.isIntersecting);
      if (!entry.isIntersecting) entry.target.pause();
    }));
    const visit = (node, action) => {
      if (!(node instanceof Element)) return;
      if (node.matches("video")) action(node);
      node.querySelectorAll("video").forEach(action);
    };
    const install = video => {
      observer.observe(video);
      if (installed.has(video)) return;
      installed.add(video);
      video.controls = false; video.playsInline = true; video.autoplay = false; video.pause();
      video.addEventListener("play", () => {
        if (document.hidden || !hostVisible || visibility.get(video) === false) { video.pause(); return; }
        document.querySelectorAll("video").forEach(other => { if (other !== video) other.pause(); });
      });
    };
    visit(document.body, install);
    new MutationObserver(records => records.forEach(record => {
      record.addedNodes.forEach(node => visit(node, install));
      record.removedNodes.forEach(node => visit(node, video => {
        if (!video.isConnected) { video.pause(); observer.unobserve(video); }
      }));
    })).observe(document.body, { subtree: true, childList: true });
  });
})();

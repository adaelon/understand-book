function (config, staticCapture) {
  const send = message => parent.postMessage({ channel: config.channel, ...message }, "*");
  let observer;
  let stateReader;
  let stateRestorer;
  let restoring = true;
  let commitTimer;
  let hostGeneration;
  let editing = false;
  let zoom = 1;
  const pendingCaptures = new Map();
  window.presentation = Object.freeze({
    initialState: config.initialState,
    restoredState: config.restoredState ?? null,
    registerStateReader: reader => { stateReader = reader; },
    registerStateRestorer: restorer => { stateRestorer = restorer; },
    commitState: () => scheduleCommit(),
    // Opens the host action form; only an explicit host submission records a learner action.
    requestTeachingAction: (moveId, response) => send({ kind: "teaching-action", move_id: moveId, response }),
  });
  window.addEventListener("error", () => send({ kind: "error" }));
  window.addEventListener("unhandledrejection", () => send({ kind: "error" }));
  window.addEventListener("message", event => {
    if (event.source !== parent || event.data?.channel !== config.channel) return;
    if (event.data.kind === "zoom" && Number.isFinite(event.data.scale) && event.data.scale >= .5 && event.data.scale <= 1.5) {
      const next = event.data.scale;
      if (next !== zoom) {
        const x = window.scrollX / zoom, y = window.scrollY / zoom;
        document.documentElement.style.zoom = String(next);
        zoom = next;
        window.scrollTo(x * zoom, y * zoom);
      }
      return;
    }
    if (event.data.kind === "theme") {
      if (Number.isInteger(event.data.generation)) hostGeneration = event.data.generation;
      for (const [name, value] of Object.entries(event.data.values)) document.documentElement.style.setProperty(name, value);
    }
    if (event.data.kind === "snapshot") capture(event.data.request_id);
    if (event.data.kind === "export-static") {
      const id = event.data.request_id;
      Promise.all([document.fonts.ready, ...Array.from(document.images, image => image.decode())])
        .then(() => capture(id, true), () => send({ kind: "static-error", request_id: id, message: "图解图片资源加载失败，请重新加载后重试。" }));
    }
  });
  // The host owns source labels and actions; ordinary page rendering stays local.
  function normalizeSources() {
    observer.disconnect();
    const refs = [];
    document.querySelectorAll("[data-source-ref]").forEach(node => {
      const id = node.getAttribute("data-source-ref");
      refs.push(id);
      const source = config.sources.find(source => source.source_ref_id === id);
      node.textContent = sourceChipLabel(config.sources, id);
      node.setAttribute("title", source ? source.label : "来源不可用");
      node.setAttribute("aria-label", source ? `${node.textContent}：${source.label}` : "来源不可用");
      if (node instanceof HTMLButtonElement) node.disabled = !source;
    });
    observer.observe(document.body, { subtree: true, childList: true, characterData: true, attributes: true });
    return refs;
  }
  function capture(request_id, exporting = false) {
    if (restoring || !observer) { pendingCaptures.set(request_id, exporting); return; }
    const sourceRefs = normalizeSources();
    const controls = Array.from(document.querySelectorAll("input, textarea, select"))
      .map((node, index) => ({
      key: node.id || node.name || `control-${index}`, type: node.type,
      value: node instanceof HTMLSelectElement && node.multiple ? Array.from(node.selectedOptions, option => option.value) : node.value,
      ...(node.type === "checkbox" || node.type === "radio" ? { checked: node.checked } : {}),
      }));
    try {
    const custom = stateReader ? stateReader() : {};
    // One synchronous browser read freezes parameters and actual rendered semantics together.
    send({ kind: exporting ? "static-result" : "state", request_id, ...(exporting ? { image: staticCapture(), parameters: Array.from(document.querySelectorAll('input, textarea, select')).filter(node => !node.closest('[data-presentation-mask]') && node.getClientRects().length > 0 && getComputedStyle(node).visibility !== 'hidden' && node.type !== 'hidden').map((node, index) => ({
      label: node.getAttribute('aria-label') || Array.from(node.labels ?? [], label => label.textContent.trim()).join(' ') || `参数 ${index + 1}`,
      value: node.type === 'checkbox' || node.type === 'radio' ? (node.checked ? '已选' : '未选') : node instanceof HTMLSelectElement ? Array.from(node.selectedOptions, option => option.text).join('、') : node.value,
    })) } : {}), state: {
      values: { controls, page: custom.values ?? null },
      visible_step: custom.visible_step ?? document.querySelector("[data-presentation-step]")?.getAttribute("data-presentation-step") ?? null,
      observed_result: visibleText(document.body).trim(), source_ref_ids: sourceRefs,
    } });
    } catch (error) {
      if (exporting) send({ kind: "static-error", request_id, message: error instanceof Error ? error.message : String(error) });
      else throw error;
    }
  }
  function visibleText(node) {
    if (node.nodeType === Node.TEXT_NODE) return node.textContent;
    if (!(node instanceof Element) || node.matches("script, style, [data-source-ref]")) return "";
    const style = getComputedStyle(node);
    if (style.display === "none" || style.visibility === "hidden") return "";
    const label = [node.getAttribute("alt"), node.getAttribute("aria-label")].filter(Boolean).join(" ");
    return [label, ...Array.from(node.childNodes, visibleText)].join(" ");
  }
  function scheduleCommit() {
    if (restoring) return;
    clearTimeout(commitTimer);
    commitTimer = setTimeout(() => capture(), 0);
  }
  function isEditingTarget(node) {
    return node instanceof Element && !!node.closest("input, textarea, select, [contenteditable]:not([contenteditable='false'])");
  }
  function publishEditing(next) {
    if (!Number.isInteger(hostGeneration) || editing === next) return;
    editing = next;
    send({ kind: "editing-focus", generation: hostGeneration, editing });
  }
  document.addEventListener("click", event => {
    const node = event.target instanceof Element ? event.target.closest("[data-source-ref], a") : null;
    if (!node) return;
    event.preventDefault();
    const id = node.getAttribute("data-source-ref");
    if (config.sources.some(source => source.source_ref_id === id)) send({ kind: "source", source_ref_id: id });
  }, true);
  document.addEventListener("submit", event => event.preventDefault(), true);
  document.addEventListener("DOMContentLoaded", () => {
    // Run after the page's own DOMContentLoaded handlers installed its controls.
    setTimeout(async () => {
    try {
    restoring = true;
    if (config.restoredState) {
      const controls = config.restoredState.values?.controls ?? [];
      document.querySelectorAll("input, textarea, select").forEach((node, index) => {
        const saved = controls.find(control => control.key === (node.id || node.name || `control-${index}`) && control.type === node.type);
        if (!saved) return;
        if (node instanceof HTMLSelectElement && node.multiple) {
          Array.from(node.options).forEach(option => { option.selected = saved.value.includes(option.value); });
        } else if (node.type !== "file") node.value = saved.value;
        if ("checked" in saved) node.checked = saved.checked;
        node.dispatchEvent(new Event("input", { bubbles: true }));
        node.dispatchEvent(new Event("change", { bubbles: true }));
      });
      if (stateRestorer) await stateRestorer(config.restoredState);
      if (!stateRestorer && (config.restoredState.values?.page != null || config.restoredState.visible_step != null)) {
        send({ kind: "restore-partial" });
      }
    }
    restoring = false;
    observer = new MutationObserver(normalizeSources);
    document.addEventListener("change", scheduleCommit);
    document.addEventListener("focusin", event => publishEditing(isEditingTarget(event.target)));
    document.addEventListener("focusout", () => queueMicrotask(() => publishEditing(isEditingTarget(document.activeElement))));
    window.addEventListener("blur", () => publishEditing(false));
    document.addEventListener("visibilitychange", () => { if (document.hidden) publishEditing(false); });
    document.addEventListener("click", event => {
      if (!(event.target instanceof Element) || event.target.closest("[data-source-ref], a")) return;
      if (event.target.closest("button, summary, [role=button], [data-presentation-step]")) scheduleCommit();
    });
    normalizeSources();
    send({ kind: "ready" });
    const captures = Array.from(pendingCaptures);
    pendingCaptures.clear();
    captures.forEach(([id, mode]) => capture(id, mode));
    } catch {
      restoring = true;
      send({ kind: "error" });
    }
    });
  });
}

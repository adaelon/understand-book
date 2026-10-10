function (config, staticCapture) {
  const send = message => parent.postMessage({ channel: config.channel, ...message }, "*");
  let revision = 0;
  let observer;
  let last = "";
  let stateReader;
  let stateRestorer;
  let restoring = true;
  let commitTimer;
  let pendingFocus;
  let pendingRevision = 0;
  let acceptedBody;
  let pendingBody;
  let masks = [];
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
    if (event.data.kind === "accepted" && event.data.revision === revision) {
      observer?.disconnect();
      clearMasks();
      acceptedBody = pendingBody;
      pendingBody = undefined;
      pendingRevision = 0;
      document.documentElement.removeAttribute("data-presentation-pending");
      if (pendingFocus?.isConnected && document.hasFocus() && document.activeElement === document.body) {
        pendingFocus.focus({ preventScroll: true });
      }
      pendingFocus = undefined;
      const captures = Array.from(pendingCaptures);
      pendingCaptures.clear();
      captures.forEach(([id, mode]) => capture(id, mode));
      observer?.observe(document.body, { subtree: true, childList: true, characterData: true, attributes: true });
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
  function clearMasks() {
    for (const { node, style, ariaHidden, overlay } of masks) {
      for (const [name, value] of Object.entries(style)) node.style[name] = value;
      if (ariaHidden === null) node.removeAttribute("aria-hidden");
      else node.setAttribute("aria-hidden", ariaHidden);
      overlay.remove();
    }
    masks = [];
  }
  function maskUnaccepted() {
    if (!acceptedBody) return;
    const changed = [];
    const add = (node, old) => {
      if (node instanceof Element && old instanceof Element && !changed.some(item => item.node.contains(node))) {
        for (let i = changed.length - 1; i >= 0; i--) if (node.contains(changed[i].node)) changed.splice(i, 1);
        changed.push({ node, old });
      }
    };
    const compare = (old, node, parent, oldParent) => {
      if (!old || !node || old.nodeType !== node.nodeType || old.nodeName !== node.nodeName) {
        add(parent, oldParent); return;
      }
      if (node.nodeType === Node.TEXT_NODE) {
        if (old.textContent !== node.textContent) add(parent, oldParent);
        return;
      }
      if (!(node instanceof Element)) return;
      if (["alt", "title", "aria-label", "data-source-ref"].some(name => old.getAttribute(name) !== node.getAttribute(name))
          || old.childNodes.length !== node.childNodes.length) {
        add(node, old); return;
      }
      for (let i = 0; i < node.childNodes.length; i++) compare(old.childNodes[i], node.childNodes[i], node, old);
    };
    compare(acceptedBody, document.body, null, null);
    for (const { node, old } of changed) {
      const rect = node.getBoundingClientRect();
      const overlay = document.createElement("div");
      overlay.setAttribute("data-presentation-mask", "");
      overlay.setAttribute("data-accepted-text", old.textContent?.trim().slice(0, 16000) ?? "");
      const shadow = overlay.attachShadow({ mode: "closed" });
      document.querySelectorAll("style").forEach(style => shadow.appendChild(style.cloneNode(true)));
      const copy = old.cloneNode(true);
      copy.querySelectorAll?.("script").forEach(script => script.remove());
      shadow.appendChild(copy);
      Object.assign(overlay.style, { position: "absolute", left: `${rect.left + scrollX}px`, top: `${rect.top + scrollY}px`,
        width: `${rect.width}px`, height: `${rect.height}px`, margin: "0", zIndex: "2147483647",
        pointerEvents: "none", opacity: "1" });
      overlay.style.overflow = "hidden";
      const style = Object.fromEntries(["opacity", "width", "height", "overflow", "boxSizing"].map(name => [name, node.style[name]]));
      const ariaHidden = node.getAttribute("aria-hidden");
      node.setAttribute("aria-hidden", "true");
      node.style.opacity = "0";
      node.style.width = `${rect.width}px`;
      node.style.height = `${rect.height}px`;
      node.style.boxSizing = "border-box";
      node.style.overflow = "hidden";
      (node === document.body ? document.documentElement : document.body).appendChild(overlay);
      masks.push({ node, style, ariaHidden, overlay });
    }
  }
  function observe() {
    observer.disconnect();
    clearMasks();
    const refs = [];
    document.querySelectorAll("[data-source-ref]").forEach(node => {
      if (node.closest("[data-presentation-mask]")) return;
      const id = node.getAttribute("data-source-ref");
      refs.push(id);
      const source = config.sources.find(source => source.source_ref_id === id);
      node.textContent = sourceChipLabel(config.sources, id);
      node.setAttribute("title", source ? source.label : "来源不可用");
      node.setAttribute("aria-label", source ? `${node.textContent}：${source.label}` : "来源不可用");
      if (node instanceof HTMLButtonElement) node.disabled = !source;
    });
    const copy = document.body.cloneNode(true);
    copy.querySelectorAll("script, style, [data-source-ref], [data-presentation-mask]").forEach(node => node.remove());
    const extras = Array.from(copy.querySelectorAll("[alt], [title], [aria-label], input, textarea, select"))
      .map(node => [node.getAttribute("alt"), node.getAttribute("title"), node.getAttribute("aria-label"), node.value].filter(Boolean).join(" "));
    const text = [copy.textContent, ...extras].join("\n").trim();
    const signature = JSON.stringify([text, refs]);
    if (signature !== last) {
      last = signature;
      if (document.activeElement instanceof HTMLElement && document.activeElement !== document.body) {
        pendingFocus = document.activeElement;
      }
      pendingRevision = ++revision;
      pendingBody = document.body.cloneNode(true);
      send({ kind: "observe", revision, text, source_ref_ids: refs });
    }
    if (pendingRevision) maskUnaccepted();
    observer.observe(document.body, { subtree: true, childList: true, characterData: true, attributes: true });
    return { text, source_ref_ids: refs };
  }
  function capture(request_id, exporting = false) {
    if (restoring || !observer) { pendingCaptures.set(request_id, exporting); return; }
    const observation = observe();
    // Read the result only after the exact observed revision is accepted.
    if (pendingRevision) {
      pendingCaptures.set(request_id, exporting);
      return;
    }
    const controls = Array.from(document.querySelectorAll("input, textarea, select"))
      .filter(node => !node.closest("[data-presentation-mask]"))
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
      observed_result: visibleText(document.body).trim(), source_ref_ids: observation.source_ref_ids,
    } });
    } catch (error) {
      if (exporting) send({ kind: "static-error", request_id, message: error instanceof Error ? error.message : String(error) });
      else throw error;
    }
  }
  function visibleText(node) {
    if (node.nodeType === Node.TEXT_NODE) return node.textContent;
    if (!(node instanceof Element) || node.matches("script, style, [data-source-ref], [data-presentation-mask]")) return "";
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
    if (!pendingRevision && config.sources.some(source => source.source_ref_id === id)) send({ kind: "source", source_ref_id: id });
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
    observer = new MutationObserver(observe);
    document.addEventListener("input", observe);
    document.addEventListener("change", observe);
    document.addEventListener("change", scheduleCommit);
    document.addEventListener("focusin", event => publishEditing(isEditingTarget(event.target)));
    document.addEventListener("focusout", () => queueMicrotask(() => publishEditing(isEditingTarget(document.activeElement))));
    window.addEventListener("blur", () => publishEditing(false));
    document.addEventListener("visibilitychange", () => { if (document.hidden) publishEditing(false); });
    document.addEventListener("click", event => {
      if (!(event.target instanceof Element) || event.target.closest("[data-source-ref], a")) return;
      if (event.target.closest("button, summary, [role=button], [data-presentation-step]")) scheduleCommit();
    });
    observe();
    } catch {
      restoring = true;
      send({ kind: "error" });
    }
    });
  });
}

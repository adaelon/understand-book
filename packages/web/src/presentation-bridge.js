function (config, staticCapture, createActions) {
  const send = message => parent.postMessage({ channel: config.channel, ...message }, "*");
  let observer;
  let stateReader;
  let actionHandler;
  const parameterEdits = {};
  const parameters = (config.actionContract ?? []).filter(entry => entry.kind === "set_parameters").flatMap(entry => entry.parameters);
  let interaction;
  let stateRestorer;
  let restoring = true;
  let commitTimer;
  let hostGeneration;
  let editing = false;
  let zoom = 1;
  const pendingCaptures = new Map();
  const pendingHelp = new Map();
  let helpRequest = 0;
  function teachingRegion(regionId) {
    if (!config.teachingBinding?.regions.some(region => region.region_id === regionId)) return null;
    return Array.from(document.querySelectorAll('[data-teaching-region], [id]'))
      .find(node => node.getAttribute('data-teaching-region') === regionId || node.id === regionId) ?? null;
  }
  window.presentation = Object.freeze({
    initialState: config.initialState,
    restoredState: config.restoredState ?? null,
    registerActionHandler: handler => { actionHandler = handler; },
    registerStateReader: reader => { stateReader = reader; },
    registerStateRestorer: restorer => { stateRestorer = restorer; },
    commitState: () => scheduleCommit(),
    // Opens the host action form; only an explicit host submission records a learner action.
    requestTeachingAction: (moveId, response) => send({ kind: "teaching-action", move_id: moveId, response }),
    // Only teaching help explicitly revealed on the page is reported. Moving
    // sliders, expanding ordinary prose or locating a region is not an answer.
    reportTeachingHelp: (moveId, kind, regionId) => new Promise((resolve, reject) => {
      const region = teachingRegion(regionId);
      const bounds = region?.getBoundingClientRect();
      const text = region ? visibleText(region).trim() : '';
      if (!region || !bounds || !text || !['hint', 'explanation'].includes(kind)
        || bounds.bottom <= 0 || bounds.top >= innerHeight || bounds.right <= 0 || bounds.left >= innerWidth) {
        reject(new Error('帮助区域尚未显示。')); return;
      }
      const requestId = ++helpRequest;
      const timer = setTimeout(() => { pendingHelp.delete(requestId); reject(new Error('帮助展示记录未确认。')); }, 15000);
      pendingHelp.set(requestId, { resolve, reject, timer });
      send({ kind: 'teaching-help', request_id: requestId, move_id: moveId, help_kind: kind, region_id: regionId, text });
    }),
  });
  window.addEventListener("error", () => send({ kind: "error" }));
  window.addEventListener("unhandledrejection", () => send({ kind: "error" }));
  window.addEventListener("message", event => {
    if (event.source !== parent || event.data?.channel !== config.channel) return;
    if (event.data.kind === 'interaction-cancel') {
      if (interaction?.id === event.data.request_id) interaction.controller.abort();
      return;
    }
    if (event.data.kind === 'interaction') {
      const id = event.data.request_id;
      if (restoring || interaction) {
        send({ kind: 'interaction-result', request_id: id, status: 'failed', reason: 'The page is not ready for an operation.' });
        return;
      }
      const current = { id, controller: new AbortController() };
      interaction = current;
      clearTimeout(commitTimer);
      let before = {};
      try { before = { ...stateReader?.()?.values }; } catch {}
      const actions = createActions(config.actionContract, () => stateReader?.(), () => actionHandler, () => parameterEdits);
      actions.execute(event.data.request, event.data.expected_values, current.controller.signal, event.data.expected_edits).then(status => {
        if (!current.controller.signal.aborted) {
          const action = event.data.request.action;
          const after = stateReader?.()?.values ?? {};
          const support = status !== 'applied' || action?.kind === 'highlight' ? [] :
            (config.teachingBinding?.regions ?? []).filter(region =>
              ['reveal', 'annotate'].includes(action.kind) ? region.region_id === action.region_id :
                region.state_fields.some(key => before[key] !== after[key])
            ).flatMap(region => {
              const node = teachingRegion(region.region_id);
              const text = node?.getClientRects().length ? visibleText(node).trim() : '';
              return text ? [{ region_id: region.region_id, text }] : [];
            });
          capture(id, false, { status, support });
        }
      }).catch(failure => {
        const result = { status: current.controller.signal.aborted ? 'cancelled' : 'failed', reason: String(failure.message ?? failure) };
        if (current.controller.signal.aborted) send({ kind: 'interaction-result', request_id: id, ...result });
        else capture(id, false, result);
      }).finally(() => { if (interaction === current) interaction = undefined; });
      return;
    }
    if (event.data.kind === 'locate-region') {
      const region = teachingRegion(event.data.region_id);
      if (region) {
        region.scrollIntoView({ block: 'center', behavior: 'smooth' });
        region.animate([{ outline: '3px solid var(--accent)', outlineOffset: '5px' },
          { outline: '3px solid transparent', outlineOffset: '5px' }], { duration: 1600 });
      }
      send({ kind: 'region-located', region_id: event.data.region_id, located: !!region });
      return;
    }
    if (event.data.kind === 'teaching-help-recorded') {
      const pending = pendingHelp.get(event.data.request_id);
      if (pending) {
        clearTimeout(pending.timer); pendingHelp.delete(event.data.request_id);
        if (event.data.error) pending.reject(new Error(event.data.error)); else pending.resolve();
      }
      return;
    }
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
  function capture(request_id, exporting = false, result) {
    if (restoring || !observer) { pendingCaptures.set(request_id, exporting); return; }
    const sourceRefs = normalizeSources();
    const controls = Array.from(document.querySelectorAll("input, textarea, select"))
      .map((node, index) => ({
      key: node.id || node.name || `control-${index}`, type: node.type,
      value: node instanceof HTMLSelectElement && node.multiple ? Array.from(node.selectedOptions, option => option.value) : node.value,
      ...(node.type === "checkbox" || node.type === "radio" ? { checked: node.checked } : {}),
      }));
    try {
    // A failed reader must not reuse an earlier semantic scene or hide the DOM
    // that is still visible now. Read afresh for every capture.
    let custom = {};
    try { custom = stateReader ? stateReader() ?? {} : {}; } catch {}
    // One synchronous browser read freezes parameters and actual rendered semantics together.
    send({ kind: result ? "interaction-result" : exporting ? "static-result" : "state", request_id, ...result, ...(exporting ? { image: staticCapture(), parameters: Array.from(document.querySelectorAll('input, textarea, select')).filter(node => !node.closest('[data-presentation-mask]') && node.getClientRects().length > 0 && getComputedStyle(node).visibility !== 'hidden' && node.type !== 'hidden').map((node, index) => ({
      label: node.getAttribute('aria-label') || Array.from(node.labels ?? [], label => label.textContent.trim()).join(' ') || `参数 ${index + 1}`,
      value: node.type === 'checkbox' || node.type === 'radio' ? (node.checked ? '已选' : '未选') : node instanceof HTMLSelectElement ? Array.from(node.selectedOptions, option => option.text).join('、') : node.value,
    })) } : {}), state: {
      values: { controls, page: custom.values ?? null, parameter_edits: { ...parameterEdits } },
      visible_step: custom.visible_step ?? document.querySelector("[data-presentation-step]")?.getAttribute("data-presentation-step") ?? null,
      observed_result: visibleText(document.body).trim(), source_ref_ids: sourceRefs,
      ...(custom.observation ? { observation: custom.observation } : {}),
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
    if (restoring || interaction) return;
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
    // Only human control edits invalidate an observation. Timer/playback updates
    // still appear in the snapshot, but do not masquerade as conflicting edits.
    for (const kind of ['input', 'change', 'click']) document.addEventListener(kind, event => {
      if (!event.isTrusted || interaction || !parameters.length) return;
      let before;
      try { before = { ...stateReader?.()?.values }; } catch { return; }
      queueMicrotask(() => {
        let after;
        try { after = stateReader?.()?.values ?? {}; } catch { return; }
        for (const key of parameters) {
          const target = event.target;
          if (before[key] !== after[key] || ((kind === 'input' || kind === 'change') && target instanceof Element && target.getAttribute('data-presentation-parameter') === key)) {
            parameterEdits[key] = (parameterEdits[key] ?? 0) + 1;
          }
        }
      });
    }, true);
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

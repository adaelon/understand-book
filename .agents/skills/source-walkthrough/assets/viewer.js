(() => {
  const tour = JSON.parse(document.getElementById('tour-data').textContent);
  const byId = id => document.getElementById(id);
  const sourceFiles = new Map(tour.files.map(file => [file.path, file]));
  const stepsById = new Map(tour.steps.map(step => [step.id, step]));
  const stepIndices = new Map(tour.steps.map((step, index) => [step.id, index]));
  const mobile = window.matchMedia('(max-width: 760px)');
  const workspace = document.querySelector('.workspace');
  const codeViewport = byId('code-viewport');
  const codeLines = byId('code-lines');
  const flowViewport = byId('flow-viewport');
  const flowMap = byId('flow-map');
  const nodeButtons = new Map();
  const fileTabs = new Map();
  let active;
  let visibleFile;
  let markedLines = [];

  function element(tag, className, text) {
    const node = document.createElement(tag);
    if (className) node.className = className;
    if (text != null) node.textContent = text;
    return node;
  }

  function fileName(path) { return path.split('/').pop(); }
  function lineRange(step) { return step.start === step.end ? 'L' + step.start : 'L' + step.start + '–' + step.end; }
  function stepInUrl() { return new URLSearchParams(location.hash.slice(1)).get('step'); }
  function rememberStep(id) {
    if (stepInUrl() !== id) history.replaceState(null, '', '#step=' + encodeURIComponent(id));
  }

  // Text nodes keep the original characters selectable and preserve HTML shown as source.
  const tokens = /\/\/[^\n]*|\/\*.*?(?:\*\/|$)|"(?:\\.|[^"\\])*"?|'(?:\\.|[^'\\])*'?|\x60(?:\\.|[^\x60\\])*\x60?|\b(?:async|await|break|case|catch|class|const|continue|def|else|enum|export|false|fn|for|from|function|if|impl|import|in|interface|let|match|new|null|pub|return|self|static|struct|switch|throw|true|try|type|use|var|while|yield)\b|\b\d+(?:\.\d+)?\b/g;
  function paintLine(container, line, path) {
    if (/^\s*\*/.test(line) || (/\.py$|\.rb$|\.sh$/.test(path) && /^\s*#/.test(line))) {
      container.append(element('span', 'token-comment', line));
      return;
    }
    let offset = 0;
    for (const match of line.matchAll(tokens)) {
      if (match.index > offset) container.append(line.slice(offset, match.index));
      const value = match[0];
      let kind = 'keyword';
      if (value[0] === '/') kind = 'comment';
      else if (/^["'\x60]/.test(value)) kind = 'string';
      else if (/^\d/.test(value)) kind = 'number';
      container.append(element('span', 'token-' + kind, value));
      offset = match.index + value.length;
    }
    if (offset < line.length) container.append(line.slice(offset));
  }

  function openFile(path) {
    if (visibleFile === path) return;
    visibleFile = path;
    const file = sourceFiles.get(path);
    const steps = tour.steps.filter(step => step.file === path);
    const rows = document.createDocumentFragment();
    file.lines.forEach((line, index) => {
      const row = element('div', 'code-line');
      const lineNumber = index + 1;
      const step = steps.find(item => item.start <= lineNumber && lineNumber <= item.end);
      if (step) {
        row.classList.add('linked');
        row.dataset.step = step.id;
        row.title = '查看讲解 · ' + step.title;
      }
      const number = element('span', 'line-number', lineNumber);
      number.setAttribute('aria-hidden', 'true');
      const text = element('code', 'line-text');
      paintLine(text, line, path);
      row.append(number, text);
      rows.append(row);
    });
    codeLines.replaceChildren(rows);
    markedLines = [];
    byId('source-path').textContent = path;
    byId('file-stat').textContent = file.lines.length + ' 行';
    for (const [filePath, tab] of fileTabs) tab.setAttribute('aria-pressed', String(filePath === path));
    const tab = fileTabs.get(path);
    const tabs = tab.parentElement;
    tabs.scrollLeft += tab.getBoundingClientRect().left - tabs.getBoundingClientRect().left -
      Math.max(0, (tabs.clientWidth - tab.offsetWidth) / 2);
    codeViewport.scrollLeft = 0;
  }

  function markCode(step) {
    for (const row of markedLines) row.classList.remove('selected');
    markedLines = [];
    for (let number = step.start; number <= step.end; number++) {
      const row = codeLines.children[number - 1];
      row.classList.add('selected');
      markedLines.push(row);
    }
    byId('line-range').textContent = lineRange(step);
  }

  function revealCode() {
    if (!codeViewport.clientHeight || !markedLines.length) return;
    const first = markedLines[0];
    const last = markedLines[markedLines.length - 1];
    const height = last.offsetTop + last.offsetHeight - first.offsetTop;
    // Leave context after a selection that falls on the final lines of a file.
    codeLines.style.paddingBottom = Math.round(codeViewport.clientHeight * .33) + 'px';
    const margin = Math.max(9, Math.min(codeViewport.clientHeight * .30, (codeViewport.clientHeight - height) / 2));
    codeViewport.scrollTop = first.offsetTop - margin;
  }

  function keepNodeVisible() {
    const button = nodeButtons.get(active.id);
    const area = flowViewport.getBoundingClientRect();
    const target = button.getBoundingClientRect();
    if (!area.height) return;
    if (target.bottom > area.bottom - 9) flowViewport.scrollTop += target.bottom - area.bottom + 9;
    if (target.top < area.top + 9) flowViewport.scrollTop += target.top - area.top - 9;
    if (target.right > area.right - 9) flowViewport.scrollLeft += target.right - area.right + 9;
    if (target.left < area.left + 9) flowViewport.scrollLeft += target.left - area.left - 9;
  }

  function setView(view) {
    workspace.dataset.view = view;
    for (const button of document.querySelectorAll('.mobile-switch button')) {
      button.setAttribute('aria-pressed', String(button.dataset.view === view));
    }
    if (view === 'code') revealCode();
    else {
      drawEdges();
      keepNodeVisible();
    }
  }

  function selectStep(id, origin) {
    const step = stepsById.get(id);
    if (!step) return;
    const index = stepIndices.get(id);
    if (active) nodeButtons.get(active.id).setAttribute('aria-pressed', 'false');
    active = step;
    nodeButtons.get(id).setAttribute('aria-pressed', 'true');
    openFile(step.file);
    markCode(step);
    byId('progress').textContent = '步骤 ' + (index + 1) + ' / ' + tour.steps.length;
    byId('step-title').textContent = step.title;
    byId('explanation').textContent = step.explanation;
    byId('source-jump').textContent = '↗ ' + fileName(step.file) + ' · ' + lineRange(step);
    byId('previous').disabled = index === 0;
    byId('next').disabled = index === tour.steps.length - 1;
    if (origin !== 'initial') rememberStep(id);
    if (mobile.matches && origin === 'node') {
      setView('code');
      byId('code-viewport').focus({ preventScroll: true });
    }
    revealCode();
    drawEdges();
    keepNodeVisible();
  }

  function makeGraph() {
    const levels = tour.steps.map(() => 0);
    tour.steps.forEach((step, index) => {
      for (const id of step.next) {
        const target = stepIndices.get(id);
        if (target > index) levels[target] = Math.max(levels[target], levels[index] + 1);
      }
    });
    const rows = [];
    tour.steps.forEach((step, index) => {
      const level = levels[index];
      if (!rows[level]) rows[level] = element('div', 'flow-row');
      const node = element('button', 'node');
      node.type = 'button';
      node.dataset.step = step.id;
      node.setAttribute('aria-pressed', 'false');
      node.title = step.file + ' · ' + lineRange(step);
      node.append(element('span', 'node-title', step.title), element('span', 'node-symbol', step.symbol));
      rows[level].append(node);
      nodeButtons.set(step.id, node);
    });
    byId('flow-rows').append(...rows);
    const widest = Math.max(...rows.map(row => row.childElementCount));
    if (widest > 2) flowMap.style.minWidth = (widest * 136 + 50) + 'px';
  }

  function drawEdges() {
    const bounds = flowMap.getBoundingClientRect();
    if (!bounds.width || !bounds.height) return;
    const canvas = byId('edges');
    const scale = window.devicePixelRatio || 1;
    canvas.width = Math.round(bounds.width * scale);
    canvas.height = Math.round(bounds.height * scale);
    const ctx = canvas.getContext('2d');
    ctx.scale(scale, scale);
    ctx.lineJoin = 'round';
    const boxes = new Map();
    for (const [id, button] of nodeButtons) {
      const rect = button.getBoundingClientRect();
      boxes.set(id, { x: rect.left - bounds.left + rect.width / 2, top: rect.top - bounds.top,
        bottom: rect.bottom - bounds.top, right: rect.right - bounds.left, cy: rect.top - bounds.top + rect.height / 2 });
    }
    for (const step of tour.steps) {
      for (const targetId of step.next) {
        const a = boxes.get(step.id);
        const b = boxes.get(targetId);
        const selected = step.id === active?.id || targetId === active?.id;
        ctx.strokeStyle = ctx.fillStyle = selected ? '#a45e35' : '#77917c';
        ctx.lineWidth = selected ? 1.7 : 1.2;
        const forward = b.top >= a.bottom;
        ctx.beginPath();
        if (forward) {
          const midY = (a.bottom + b.top) / 2;
          ctx.moveTo(a.x, a.bottom);
          ctx.lineTo(a.x, midY);
          ctx.lineTo(b.x, midY);
          ctx.lineTo(b.x, b.top - 2);
        } else {
          const lane = bounds.width - 8;
          ctx.moveTo(a.right, a.cy);
          ctx.lineTo(lane, a.cy);
          if (targetId === step.id) {
            ctx.lineTo(lane, b.top - 8);
            ctx.lineTo(b.x, b.top - 8);
            ctx.lineTo(b.x, b.top - 2);
          } else {
            ctx.lineTo(lane, b.cy);
            ctx.lineTo(b.right + 2, b.cy);
          }
        }
        ctx.stroke();
        ctx.beginPath();
        if (forward || targetId === step.id) {
          ctx.moveTo(b.x, b.top);
          ctx.lineTo(b.x - 3.5, b.top - 6);
          ctx.lineTo(b.x + 3.5, b.top - 6);
        } else {
          ctx.moveTo(b.right, b.cy);
          ctx.lineTo(b.right + 6, b.cy - 3.5);
          ctx.lineTo(b.right + 6, b.cy + 3.5);
        }
        ctx.closePath();
        ctx.fill();
      }
    }
  }

  byId('project').textContent = tour.project;
  byId('lesson-title').textContent = tour.title;
  byId('summary').textContent = tour.summary;
  byId('node-count').textContent = tour.steps.length + ' 个节点';
  document.title = tour.title + ' · ' + tour.project;
  makeGraph();
  const basenames = tour.files.map(file => fileName(file.path));
  for (const file of tour.files) {
    const name = fileName(file.path);
    const label = basenames.indexOf(name) === basenames.lastIndexOf(name) ? name : file.path;
    const tab = element('button', 'file-tab', label);
    tab.type = 'button';
    tab.title = file.path;
    tab.addEventListener('click', () => {
      if (active.file !== file.path) selectStep(tour.steps.find(step => step.file === file.path).id, 'file');
    });
    byId('file-tabs').append(tab);
    fileTabs.set(file.path, tab);
  }
  byId('flow-rows').addEventListener('click', event => {
    const node = event.target.closest('button[data-step]');
    if (node) selectStep(node.dataset.step, 'node');
  });
  codeLines.addEventListener('click', event => {
    const row = event.target.closest('[data-step]');
    if (row && !window.getSelection()?.toString()) selectStep(row.dataset.step, 'source');
  });
  function advance(direction) {
    const target = tour.steps[stepIndices.get(active.id) + direction];
    if (target) selectStep(target.id, 'pager');
  }
  byId('previous').addEventListener('click', () => advance(-1));
  byId('next').addEventListener('click', () => advance(1));
  document.addEventListener('keydown', event => {
    if (event.altKey || event.ctrlKey || event.metaKey || event.shiftKey || codeViewport.contains(event.target)) return;
    if (event.key === 'ArrowRight' || event.key === 'ArrowLeft') {
      event.preventDefault();
      advance(event.key === 'ArrowRight' ? 1 : -1);
      if (event.target.closest('.node')) nodeButtons.get(active.id).focus({ preventScroll: true });
    }
  });
  for (const button of document.querySelectorAll('.mobile-switch button')) {
    button.addEventListener('click', () => setView(button.dataset.view));
  }
  byId('source-jump').addEventListener('click', () => {
    if (mobile.matches) setView('code');
    revealCode();
    codeViewport.focus({ preventScroll: true });
  });
  window.addEventListener('hashchange', () => selectStep(stepInUrl() || tour.steps[0].id, 'initial'));
  selectStep(stepsById.has(stepInUrl()) ? stepInUrl() : tour.steps[0].id, 'initial');
  new ResizeObserver(() => { drawEdges(); keepNodeVisible(); }).observe(flowMap);
  new ResizeObserver(revealCode).observe(codeViewport);
})();

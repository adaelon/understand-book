function (contract, read, handler, edits = () => ({})) {
  const entries = contract ?? [];
  const region = id => Array.from(document.querySelectorAll('[data-teaching-region],[id]'))
    .find(node => node.getAttribute('data-teaching-region') === id || node.id === id);
  const control = key => Array.from(document.querySelectorAll('input,select,textarea'))
    .find(node => node.getAttribute('data-presentation-parameter') === key);
  const invocation = id => Array.from(document.querySelectorAll('[data-presentation-action]'))
    .find(node => node.getAttribute('data-presentation-action') === id);
  function inspect() {
    const problems = [];
    for (const entry of entries) {
      if (entry.kind !== 'highlight' && typeof handler() !== 'function') problems.push('Register registerActionHandler for declared actions.');
      if (entry.kind === 'set_parameters') {
        for (const key of entry.parameters) {
          if (!control(key) || !(key in (read()?.values ?? {}))) problems.push(`Parameter ${key} needs a bound control and reader value.`);
        }
      } else if (entry.kind === 'invoke') {
        if (!invocation(entry.action_id)) problems.push(`Action ${entry.action_id} needs a bound control.`);
      } else if (!region(entry.region_id)) problems.push(`Region ${entry.region_id} does not exist.`);
    }
    return problems;
  }
  function locate(target) {
    target.scrollIntoView({ block: 'center', behavior: 'instant' });
    target.animate?.([{ outline: '3px solid var(--accent)', outlineOffset: '5px' },
      { outline: '3px solid transparent', outlineOffset: '5px' }], { duration: 1600 });
  }
  async function execute(request, expected, signal, expectedEdits) {
    signal?.throwIfAborted();
    const problems = inspect();
    if (problems.length) throw new Error(problems.join(' '));
    if (request.operation === 'observe') return 'observed';
    const action = request.action;
    const declared = entries.some(entry => entry.kind === action.kind && (
      action.kind === 'set_parameters' ? Object.keys(action.values).length > 0 && Object.keys(action.values).every(key => entries.filter(item => item.kind === 'set_parameters').flatMap(item => item.parameters).includes(key))
        : action.kind === 'invoke' ? entry.action_id === action.action_id : entry.region_id === action.region_id));
    if (!declared) throw new Error('This action is not declared by this content version.');
    const values = read()?.values ?? {};
    if (Object.entries(expected ?? {}).some(([key, value]) => values[key] !== value
      && (!expectedEdits || (edits()[key] ?? 0) !== (expectedEdits[key] ?? 0)))) return 'conflict';
    if (action.kind === 'set_parameters') {
      for (const [key, value] of Object.entries(action.values)) {
        const node = control(key);
        if (!node || node.disabled || (values[key] !== null && typeof value !== typeof values[key])) throw new Error(`Parameter ${key} is unavailable or has the wrong type.`);
        if (typeof value === 'number' && (!Number.isFinite(value)
          || (node.min !== '' && node.min != null && value < Number(node.min))
          || (node.max !== '' && node.max != null && value > Number(node.max)))) throw new Error(`Parameter ${key} is outside its control range.`);
      }
    }
    if (action.kind === 'invoke' && invocation(action.action_id)?.disabled) throw new Error('This operation is currently disabled.');
    if (action.kind === 'highlight') locate(region(action.region_id));
    else await handler()(action, { signal });
    signal?.throwIfAborted();
    if (action.kind === 'set_parameters' && Object.entries(action.values).some(([key, value]) => read()?.values?.[key] !== value)) {
      throw new Error('The handler did not apply the requested parameters.');
    }
    if (['reveal', 'annotate'].includes(action.kind)) {
      const target = region(action.region_id);
      if (!target || !target.getClientRects().length || getComputedStyle(target).visibility === 'hidden' || !target.innerText?.trim()) {
        throw new Error('The requested content has not become visible.');
      }
      locate(target);
    }
    return 'applied';
  }
  return { inspect, execute, locate };
}

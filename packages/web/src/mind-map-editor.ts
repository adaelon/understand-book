import { computed, ref, type Ref } from 'vue';
import type { BoardDocument } from './generated/BoardDocument';
import type { BoardItem } from './generated/BoardItem';
import type { BoardEditOperation } from './generated/BoardEditOperation';
import type { BoardTopicInput } from './generated/BoardTopicInput';
import type { BoardView } from './generated/BoardView';

export const newTopicId = () => `topic-${crypto.randomUUID()}`;
export function topicInput(title = '新主题', itemId = newTopicId()): BoardTopicInput {
  return { item_id: itemId, title, style: { color: null, bold: false }, based_on: [], source_bindings: [] };
}
export function topicChildren(item?: BoardItem): string[] { return item?.target.kind === 'topic' ? item.target.children : []; }
export function parentOf(items: BoardItem[], id: string) { return items.find(item => topicChildren(item).includes(id)); }
export function branchIds(items: BoardItem[], id: string): string[] {
  const byId = new Map(items.map(item => [item.item_id, item])), result: string[] = [], seen = new Set<string>();
  function walk(current: string) { if (seen.has(current) || !byId.has(current)) return; seen.add(current); result.push(current); topicChildren(byId.get(current)).forEach(walk); }
  walk(id); return result;
}
export function selectionRoots(items: BoardItem[], ids: string[]): string[] {
  const selected = new Set(ids), parents = new Map<string, string>();
  for (const item of items) for (const child of topicChildren(item)) parents.set(child, item.item_id);
  return [...selected].filter(id => { let ancestor = parents.get(id); while (ancestor) { if (selected.has(ancestor)) return false; ancestor = parents.get(ancestor); } return true; });
}
/** A drop index describes the dragged item alone. Resolve a stable unselected neighbor before moving several roots. */
export function moveSelectionOperations(document: BoardDocument, ids: string[], draggedId: string, parentId: string, dropIndex: number, indexExcludesSelection = false): BoardEditOperation[] {
  const roots = selectionRoots(document.items, ids.filter(id => !!parentOf(document.items, id)));
  const moving = new Set(roots), parent = document.items.find(item => item.item_id === parentId);
  const withoutDragged = topicChildren(parent).filter(id => id !== draggedId);
  const index = indexExcludesSelection ? dropIndex : withoutDragged.slice(0, dropIndex).filter(id => !moving.has(id)).length;
  const remaining = withoutDragged.filter(id => !moving.has(id)), anchor = remaining[index];
  let projection = document; const operations: BoardEditOperation[] = [];
  for (const id of roots) {
    const ordered = topicChildren(projection.items.find(item => item.item_id === parentId)).filter(child => child !== id);
    const operation: BoardEditOperation = { kind: 'move_subtree', item_id: id, parent_item_id: parentId, index: anchor ? ordered.indexOf(anchor) : ordered.length };
    projection = applyMindMapOperations(projection, [operation]); operations.push(operation);
  }
  return operations;
}
function clone<T>(value: T): T { return JSON.parse(JSON.stringify(value)); }
function createItem(input: BoardTopicInput): BoardItem {
  return { item_id: input.item_id, title: input.title, readable_content: '', position: { x: 0, y: 0 }, size: { width: 160, height: 48 },
    created_at: new Date().toISOString(), explanation_blocks: [], source_bindings: clone(input.source_bindings ?? []),
    target: { kind: 'topic', children: [], style: input.style ?? { color: null, bold: false }, based_on: input.based_on ?? [] } };
}
/** Optimistic projection of the same semantic batch; the service validates the authoritative transaction. */
export function applyMindMapOperations(document: BoardDocument, operations: BoardEditOperation[]): BoardDocument {
  const next = clone(document); next.mind_maps ??= [];
  const find = (id: string) => next.items.find(item => item.item_id === id);
  const children = (id: string) => { const item = find(id); if (!item || item.target.kind !== 'topic') throw new Error('主题已被移除，请重新选择。'); return item.target.children; };
  function insert(parent: string, index: number, item: BoardItem) { const ordered = children(parent); ordered.splice(Math.max(0, Math.min(index, ordered.length)), 0, item.item_id); next.items.push(item); }
  for (const operation of operations) {
    switch (operation.kind) {
      case 'create_mind_map': if (find(operation.root.item_id)) throw new Error('这个主题已保存，请重新载入查看。'); next.mind_maps.push(clone(operation.map)); next.items.push(createItem(operation.root)); break;
      case 'insert_topic': if (find(operation.topic.item_id)) throw new Error('这个主题已保存，请重新载入查看。'); insert(operation.parent_item_id, operation.index, createItem(operation.topic)); break;
      case 'update_topic': { const item = find(operation.item_id); if (!item || item.target.kind !== 'topic') throw new Error('主题已被移除，请重新选择。'); if (operation.title !== undefined && operation.title !== null) item.title = operation.title; if (operation.style) item.target.style = clone(operation.style); break; }
      case 'move_subtree': {
        if (branchIds(next.items, operation.item_id).includes(operation.parent_item_id)) throw new Error('不能把主题移入自己的分支。');
        const old = parentOf(next.items, operation.item_id); if (!old) throw new Error('中心主题不能移入另一分支。');
        const oldChildren = children(old.item_id); oldChildren.splice(oldChildren.indexOf(operation.item_id), 1);
        const ordered = children(operation.parent_item_id); ordered.splice(Math.max(0, Math.min(operation.index, ordered.length)), 0, operation.item_id); break;
      }
      case 'copy_subtree': {
        const ids = new Set(branchIds(next.items, operation.item_id)), remap = (id: string) => `${operation.id_prefix}-${id}`;
        const copies = next.items.filter(item => ids.has(item.item_id)).map(item => {
          const copy = clone(item); copy.item_id = remap(item.item_id); if (copy.target.kind === 'topic') copy.target.children = copy.target.children.map(remap); return copy;
        });
        const ordered = children(operation.parent_item_id); ordered.splice(Math.max(0, Math.min(operation.index, ordered.length)), 0, remap(operation.item_id));
        next.items.push(...copies);
        for (const link of [...next.links]) if (!link.note_relation_id && link.from.kind === 'item' && link.to.kind === 'item' && ids.has(link.from.item_id) && ids.has(link.to.item_id)) next.links.push({ ...clone(link), link_id: remap(link.link_id), from: { kind: 'item', item_id: remap(link.from.item_id) }, to: { kind: 'item', item_id: remap(link.to.item_id) } });
        break;
      }
      case 'remove_subtree': {
        const removed = new Set(branchIds(next.items, operation.item_id));
        for (const item of next.items) if (item.target.kind === 'topic') item.target.children = item.target.children.filter(id => !removed.has(id));
        next.items = next.items.filter(item => !removed.has(item.item_id)); next.mind_maps = next.mind_maps.filter(map => !removed.has(map.root_item_id));
        const missing = (target: { kind: string; item_id?: string; child_item_id?: string; link_id?: string }) => removed.has(target.item_id ?? target.child_item_id ?? '') || (target.kind === 'link' && !next.links.some(link => link.link_id === target.link_id));
        next.links = next.links.filter(link => !missing(link.from) && !missing(link.to));
        next.mappings = next.mappings.filter(mapping => !removed.has(mapping.explanation.item_id) && !missing(mapping.visual));
        next.routes = next.routes.map(route => ({ ...route, steps: route.steps.filter(step => !step.explanation || !removed.has(step.explanation.item_id)).map(step => ({ ...step, reveal_targets: step.reveal_targets.filter(target => !missing(target)), focus_targets: step.focus_targets.filter(target => !missing(target)) })) })); break;
      }
      case 'set_mind_map': { const map = next.mind_maps.find(entry => entry.map_id === operation.map_id); if (!map) throw new Error('导图已被移除。'); if (operation.layout) map.layout = operation.layout; if (operation.anchor) map.anchor = clone(operation.anchor); if (operation.branch_sides) map.branch_sides = clone(operation.branch_sides); break; }
      case 'put_link': next.links = [...next.links.filter(link => link.link_id !== operation.link.link_id), clone(operation.link)]; break;
      case 'remove_link': next.links = next.links.filter(link => link.link_id !== operation.link_id); break;
      case 'put_mapping': next.mappings = [...next.mappings.filter(mapping => mapping.mapping_id !== operation.mapping.mapping_id), clone(operation.mapping)]; break;
      case 'remove_mapping': next.mappings = next.mappings.filter(mapping => mapping.mapping_id !== operation.mapping_id); break;
      case 'put_route': next.routes = [...next.routes.filter(route => route.route_id !== operation.route.route_id), clone(operation.route)]; break;
      case 'remove_route': next.routes = next.routes.filter(route => route.route_id !== operation.route_id); break;
    }
  }
  return next;
}

type Dependency = { key: string; value: string };
function dependencyValue(document: BoardDocument, key: string): string {
  const [kind, id, field] = key.split('|');
  if (kind === 'topic') { const item = document.items.find(item => item.item_id === id); return JSON.stringify(field === 'title' ? item?.title : field === 'children' ? item && topicChildren(item) : field === 'style' ? item?.target.kind === 'topic' && item.target.style : item ? { title: item.title, target: item.target, source_bindings: item.source_bindings } : null) ?? 'undefined'; }
  const values = kind === 'map' ? document.mind_maps : kind === 'link' ? document.links : kind === 'mapping' ? document.mappings : document.routes;
  return JSON.stringify(values?.find(value => (value as unknown as Record<string, unknown>)[`${kind === 'map' ? 'map' : kind}_id`] === id) ?? null);
}
function dependencies(document: BoardDocument, operations: BoardEditOperation[]): Dependency[] {
  const keys = new Set<string>(); const current = document;
  const topic = (id: string, field = 'all') => keys.add(`topic|${id}|${field}`);
  for (const op of operations) {
    if (op.kind === 'create_mind_map') { keys.add(`map|${op.map.map_id}`); topic(op.root.item_id); }
    else if (op.kind === 'insert_topic') topic(op.parent_item_id, 'children');
    else if (op.kind === 'update_topic') { if (op.title != null) topic(op.item_id, 'title'); if (op.style) topic(op.item_id, 'style'); }
    else if (op.kind === 'move_subtree') { topic(op.item_id); topic(op.parent_item_id, 'children'); const parent = parentOf(current.items, op.item_id); if (parent) topic(parent.item_id, 'children'); }
    else if (op.kind === 'copy_subtree' || op.kind === 'remove_subtree') { branchIds(current.items, op.item_id).forEach(id => topic(id)); const parent = parentOf(current.items, op.item_id); if (parent) topic(parent.item_id, 'children'); if (op.kind === 'copy_subtree') topic(op.parent_item_id, 'children'); }
    else if (op.kind === 'set_mind_map') keys.add(`map|${op.map_id}`);
    else if (op.kind === 'put_link' || op.kind === 'remove_link') keys.add(`link|${op.kind === 'put_link' ? op.link.link_id : op.link_id}`);
    else if (op.kind === 'put_mapping' || op.kind === 'remove_mapping') keys.add(`mapping|${op.kind === 'put_mapping' ? op.mapping.mapping_id : op.mapping_id}`);
    else if (op.kind === 'put_route' || op.kind === 'remove_route') keys.add(`route|${op.kind === 'put_route' ? op.route.route_id : op.route_id}`);
  }
  // Capture the batch's pre-state. Operations within the batch intentionally see preceding operations.
  return [...keys].map(key => ({ key, value: dependencyValue(document, key) }));
}
type QueueEntry = { operations: BoardEditOperation[]; action: string; dependencies: Dependency[] };
type EditApi = { boardEdit(request: { board_id: string; expected_revision: number; operations: BoardEditOperation[]; action: string }): Promise<BoardDocument>; boardUndo(boardId: string, revision: number): Promise<BoardDocument>; boardRedo(boardId: string, revision: number): Promise<BoardDocument> };
/** Keeps draft text outside snapshots and serializes edits so successive local operations share the saved revision. */
export function useMindMapEditor(board: Ref<BoardDocument | null>, view: Ref<BoardView>, api: EditApi, saveOther: () => Promise<boolean>) {
  const queue = ref<QueueEntry[]>([]), saving = ref(false), error = ref(''), notice = ref('');
  const editingId = ref<string | null>(null), draftTitle = ref(''), draftBase = ref(''), draftConflict = ref(false);
  let authoritative: BoardDocument | null = null, savingTask: Promise<boolean> | null = null, conflicted = false;
  let editInFlight = false, deferredDocument: BoardDocument | null = null;
  const dirty = computed(() => !!queue.value.length || (editingId.value !== null && draftTitle.value !== draftBase.value));
  const selectedIds = computed(() => view.value.selected_item_ids?.length ? view.value.selected_item_ids : view.value.selected_item_id ? [view.value.selected_item_id] : []);
  const history = computed(() => board.value?.edit_history);
  const canUndo = computed(() => !!history.value?.undo_stack.length && !queue.value.length);
  const canRedo = computed(() => !!history.value?.redo_stack.length && !queue.value.length);
  function install(document: BoardDocument) { authoritative = clone(document); queue.value = []; editingId.value = null; draftTitle.value = ''; draftConflict.value = false; error.value = ''; notice.value = ''; conflicted = false; deferredDocument = null; }
  function receive(document: BoardDocument) {
    // A read can already contain our committed insert while its HTTP response is
    // still travelling. Wait for that acknowledgement before replaying the queue.
    if (editInFlight) { if (!deferredDocument || deferredDocument.revision < document.revision) deferredDocument = clone(document); return; }
    if (authoritative?.board_id === document.board_id && authoritative.revision > document.revision) return;
    authoritative = clone(document);
    try {
      let projected = document;
      for (const entry of queue.value) {
        if (entry.dependencies.some(dependency => dependencyValue(projected, dependency.key) !== dependency.value)) conflicted = true;
        projected = applyMindMapOperations(projected, entry.operations);
      }
      board.value = projected;
      if (conflicted) error.value = '同一分支已在其他窗口或 AI 更新中改变。你的未保存修改仍显示在这里；请复制需要的文字，再重新载入当前版本。';
      if (editingId.value && !projected.items.some(item => item.item_id === editingId.value)) { draftConflict.value = true; error.value = '正在编辑的主题已被删除。你的草稿保留在下方，可以复制或另存为新主题。'; }
    }
    catch { conflicted = true; error.value = '主题已在其他窗口改变，未保存的操作仍保留。请重新载入后整理。'; }
  }
  async function flush(): Promise<boolean> {
    if (conflicted) return false;
    if (savingTask) return savingTask;
    if (!queue.value.length) return true;
    savingTask = (async () => {
      saving.value = true; error.value = '';
      try {
        if (!await saveOther()) return false;
        while (queue.value.length && board.value) {
          const entry = queue.value[0];
          let saved: BoardDocument;
          editInFlight = true;
          try { saved = await api.boardEdit({ board_id: board.value.board_id, expected_revision: authoritative?.revision ?? board.value.revision, operations: clone(entry.operations), action: entry.action }); }
          finally { editInFlight = false; }
          queue.value.shift(); receive(saved); notice.value = entry.action;
          if (deferredDocument) { const deferred = deferredDocument; deferredDocument = null; receive(deferred); if (conflicted) return false; }
        }
        return true;
      } catch (failure) { error.value = `修改尚未保存：${failure instanceof Error ? failure.message : String(failure)}。本地修改仍保留，可重试保存。`; return false; }
      finally { saving.value = false; if (deferredDocument) { const deferred = deferredDocument; deferredDocument = null; receive(deferred); } }
    })().finally(() => { savingTask = null; }); return savingTask;
  }
  function apply(operations: BoardEditOperation[], action: string) {
    if (!board.value || !operations.length) return false;
    try { const deps = dependencies(board.value, operations), projected = applyMindMapOperations(board.value, operations); queue.value.push({ operations, action, dependencies: deps }); board.value = projected; if (!conflicted) error.value = ''; void flush(); return true; }
    catch (failure) { error.value = failure instanceof Error ? failure.message : String(failure); return false; }
  }
  function startEdit(id: string) {
    if (editingId.value === id) return;
    if (editingId.value && !commitEdit()) return;
    const item = board.value?.items.find(item => item.item_id === id); if (!item || item.target.kind !== 'topic') return;
    editingId.value = id; draftTitle.value = item.title; draftBase.value = item.title; draftConflict.value = false;
  }
  function commitEdit(overwrite = false): boolean {
    if (!editingId.value || !board.value) return true;
    const item = board.value.items.find(item => item.item_id === editingId.value), title = draftTitle.value.trim();
    if (!item) { draftConflict.value = true; error.value = '正在编辑的主题已被删除。你的文字仍保留，可以复制后放到其他主题。'; return false; }
    if (!title) { error.value = '请给主题写一个名称。'; return false; }
    if (!overwrite && item.title !== draftBase.value && title !== item.title) { draftConflict.value = true; error.value = '这个主题在你输入时已更新。你的草稿仍保留，请选择保留自己的文字，或查看当前内容。'; return false; }
    const id = editingId.value; editingId.value = null; draftConflict.value = false;
    if (title !== item.title) return apply([{ kind: 'update_topic', item_id: id, title }], '已修改主题');
    error.value = ''; return true;
  }
  function cancelEdit() { editingId.value = null; draftConflict.value = false; draftTitle.value = ''; error.value = ''; }
  async function undoRedo(redo = false) {
    if (!commitEdit() || !await flush() || !await saveOther() || !board.value) return false;
    saving.value = true; error.value = '';
    try { receive(await (redo ? api.boardRedo : api.boardUndo)(board.value.board_id, authoritative?.revision ?? board.value.revision)); notice.value = redo ? '已重做' : '已撤销'; return true; }
    catch (failure) { error.value = failure instanceof Error ? failure.message : String(failure); return false; }
    finally { saving.value = false; }
  }
  return { queue, saving, error, notice, dirty, editingId, draftTitle, draftConflict, selectedIds, canUndo, canRedo, install, receive, apply, flush, startEdit, commitEdit, cancelEdit, undoRedo };
}

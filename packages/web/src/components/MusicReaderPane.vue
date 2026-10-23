<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { Play, Pause, RotateCcw, Repeat2, Focus, Music2, MessageCircle, Minus, Plus, X, ChevronDown, Volume2, SlidersHorizontal, Maximize2, Minimize2, ArrowUpRight, Check, Scan } from '@lucide/vue';
import type { MusicContext, MusicCommand, MusicPlaybackReceipt, MusicQuestionContext } from '../music-api';
import type { MusicEvent, MusicDemonstration } from '../../../core/src/music-contract';
import { MUSIC_PART_COLORS, MUSIC_TRAIL_MS, MusicTimeline, musicQuestion, musicTime } from '../music-playback';
import { MusicScoreMarkers } from '../music-score';
import { registerMusicPlayer } from '../music-interaction';
import { musicRoleColors, musicRoleGroups, type MusicRoleGroup } from '../music-roles';
import { recordMusicTiming } from '../music-performance';
import { captureScoreQuestionImage, scoreImageRect, type ImageRect } from '../question-image';

const props = defineProps<{ context: MusicContext; owner: string; question?: MusicQuestionContext | null; selectionDisabled?: boolean }>();
const emit = defineEmits<{ (event: 'question', context: MusicQuestionContext): void; (event: 'clear-question'): void; (event: 'image-loading', active: boolean): void }>();
const manifest = computed(() => props.context.manifest);
const pane = ref<HTMLElement>(), focusToggle = ref<HTMLButtonElement>();
const focusMenu = ref(false), fullscreen = ref(false), canFullscreen = ref(false);
const player = ref<HTMLAudioElement>(), demoPlayer = ref<HTMLAudioElement>(), scoreSurface = ref<HTMLElement>();
const position = ref(0), paused = ref(true), buffering = ref(false), error = ref(''), scoreError = ref('');
const rate = ref(1), zoom = ref(1), following = ref(true), focused = ref<string[]>([]), details = ref(false);
const focusedVoices = ref<{ part_id: string; voice: string }[]>([]);
const colorMode = ref<'part' | 'role'>('part'), selectedRole = ref<string | null>(null);
const loop = ref<{ start_ms: number; end_ms: number } | null>(null), stopAt = ref<number | null>(null);
const demo = ref<MusicDemonstration | null>(null), scoreLoading = ref(true);
const displayedCue = ref<{ event_id: string; text: string } | null>(null);
const displayedCueTitle = computed(() => manifest.value.events.find(event => event.event_id === displayedCue.value?.event_id)?.title);
const pageMarkup = ref<Record<string, string>>({});
const pageHosts = new Map<string, HTMLElement>(), markers = new Map<string, MusicScoreMarkers>();
const selecting = ref(false), capturing = ref(false), selectionError = ref('');
const selectionBox = ref<{ left: number; top: number; width: number; height: number } | null>(null);
let selectionContext: MusicQuestionContext | null = null;
let scoreDrag: { id: number; svg: SVGSVGElement; box: DOMRect; start: { x: number; y: number }; rect: ImageRect | null } | null = null;
function cancelSelection() { selecting.value = false; selectionBox.value = null; selectionContext = null; scoreDrag = null; }
function beginSelection() {
  if (props.selectionDisabled || capturing.value) return;
  if (selecting.value) { cancelSelection(); return; }
  selectionError.value = ''; details.value = false; focusMenu.value = false;
  const current = captureQuestion();
  selectionContext = props.question ? JSON.parse(JSON.stringify(props.question)) : current;
  following.value = false; selecting.value = true;
}
function startScoreSelection(event: PointerEvent) {
  if (!selecting.value || props.selectionDisabled || event.button !== 0 || scoreDrag) return;
  const svg = (event.target as Element).closest('.music-score-page')?.querySelector('svg');
  if (!svg) return;
  scoreDrag = { id: event.pointerId, svg, box: svg.getBoundingClientRect(), start: { x: event.clientX, y: event.clientY }, rect: null };
  scoreSurface.value?.setPointerCapture(event.pointerId); event.preventDefault();
}
function moveScoreSelection(event: PointerEvent) {
  if (!scoreDrag || scoreDrag.id !== event.pointerId) return;
  const { box, start } = scoreDrag;
  scoreDrag.rect = scoreImageRect(box, start, { x: event.clientX, y: event.clientY });
  const surface = scoreSurface.value!.getBoundingClientRect(), rect = scoreDrag.rect;
  selectionBox.value = rect ? { left: box.left - surface.left + rect[0] * box.width, top: box.top - surface.top + rect[1] * box.height, width: rect[2] * box.width, height: rect[3] * box.height } : null;
}
async function finishScoreSelection(event: PointerEvent) {
  if (!scoreDrag || scoreDrag.id !== event.pointerId || !selectionContext) return;
  moveScoreSelection(event);
  const { svg, rect } = scoreDrag, question = selectionContext, source = props.context;
  if (!rect) { scoreDrag = null; selectionBox.value = null; selectionError.value = '请拖出一个框，圈住想问的谱线、音符或符号'; return; }
  const image = captureScoreQuestionImage(svg, rect);
  cancelSelection(); capturing.value = true; emit('image-loading', true);
  try {
    const pixels = await image;
    if (disposed || props.context !== source) return;
    if (document.fullscreenElement === pane.value) await document.exitFullscreen();
    emit('question', { ...question, image: pixels });
  } catch (failure) { if (!disposed) selectionError.value = (failure as Error).message; }
  finally { capturing.value = false; if (!disposed) emit('image-loading', false); }
}
const playerId = crypto.randomUUID();
let controlRevision = 0;
let lastReceipt: { id: string; revision: number } | null = null;
const commandRevisions = new Map<string, number>();
function userControl() { controlRevision++; displayedCue.value = null; }
const notes = computed(() => new MusicTimeline(manifest.value.note_alignments));
const occurrences = computed(() => new MusicTimeline(manifest.value.occurrences));
const eventTimeline = computed(() => new MusicTimeline(manifest.value.events));
const objects = computed(() => new Map(manifest.value.score_objects.map(object => [object.source_id, object])));
const activeOccurrence = computed(() => manifest.value.coverage.some(item => item.status === 'accepted' && item.granularity !== 'phrase' && item.start_ms <= position.value && item.end_ms > position.value) ? occurrences.value.at(position.value).at(-1) : undefined);
const noteCoverageReady = computed(() => manifest.value.coverage.some(item => item.granularity === 'note' && item.status === 'accepted'));
const anyCoverageReady = computed(() => manifest.value.coverage.some(item => item.status === 'accepted'));
const currentNoteReady = computed(() => manifest.value.coverage.some(item => item.granularity === 'note' && item.status === 'accepted' && item.start_ms <= position.value && item.end_ms > position.value));
const visibleVoiceParts = computed(() => manifest.value.parts.filter(part => part.voices.length > 1 && (manifest.value.parts.length === 1 || focused.value.includes(part.part_id) || focusedVoices.value.some(voice => voice.part_id === part.part_id))));
const activeEvents = computed(() => eventTimeline.value.at(position.value));
const activeEvent = computed(() => activeEvents.value.at(-1));
const roleGroups = computed(() => musicRoleGroups(activeEvents.value, manifest.value.events));
const hasSelectedRole = computed(() => roleGroups.value.some(group => group.key === selectedRole.value));
const roleColors = computed(() => musicRoleColors(activeEvents.value, manifest.value.events, objects.value));
const coverage = computed(() => manifest.value.coverage.find(item => item.start_ms <= position.value && item.end_ms > position.value && item.status === 'accepted'));
const detailedSections = computed(() => manifest.value.structure.filter(section => section.start_ms !== null));
const demoUrl = computed(() => props.context.demonstrations.find(asset => asset.demonstration_id === demo.value?.demonstration_id)?.url);
const positionKey = computed(() => `music-position:${props.owner}:${manifest.value.recording.source.recording_id}:${manifest.value.recording.source.revision}`);
const focusLabel = computed(() => {
  if (selectedRole.value) return roleGroups.value.find(group => group.key === selectedRole.value)?.label ?? '音乐作用';
  const count = focused.value.length + focusedVoices.value.length;
  if (!count) return '全部声部';
  if (count === 1 && focusedVoices.value[0]) return voiceLabel(focusedVoices.value[0].part_id, focusedVoices.value[0].voice);
  if (count === 1) return manifest.value.parts.find(part => part.part_id === focused.value[0])?.name ?? '关注声部';
  return `关注 ${count} 个声部`;
});
const materialHint = computed(() => manifest.value.recording.kind === 'score_synthesis' ? '谱面合成 · 音频由乐谱生成，并非原录音' : noteCoverageReady.value ? '原录音 · 已校准逐音符跟谱' : anyCoverageReady.value ? '原录音 · 已有片段定位，逐音符跟谱待校准' : '原录音 · 录音与谱面的对齐待校准，可以自由回听、查看谱面');
function resetFocus() { userControl(); selectedRole.value = null; focused.value = []; focusedVoices.value = []; paint(); }
function dismissMenus(event: PointerEvent) { const target = event.target as Element; if (!target.closest('.music-focus-control')) focusMenu.value = false; if (!target.closest('.music-guide, .music-details-toggle')) details.value = false; }
function closeMenus(event: KeyboardEvent) { if (event.key !== 'Escape') return; cancelSelection(); if (focusMenu.value) { focusMenu.value = false; focusToggle.value?.focus(); } details.value = false; }
async function toggleFullscreen() { try { if (document.fullscreenElement === pane.value) await document.exitFullscreen(); else await pane.value?.requestFullscreen(); } catch { error.value = '暂时无法进入全屏，请使用窗口内的乐谱视图。'; } }
function syncFullscreen() { fullscreen.value = document.fullscreenElement === pane.value; }
function partColor(id: string, voice?: string) { const index = manifest.value.parts.length === 1 && voice ? manifest.value.parts[0].voices.indexOf(voice) : manifest.value.parts.findIndex(part => part.part_id === id); return MUSIC_PART_COLORS[Math.max(0, index) % MUSIC_PART_COLORS.length]; }
function roleFor(id: string) { return [...new Set(activeEvents.value.flatMap(event => event.roles.filter(role => role.part_id === id).map(role => role.role)))].join(' · '); }
function togglePart(id: string) { userControl(); selectedRole.value = null; focusedVoices.value = []; focused.value = focused.value.includes(id) ? focused.value.filter(part => part !== id) : [...focused.value, id]; paint(); }
function voiceSelected(partId: string, voice: string) { return focusedVoices.value.some(item => item.part_id === partId && item.voice === voice); }
function toggleVoice(partId: string, voice: string) { userControl(); selectedRole.value = null; focused.value = []; focusedVoices.value = voiceSelected(partId, voice) ? focusedVoices.value.filter(item => !(item.part_id === partId && item.voice === voice)) : [...focusedVoices.value, { part_id: partId, voice }]; paint(); }
function roleParticipants(group: MusicRoleGroup) { return [...new Set(group.roles.map(role => manifest.value.parts.find(part => part.part_id === role.part_id)?.name ?? role.part_id))].join('、'); }
function focusRole(group?: MusicRoleGroup) { focused.value = [...new Set(group?.roles.filter(role => !role.voice_ids?.length).map(role => role.part_id) ?? [])]; focusedVoices.value = group?.roles.flatMap(role => (role.voice_ids ?? []).map(voice => ({ part_id: role.part_id, voice }))) ?? []; }
function followRole() { const id = [...roleColors.value.keys()].find(id => { const object = objects.value.get(id); return object && !object.rest && (focused.value.includes(object.part_id) || voiceSelected(object.part_id, object.voice)); }); if (id) followObject(id, true); }
function selectRole(group: MusicRoleGroup) { userControl(); selectedRole.value = selectedRole.value === group.key ? null : group.key; focusRole(selectedRole.value ? group : undefined); applyFocus(); paint(); followRole(); }
function changeColorMode() { userControl(); selectedRole.value = null; focused.value = []; focusedVoices.value = []; applyRoleColors(); applyFocus(); paint(); }
function applyRoleColors() { for (const marker of markers.values()) marker.colors(colorMode.value === 'role' ? roleColors.value : null); }
function voiceLabel(partId: string, voice: string) { const part = manifest.value.parts.find(item => item.part_id === partId); const staff = voice.split(':')[0]; const siblings = part?.voices.filter(item => item.split(':')[0] === staff) ?? []; const name = staff === '1' ? '上谱表' : staff === '2' ? '下谱表' : `谱表 ${staff}`; return `${name}${siblings.length > 1 ? ` · 声部 ${siblings.indexOf(voice) + 1}` : ''}`; }
function voiceRole(partId: string, voice: string) { return [...new Set(activeEvents.value.flatMap(event => event.roles.filter(role => role.part_id === partId && role.voice_ids?.includes(voice)).map(role => role.role)))].join(' · '); }
function setPageHost(id: string, element: unknown) { if (element instanceof HTMLElement) pageHosts.set(id, element); else pageHosts.delete(id); }
let frame = 0, disposed = false, releasePlayer: (() => void) | undefined;
let lastFrameTime = 0;
let surfaceResize: ResizeObserver | undefined;
let lastFollow = '', lastSaved = -1;
let didInitialFocus = false;
function initialOrchestraFocus() {
  if (didInitialFocus || controlRevision || manifest.value.parts.length < 2 || !markers.size) return;
  const theme = roleGroups.value.find(group => group.key.startsWith('theme:')); if (!theme) return;
  didInitialFocus = true; colorMode.value = 'role'; selectedRole.value = theme.key; focusRole(theme); applyRoleColors(); applyFocus();
  const id = [...roleColors.value.keys()].find(id => { const object = objects.value.get(id); return object && !object.rest && focused.value.includes(object.part_id); });
  if (id) followObject(id, true);
}
function remember() { if (!player.value?.readyState) return; try { localStorage.setItem(positionKey.value, JSON.stringify({ ms: position.value, rate: rate.value })); } catch { /* Listening remains available without local storage. */ } }
function loaded() {
  const audio = player.value; if (!audio) return;
  let restored = false;
  try { const saved = JSON.parse(localStorage.getItem(positionKey.value) || 'null'); if (saved && Number.isFinite(saved.ms)) { audio.currentTime = Math.max(0, Math.min(saved.ms / 1000, audio.duration - .01)); restored = true; } if (saved && Number.isFinite(saved.rate) && saved.rate >= .5 && saved.rate <= 2) rate.value = saved.rate; } catch { /* Start at the selected passage. */ }
  const opening = manifest.value.structure.find(section => section.coverage === 'detailed' && section.start_ms !== null)?.start_ms;
  if (!restored && opening != null) audio.currentTime = opening / 1000;
  audio.playbackRate = rate.value; sync(); initialOrchestraFocus();
}
function sync() {
  const audio = player.value; if (!audio) return;
  position.value = audio.currentTime * 1000; paused.value = audio.paused;
  if (!audio.paused && loop.value && position.value >= loop.value.end_ms) { audio.currentTime = loop.value.start_ms / 1000; position.value = loop.value.start_ms; }
  else if (!audio.paused && stopAt.value !== null && position.value >= stopAt.value) { audio.pause(); audio.currentTime = stopAt.value / 1000; position.value = stopAt.value; stopAt.value = null; }
  const second = Math.floor(position.value / 1000); if (second !== lastSaved && second % 5 === 0) { remember(); lastSaved = second; }
  paint();
}
function tick() { if (import.meta.env.DEV) { const now = performance.now(); if (lastFrameTime) recordMusicTiming('playing_frame_interval', now - lastFrameTime); lastFrameTime = now; } sync(); if (!disposed && !player.value?.paused) frame = requestAnimationFrame(tick); }
function onPlay() { cancelSelection(); demoPlayer.value?.pause(); paused.value = false; buffering.value = false; cancelAnimationFrame(frame); lastFrameTime = 0; tick(); }
function onPause() { paused.value = true; cancelAnimationFrame(frame); sync(); remember(); }
function paint() {
  const started = import.meta.env.DEV ? performance.now() : 0;
  const ms = position.value;
  // Gaps in note coverage must not be presented as exact following.
  const noteReady = currentNoteReady.value;
  const playing = !paused.value && !buffering.value;
  const active = noteReady ? notes.value.at(ms, playing ? MUSIC_TRAIL_MS : 0).filter(note => { const object = objects.value.get(note.score_object_id); return object && ((!focused.value.length && !focusedVoices.value.length) || focused.value.includes(object.part_id) || voiceSelected(object.part_id, object.voice)); }) : [];
  for (const marker of markers.values()) marker.render(active, ms, playing);
  const targetId = active.find(note => note.start_ms <= ms && note.end_ms > ms)?.score_object_id ?? activeOccurrence.value?.score_object_ids[0];
  const object = targetId ? objects.value.get(targetId) : undefined;
  const followKey = object ? `${object.page_id}:${object.measure}` : '';
  if (following.value && followKey && followKey !== lastFollow) { lastFollow = followKey; followObject(targetId!); }
  if (import.meta.env.DEV) recordMusicTiming('paint_js', performance.now() - started);
}
function followObject(id: string, force = false) {
  const object = objects.value.get(id), surface = scoreSurface.value;
  const element = object && markers.get(object.page_id)?.element(id);
  if (!element || !surface) return;
  const box = element.getBoundingClientRect(), view = surface.getBoundingClientRect();
  const vertical = force || box.top < view.top + 54 || box.bottom > view.bottom - 64;
  const horizontal = box.left < view.left + 22 || box.right > view.right - 22;
  if (vertical || horizontal) surface.scrollTo({ top: vertical ? Math.max(0, surface.scrollTop + box.top - view.top - surface.clientHeight * .36) : surface.scrollTop,
    left: horizontal ? Math.max(0, surface.scrollLeft + box.left - view.left - surface.clientWidth * .4) : surface.scrollLeft, behavior: force || paused.value ? 'instant' : 'smooth' });
}
function resumeFollow() { following.value = true; lastFollow = ''; const id = activeOccurrence.value?.score_object_ids[0]; if (id) followObject(id, true); else if (selectedRole.value) followRole(); paint(); }
function browseScore() { following.value = false; }
function keyBrowse(event: KeyboardEvent) { if (['ArrowUp', 'ArrowDown', 'PageUp', 'PageDown', 'Home', 'End', ' '].includes(event.key)) browseScore(); }
async function play() {
  const audio = player.value; if (!audio) throw new Error('播放器尚未就绪');
  if (audio.error) throw new Error('录音暂不可读取，请重新打开资料');
  exitDemo(); await audio.play(); error.value = '';
}
async function displayCue(eventId?: string | null) {
  const event = eventId == null ? null : manifest.value.events.find(item => item.event_id === eventId);
  if (eventId != null && (!event || !event.listen_for.trim())) throw new Error('这份作品中没有可用的伴听提示');
  displayedCue.value = event ? { event_id: event.event_id, text: event.listen_for } : null;
  if (!event) return;
  const revision = controlRevision, context = props.context;
  await nextTick();
  // The first callback precedes paint; the second allows the cue to be drawn before sound starts.
  await new Promise<void>(resolve => requestAnimationFrame(() => requestAnimationFrame(() => resolve())));
  if (disposed || revision !== controlRevision || context !== props.context) throw new Error('播放方式已改变，请重新观察当前音乐后再操作');
}
async function togglePlay() { userControl(); try { if (player.value?.paused) await play(); else player.value?.pause(); } catch (failure) { error.value = `播放未开始：${(failure as Error).message}。请点播放重试。`; } }
function seek(ms: number) {
  const audio = player.value;
  if (!audio || !audio.readyState) throw new Error('请等待录音加载');
  if (!Number.isFinite(ms) || ms < 0 || ms > manifest.value.recording.duration_ms) throw new Error('播放位置超出录音范围');
  exitDemo();
  audio.currentTime = ms / 1000; stopAt.value = null; position.value = ms; lastFollow = ''; paint(); remember();
}
async function confirmedSeek(ms: number) {
  seek(ms);
  const audio = player.value!;
  if (!audio.seeking) return;
  await new Promise<void>((resolve, reject) => {
    const done = () => { clearTimeout(timer); audio.removeEventListener('seeked', done); audio.removeEventListener('error', failed); resolve(); };
    const failed = () => { clearTimeout(timer); audio.removeEventListener('seeked', done); audio.removeEventListener('error', failed); reject(new Error('未能确认录音跳转，请重试')); };
    const timer = setTimeout(failed, 5000);
    audio.addEventListener('seeked', done, { once: true }); audio.addEventListener('error', failed, { once: true });
  });
  sync();
}
function seekInput(event: Event) { userControl(); try { seek(Number((event.target as HTMLInputElement).value)); } catch (failure) { error.value = (failure as Error).message; } }
function changeRate(user = false) { if (user) userControl(); if (player.value) player.value.playbackRate = rate.value; remember(); }
function seekSection(start: number | null) { details.value = false; userControl(); if (start === null) return; player.value?.pause(); loop.value = null; try { seek(start); resumeFollow(); } catch (failure) { error.value = (failure as Error).message; } }
function setLoop(start: number, end: number) { if (!Number.isFinite(start) || !Number.isFinite(end) || start < 0 || end <= start || end > manifest.value.recording.duration_ms) throw new Error('请选择录音内有效的循环片段'); exitDemo(); loop.value = { start_ms: start, end_ms: end }; stopAt.value = null; if (position.value < start || position.value >= end) seek(start); }
function toggleLoop() { userControl(); if (loop.value) { loop.value = null; return; } const span = activeEvent.value ?? activeOccurrence.value; if (span) setLoop(span.start_ms, span.end_ms); }
async function listenEvent(event: MusicEvent) { userControl(); try { loop.value = null; seek(event.start_ms); stopAt.value = event.end_ms; resumeFollow(); await displayCue(event.event_id); await play(); } catch (failure) { error.value = (failure as Error).message; } }
function captureQuestion(draftId = crypto.randomUUID()) { userControl(); player.value?.pause(); demoPlayer.value?.pause(); sync(); return musicQuestion(manifest.value, props.context.publication, position.value, focused.value, draftId, focusedVoices.value); }
async function ask() {
  try {
    const question = captureQuestion();
    if (document.fullscreenElement === pane.value) await document.exitFullscreen();
    emit('question', question);
  } catch (failure) { error.value = (failure as Error).message; }
}
async function showDemo(item: MusicDemonstration, autoplay = false, cueEventId?: string | null) { details.value = false; if (!autoplay) userControl(); player.value?.pause(); demoPlayer.value?.pause(); demo.value = item; await nextTick(); if (autoplay) { if (!demoPlayer.value || !demoUrl.value) throw new Error('这份示范音频暂不可读取'); await displayCue(cueEventId); await demoPlayer.value.play(); } }
function exitDemo() { demoPlayer.value?.pause(); demo.value = null; }
function closeDemo() { userControl(); exitDemo(); }
async function execute(command: MusicCommand): Promise<MusicPlaybackReceipt> {
  let failure: string | null = null;
  commandRevisions.set(command.command_id, controlRevision);
  try {
    if (command.request.operation === 'act' && (!lastReceipt || command.request.based_on !== lastReceipt.id || lastReceipt.revision !== controlRevision)) throw new Error('你已调整播放位置或播放方式，请重新观察当前音乐后再操作');
    if (command.request.publication.book_id !== props.context.publication.book_id || command.request.publication.publication_id !== props.context.publication.publication_id || command.request.recording.recording_id !== manifest.value.recording.source.recording_id || command.request.recording.revision !== manifest.value.recording.source.revision) throw new Error('播放器中的录音版本已改变');
    if (!player.value?.readyState) throw new Error('录音尚未加载完成');
    const action = command.request.action;
    if (command.request.operation === 'act' && action) switch (action.kind) {
      case 'play': {
        if (action.start_ms != null || action.end_ms != null) {
          if (action.start_ms != null) await confirmedSeek(action.start_ms);
          loop.value = null; stopAt.value = action.end_ms ?? null;
        }
        exitDemo(); await displayCue(action.cue_event_id); await play(); break;
      }
      case 'pause': player.value.pause(); demoPlayer.value?.pause(); break;
      case 'seek': displayedCue.value = null; await confirmedSeek(action.time_ms); break;
      case 'loop': displayedCue.value = null; setLoop(action.start_ms, action.end_ms); if (player.value.seeking) await confirmedSeek(position.value); break;
      case 'clear_loop': loop.value = null; break;
      case 'focus': if (action.part_ids.some(id => !manifest.value.parts.some(part => part.part_id === id)) || action.voices?.some(item => !manifest.value.parts.find(part => part.part_id === item.part_id)?.voices.includes(item.voice))) throw new Error('所选声部不在这份乐谱中'); selectedRole.value = null; focused.value = [...action.part_ids]; focusedVoices.value = (action.voices ?? []).map(item => ({ ...item })); paint(); break;
      case 'rate': if (!Number.isFinite(action.rate) || action.rate < .5 || action.rate > 2) throw new Error('播放速度应在 0.5 到 2 倍之间'); rate.value = action.rate; changeRate(); break;
      case 'demonstration': { const item = manifest.value.demonstrations.find(item => item.demonstration_id === action.demonstration_id); if (!item) throw new Error('示范素材不存在'); await showDemo(item, true, action.cue_event_id); break; }
    }
    sync();
  } catch (reason) { failure = (reason as Error).message; error.value = failure; }
  return { receipt_id: crypto.randomUUID(), command_id: command.command_id, publication: { ...props.context.publication }, recording: { ...manifest.value.recording.source }, status: failure ? 'failed' : command.request.operation === 'observe' ? 'observed' : 'applied', current_ms: (player.value?.currentTime ?? 0) * 1000, paused: player.value?.paused ?? true, playback_rate: player.value?.playbackRate ?? 1, loop: loop.value ? { ...loop.value } : null, focused_part_ids: [...focused.value], focused_voices: focusedVoices.value.map(item => ({ ...item })), occurrence_id: activeOccurrence.value?.occurrence_id ?? null, demonstration_id: demo.value?.demonstration_id ?? null, ...(demo.value && demoPlayer.value ? { demonstration_state: { current_ms: demoPlayer.value.currentTime * 1000, paused: demoPlayer.value.paused } } : {}), ...(displayedCue.value ? { displayed_cue: { ...displayedCue.value } } : {}), error: failure };
}
function applyFocus() { for (const marker of markers.values()) marker.focus((part, voice) => (!focused.value.length && !focusedVoices.value.length) || focused.value.includes(part) || voiceSelected(part, voice)); }
function rebuildMarkers() { const started = import.meta.env.DEV ? performance.now() : 0; for (const marker of markers.values()) marker.dispose(); markers.clear(); for (const page of manifest.value.pages) { const svg = pageHosts.get(page.page_id)?.querySelector<SVGSVGElement>('svg'); if (svg) markers.set(page.page_id, new MusicScoreMarkers(svg, manifest.value.score_objects.filter(object => object.page_id === page.page_id), partColor)); } for (const marker of markers.values()) marker.attach(); applyRoleColors(); applyFocus(); lastFollow = ''; paint(); if (import.meta.env.DEV) recordMusicTiming('marker_geometry', performance.now() - started); }
let loadGeneration = 0;
async function loadScores() {
  const started = import.meta.env.DEV ? performance.now() : 0;
  const generation = ++loadGeneration; scoreLoading.value = true; scoreError.value = '';
  try {
    const pages = await Promise.all(props.context.score_pages.map(async page => { const response = await fetch(page.url, { credentials: 'same-origin' }); if (!response.ok) throw new Error('乐谱暂不可读取，请重试'); const svg = await response.text(); const parsed = new DOMParser().parseFromString(svg, 'image/svg+xml'); if (parsed.querySelector('parsererror') || parsed.documentElement.localName !== 'svg') throw new Error('乐谱内容无法显示'); return [page.page_id, svg] as const; }));
    if (disposed || generation !== loadGeneration) return;
    pageMarkup.value = Object.fromEntries(pages); await nextTick(); rebuildMarkers(); initialOrchestraFocus(); if (import.meta.env.DEV) recordMusicTiming('scores_load_and_geometry', performance.now() - started);
  } catch (failure) { if (generation === loadGeneration) scoreError.value = (failure as Error).message; }
  finally { if (generation === loadGeneration) scoreLoading.value = false; }
}
watch(zoom, async () => { cancelSelection(); await nextTick(); rebuildMarkers(); });
watch(() => props.selectionDisabled, disabled => { if (disabled) cancelSelection(); });
watch(() => props.question?.draft_id, cancelSelection);
watch([focused, focusedVoices], applyFocus, { deep: true });
watch(() => activeEvents.value.map(event => event.event_id).join('|'), () => { if (selectedRole.value) focusRole(roleGroups.value.find(group => group.key === selectedRole.value)); applyRoleColors(); applyFocus(); paint(); if (selectedRole.value && following.value) followRole(); });
watch(() => props.context, () => { cancelSelection(); userControl(); exitDemo(); focusMenu.value = false; details.value = false; pageMarkup.value = {}; focused.value = []; focusedVoices.value = []; loop.value = null; void loadScores(); });
onMounted(() => { canFullscreen.value = !!document.fullscreenEnabled; document.addEventListener('fullscreenchange', syncFullscreen); });
onBeforeUnmount(() => { document.removeEventListener('fullscreenchange', syncFullscreen); });
onMounted(() => { void loadScores(); surfaceResize = new ResizeObserver(() => { lastFollow = ''; paint(); if (selectedRole.value && following.value) followRole(); }); if (scoreSurface.value) surfaceResize.observe(scoreSurface.value); releasePlayer = registerMusicPlayer({ id: playerId, execute, accepted: receipt => { if (receipt.status !== 'failed') lastReceipt = { id: receipt.receipt_id, revision: commandRevisions.get(receipt.command_id) ?? -1 }; commandRevisions.delete(receipt.command_id); } }); });
onBeforeUnmount(() => { remember(); disposed = true; cancelAnimationFrame(frame); surfaceResize?.disconnect(); releasePlayer?.(); player.value?.pause(); demoPlayer.value?.pause(); for (const marker of markers.values()) marker.dispose(); });
function openSource(target: import('../music-api').MusicSourceTarget) { userControl(); player.value?.pause(); loop.value = null; seek(target.audio.start_ms); resumeFollow(); const id = target.score?.source_ids[0]; if (id) followObject(id, true); }
defineExpose({ captureQuestion, seek, execute, openSource, playerId });
</script>

<template>
  <section ref="pane" class="music-reader reader-pane" aria-label="音乐伴听" @pointerdown="dismissMenus" @keydown="closeMenus">
    <audio ref="player" :src="context.audio_url" preload="metadata" @loadedmetadata="loaded" @play="onPlay" @pause="onPause" @timeupdate="sync" @seeked="sync" @ended="onPause" @waiting="buffering = true" @playing="buffering = false" @error="error = '录音暂不可读取，请重新打开资料'" />
    <header class="music-heading">
      <div class="music-heading-mark" aria-hidden="true"><Music2 :size="22" :stroke-width="1.4" /></div>
      <div class="music-heading-copy">
        <h1 :title="manifest.work.title">{{ manifest.work.title }}</h1>
        <p class="music-subtitle" :title="`${manifest.work.composer} · ${manifest.work.movement ?? ''}`">{{ manifest.work.composer }}<span v-if="manifest.work.movement"> · {{ manifest.work.movement }}</span></p>
      </div>
      <button class="music-details-toggle" :aria-expanded="details" aria-controls="music-work-guide" @click="details = !details; focusMenu = false"><span>作品导览</span><ChevronDown :size="15" :class="{ rotated: details }" /></button>
    </header>
    <aside v-if="details" id="music-work-guide" class="music-guide" aria-label="作品导览">
      <div class="music-panel-heading"><span>走进这首作品</span><button aria-label="关闭作品导览" @click="details = false"><X :size="17" /></button></div>
      <h2 class="music-guide-title">{{ manifest.work.title }}</h2><p class="music-guide-subtitle">{{ manifest.work.composer }}<span v-if="manifest.work.movement"> · {{ manifest.work.movement }}</span></p>
      <p class="music-recording"><Volume2 :size="15" /> {{ manifest.recording.title }} <span>{{ manifest.recording.performers }}</span></p>
      <nav aria-label="作品段落" class="music-sections"><button v-for="section in detailedSections" :key="section.section_id" @click="seekSection(section.start_ms)"><span>{{ section.title }}</span><small>{{ musicTime(section.start_ms ?? 0) }}{{ section.coverage === 'navigation_only' ? ' · 整体导览' : '' }}</small></button></nav>
      <nav v-if="manifest.events.length" aria-label="音乐事件" class="music-event-links"><button v-for="event in manifest.events" :key="event.event_id" @click="seekSection(event.start_ms)"><span>{{ event.title }}</span><small>{{ musicTime(event.start_ms) }}–{{ musicTime(event.end_ms) }} <ArrowUpRight :size="13" /></small></button></nav>
      <div v-if="manifest.demonstrations.length" class="music-demonstrations"><span>拆开听一听 <small>谱面合成示范</small></span><button v-for="item in manifest.demonstrations" :key="item.demonstration_id" @click="showDemo(item)"><Play :size="12" />{{ item.title }}</button></div>
      <details class="music-credits"><summary>录音与谱本来源</summary><p v-for="(right, index) in manifest.rights" :key="index"><a :href="right.url" target="_blank" rel="noopener noreferrer">{{ right.title }}</a> · {{ right.license }}<br>{{ right.attribution }}</p></details>
    </aside>

    <div class="music-score-toolbar">
      <div class="music-focus-control">
        <button ref="focusToggle" class="music-focus-toggle" :class="{ selected: focused.length || focusedVoices.length || selectedRole }" :aria-expanded="focusMenu" aria-controls="music-focus-menu" @click="focusMenu = !focusMenu; details = false"><SlidersHorizontal :size="15" /><span>{{ focusLabel }}</span><ChevronDown :size="12" /></button>
        <div v-if="focusMenu" id="music-focus-menu" class="music-focus-menu" role="region" aria-label="选择关注的声部">
          <div class="music-panel-heading"><span>选择关注的声音</span><button aria-label="关闭声部选择" @click="focusMenu = false; focusToggle?.focus()"><X :size="16" /></button></div>
          <p class="music-focus-hint">突出谱上的线条，原录音仍完整播放。</p>
          <label class="music-color-mode">着色方式 <select v-model="colorMode" aria-label="谱面着色方式" @change="changeColorMode"><option value="part">乐器与声部</option><option value="role">音乐作用</option></select></label>
          <div v-if="colorMode === 'part'" class="music-part-options" aria-label="谱面声部聚焦">
            <button :aria-pressed="!focused.length && !focusedVoices.length" @click="resetFocus"><span>全部声部</span><Check v-if="!focused.length && !focusedVoices.length" :size="14" /></button>
            <button v-for="part in manifest.parts" :key="part.part_id" :aria-pressed="focused.includes(part.part_id)" :style="{ '--part-color': partColor(part.part_id) }" @click="togglePart(part.part_id)"><i></i><span>{{ part.name }}<small v-if="roleFor(part.part_id)">{{ roleFor(part.part_id) }}</small></span><Check v-if="focused.includes(part.part_id)" :size="14" /></button>
          </div>
          <div v-else class="music-part-options music-role-options" aria-label="音乐作用聚焦"><button v-for="group in roleGroups" :key="group.key" :aria-pressed="selectedRole === group.key" :style="{ '--part-color': group.color }" @click="selectRole(group)"><i></i><span>{{ group.label }}<small>{{ roleParticipants(group) }}</small></span><Check v-if="selectedRole === group.key" :size="14" /></button><p v-if="!roleGroups.length" class="music-role-empty">这段还没有音乐作用标注</p></div>
          <div v-if="colorMode === 'part' && visibleVoiceParts.length" class="music-voice-options" aria-label="谱面内的声部"><p>跟着一条线听</p><template v-for="part in visibleVoiceParts" :key="part.part_id"><button v-for="voice in part.voices" :key="voice" :aria-pressed="voiceSelected(part.part_id, voice)" :style="{ '--part-color': partColor(part.part_id, voice) }" @click="toggleVoice(part.part_id, voice)"><i></i><span>{{ voiceLabel(part.part_id, voice) }}<small v-if="voiceRole(part.part_id, voice)">{{ voiceRole(part.part_id, voice) }}</small></span><Check v-if="voiceSelected(part.part_id, voice)" :size="14" /></button></template></div>
        </div>
      </div>
      <span class="music-material-tag" :class="{ pending: !noteCoverageReady, synthesis: manifest.recording.kind === 'score_synthesis' }" :title="materialHint"><i></i>{{ manifest.recording.kind === 'score_synthesis' ? '谱面合成' : '原录音' }}<span v-if="!noteCoverageReady" class="music-precision-label"> · 跟谱待校准</span></span>
      <div class="music-toolbar-spacer"></div>
      <button class="music-screenshot" :class="{ active: selecting }" :aria-pressed="selecting" :disabled="selectionDisabled || capturing || scoreLoading || !!scoreError" :aria-label="selecting ? '取消框选谱面' : '框选谱面提问'" title="框住看不懂的谱线、音符或符号" @click="beginSelection"><Scan :size="15" /><span>{{ capturing ? '截图中…' : selecting ? '取消框选' : '框选谱面' }}</span></button>
      <button class="music-follow-toggle" :disabled="!activeOccurrence && !hasSelectedRole" :class="{ 'is-detached': !following }" :aria-label="!activeOccurrence && hasSelectedRole ? '回到主题谱面' : '回到播放位置'" :title="!activeOccurrence && hasSelectedRole ? '回到主题谱面' : '回到播放位置'" @click="resumeFollow"><Focus :size="14" /><span>{{ !following ? '回到播放位置' : activeOccurrence ? `第 ${activeOccurrence.measure} 小节` : hasSelectedRole ? '主题谱面' : '跟随' }}</span></button>
      <div class="music-zoom"><button :disabled="zoom <= .75" aria-label="缩小乐谱" @click="zoom = Math.max(.75, zoom - .25)"><Minus :size="14" /></button><span>{{ Math.round(zoom * 100) }}%</span><button :disabled="zoom >= 2" aria-label="放大乐谱" @click="zoom = Math.min(2, zoom + .25)"><Plus :size="14" /></button></div>
      <button v-if="canFullscreen" class="music-fullscreen" :aria-label="fullscreen ? '退出全屏乐谱' : '全屏乐谱'" :title="fullscreen ? '退出全屏乐谱' : '全屏乐谱'" @click="toggleFullscreen"><Minimize2 v-if="fullscreen" :size="15" /><Maximize2 v-else :size="15" /></button>
    </div>

    <div class="music-score-stage">
      <div ref="scoreSurface" class="music-score-surface" :class="{ 'is-selecting': selecting }" tabindex="0" aria-label="乐谱，可滚动查看" @wheel.passive="browseScore" @touchmove.passive="browseScore" @pointerdown="browseScore(); startScoreSelection($event)" @pointermove="moveScoreSelection" @pointerup="finishScoreSelection" @pointercancel="cancelSelection" @lostpointercapture="scoreDrag = null" @keydown="keyBrowse" @scroll="selectionBox = null; scoreDrag = null">
        <div v-if="scoreLoading" class="music-score-state" role="status">正在展开乐谱…</div>
        <div v-if="scoreError" class="music-score-state" role="alert">{{ scoreError }} <button @click="loadScores">重试</button></div>
        <div class="music-score-sheets" :style="{ width: `${zoom * 100}%` }"><div v-for="page in manifest.pages" :key="page.page_id" :ref="element => setPageHost(page.page_id, element)" class="music-score-page" :data-page="page.page_id" :aria-label="`乐谱 ${page.page_id}`" v-html="pageMarkup[page.page_id] || ''"></div></div>
      </div>
      <div v-if="selectionBox" class="music-score-selection" :style="{ left: `${selectionBox.left}px`, top: `${selectionBox.top}px`, width: `${selectionBox.width}px`, height: `${selectionBox.height}px` }"></div>
      <div v-if="selecting || capturing || selectionError" class="music-selection-hint" :role="selectionError ? 'alert' : 'status'"><span>{{ selectionError || (capturing ? '正在生成谱面截图…' : '拖动框住想问的地方，不用知道它叫什么') }}</span><button v-if="!capturing" :aria-label="selecting ? '取消框选' : '关闭截图提示'" @click="cancelSelection(); selectionError = ''"><X :size="14" /></button></div>
      <div v-if="demo" class="music-demo-player"><div class="music-demo-heading"><span class="music-demo-eyebrow">拆开听 · 谱面合成</span><strong>{{ demo.title }}</strong><span>用来辨认声部，并非原录音分轨</span></div><button aria-label="返回原录音" title="返回原录音" @click="closeDemo"><X :size="17" /></button><div v-if="displayedCue" class="music-play-cue" :data-event-id="displayedCue.event_id" role="status"><strong>这一遍留意</strong><p>{{ displayedCue.text }}</p><small>作品导览 · {{ displayedCueTitle }}</small></div><audio ref="demoPlayer" :src="demoUrl" controls preload="metadata" @pointerdown="userControl" @keydown="userControl" @play="player?.pause()" /></div>
    </div>

    <div v-if="!demo && displayedCue" class="music-play-cue music-cue-strip" :data-event-id="displayedCue.event_id" role="status"><span class="music-cue-kicker">这一遍留意</span><div><p>{{ displayedCue.text }}</p><small>作品导览 · {{ displayedCueTitle }}</small></div></div>
    <div v-else-if="!demo && activeEvent" class="music-listening-cue music-cue-strip"><span class="music-cue-kicker">听见 · {{ activeEvent.title }}</span><p>{{ activeEvent.listen_for }}</p><button title="重听这一段" aria-label="重听这一段" @click="listenEvent(activeEvent)"><RotateCcw :size="16" /></button></div>
    <div v-if="question" class="music-question-anchor"><MessageCircle :size="14" /><span>{{ question.image ? '已附谱面图片，可以直接问「这是什么」' : `正在问 ${musicTime(question.audio.start_ms)}–${musicTime(question.audio.end_ms)} 这一段` }}</span><button :disabled="selectionDisabled" @click="emit('clear-question')" aria-label="取消片段提问"><X :size="14" /></button></div>
    <footer class="music-player-bar" :class="{ 'is-playing': !paused }">
      <div class="music-player-controls">
        <button class="music-play" :aria-label="paused ? '播放音乐' : '暂停音乐'" @click="togglePlay"><Play v-if="paused" :size="19" fill="currentColor" /><Pause v-else :size="19" fill="currentColor" /></button>
        <div class="music-playing-label"><strong><i></i>{{ buffering && !paused ? '正在缓冲' : paused ? '准备聆听' : '正在聆听' }}</strong><small>{{ manifest.recording.kind === 'score_synthesis' ? '谱面合成示范' : focused.length || focusedVoices.length ? '聚焦谱面 · 原声完整播放' : '原录音' }}</small></div>
        <div class="music-progress"><span>{{ musicTime(position) }}</span><input type="range" min="0" :max="manifest.recording.duration_ms" :value="position" :style="{ '--progress': `${position / manifest.recording.duration_ms * 100}%` }" step="10" aria-label="音乐播放位置" @input="seekInput"><span>{{ musicTime(manifest.recording.duration_ms) }}</span></div>
        <label class="music-rate"><span class="sr-only">播放速度</span><select v-model.number="rate" aria-label="播放速度" @change="changeRate(true)"><option :value=".5">0.5×</option><option :value=".75">0.75×</option><option :value="1">1×</option><option :value="1.25">1.25×</option><option :value="1.5">1.5×</option><option :value="2">2×</option></select></label>
        <button class="music-loop" :class="{ active: loop }" :disabled="!loop && !activeEvent && !activeOccurrence" :aria-pressed="!!loop" :title="loop ? `循环 ${musicTime(loop.start_ms)}–${musicTime(loop.end_ms)}` : '循环当前片段'" aria-label="循环当前片段" @click="toggleLoop"><Repeat2 :size="18" /><span>{{ loop ? '循环中' : '循环' }}</span></button>
        <button class="music-ask" :disabled="selectionDisabled" :title="demo ? `暂停示范，询问原录音 ${musicTime(position)} 附近的片段` : undefined" @click="ask"><MessageCircle :size="16" /><span>{{ demo ? '问原声这段' : '聊聊这段' }}</span></button>
      </div>
      <p v-if="error" class="music-error" role="alert">{{ error }}</p>
    </footer>
  </section>
</template>

<style scoped>
.music-reader{--music-ink:#263d36;--music-muted:#788178;--music-paper:#fffefa;--music-copper:#aa744c;--music-line:#dfdfd4;container:music / inline-size;min-width:0;min-height:0;height:100%;display:flex;flex-direction:column;overflow:hidden;background:#f5f3ec;color:var(--music-ink);font-family:var(--sans);padding:0!important;position:relative}
.music-reader:fullscreen{width:100vw;height:100dvh}
.music-reader button{font-family:inherit;line-height:1.3;min-height:30px;border-radius:6px;cursor:pointer;transition:background-color .16s,color .16s,border-color .16s}
.music-reader button:disabled{opacity:.4;cursor:default;background:transparent}
.music-reader button:focus-visible,.music-reader select:focus-visible,.music-reader input:focus-visible{outline:2px solid #ba8557;outline-offset:3px}
.music-heading{display:flex;align-items:center;gap:14px;padding:14px 24px 13px;min-height:74px;box-sizing:border-box;flex-shrink:0;border-bottom:1px solid var(--music-line);background:#faf9f4}
.music-heading-mark{width:36px;height:40px;display:grid;place-items:center;flex-shrink:0;color:var(--music-copper);border:1px solid #d8c9b7;border-radius:18px 18px 5px 5px;background:#f4eee2}
.music-heading-copy{flex:1;min-width:0}
.music-heading h1{font-family:Georgia,'Noto Serif SC Variable','Songti SC',SimSun,serif;font-size:21px;line-height:1.3;font-weight:500;letter-spacing:-.025em;margin:0;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
.music-subtitle{font-size:11px;line-height:1.4;color:var(--music-muted);margin:4px 0 0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
.music-details-toggle{display:flex;align-items:center;gap:10px;flex-shrink:0;background:transparent;border:1px solid #dedfd3;color:#526055;font-size:11px;padding:8px 11px}
.music-details-toggle:hover,.music-details-toggle[aria-expanded=true]{background:#eceee3;border-color:#c5cebb}
.rotated{transform:rotate(180deg)}
.music-score-toolbar{display:flex;align-items:center;gap:14px;flex-shrink:0;height:42px;box-sizing:border-box;padding:0 22px;background:#faf9f4;border-bottom:1px solid var(--music-line);position:relative;z-index:8}
.music-focus-control{flex-shrink:0}
.music-focus-toggle{display:flex;align-items:center;gap:8px;border:0;background:transparent;padding:5px 4px;color:#526458;font-size:12px}
.music-focus-toggle span{max-width:170px;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
.music-focus-toggle:hover,.music-focus-toggle.selected{color:#304d3b;background:#e9eddf}
.music-material-tag{display:flex;align-items:center;gap:6px;font-size:11px;color:#758174;white-space:nowrap}
.music-material-tag i{width:4px;height:4px;border-radius:50%;background:#789070}
.music-material-tag.pending i,.music-material-tag.synthesis i{background:#b58c55}
.music-material-tag.pending,.music-material-tag.synthesis{color:#8c7755}
.music-precision-label{margin-left:-4px}
.music-toolbar-spacer{flex:1;min-width:0}
.music-follow-toggle{display:flex;align-items:center;gap:5px;border:0;background:transparent;color:#6c796d;padding:4px 6px;font-size:10px;white-space:nowrap}
.music-follow-toggle.is-detached{background:#e6ecdd;color:#49603d}
.music-zoom{display:flex;align-items:center;gap:3px;color:#788073;font-size:10px;font-variant-numeric:tabular-nums;padding-left:10px;border-left:1px solid var(--music-line)}
.music-zoom button,.music-fullscreen{display:grid;place-items:center;border:0;background:transparent;color:inherit;width:28px;padding:4px}
.music-zoom span{min-width:30px;text-align:center}
.music-zoom button:hover,.music-fullscreen:hover{background:#e9ebdf;color:#304c3b}
.music-fullscreen{flex-shrink:0;color:#7e877a}
.music-score-stage{flex:1;min-height:0;position:relative;display:flex}
.music-screenshot{display:flex;align-items:center;gap:5px;flex-shrink:0;border:1px solid #d8dece;background:#f5f6ed;color:#4d6548;padding:5px 8px;font-size:10px;white-space:nowrap}
.music-screenshot.active{background:#e6ecdd;border-color:#a3b697;color:#304c3b}
.music-score-surface.is-selecting{cursor:crosshair;touch-action:none;user-select:none}
.music-score-selection{position:absolute;z-index:8;pointer-events:none;border:2px solid #aa744c;background:#ba85571a;box-sizing:border-box}
.music-selection-hint{position:absolute;z-index:9;left:50%;bottom:14px;transform:translateX(-50%);display:flex;align-items:center;gap:10px;max-width:calc(100% - 24px);box-sizing:border-box;padding:9px 12px;border:1px solid #d9c9a8;border-radius:8px;background:#fffaf0;box-shadow:0 3px 14px #263d361a;color:#6e593b;font-size:11px;line-height:1.6}
.music-selection-hint span{min-width:0}
.music-selection-hint button{display:grid;place-items:center;flex-shrink:0;border:0;background:transparent;color:inherit;padding:0;width:24px}
@container music (max-width:600px){.music-screenshot{width:28px;padding:4px;justify-content:center}.music-screenshot span{display:none}.music-selection-hint{width:max-content}}
.music-score-surface{flex:1;min-height:0;min-width:0;overflow:auto;scrollbar-width:thin;scrollbar-color:#bfc8b7 transparent;overscroll-behavior:contain;position:relative;outline-offset:-3px;background:#eeeee5}
.music-score-sheets{min-width:100%;margin:auto;padding:18px 24px 36px;box-sizing:border-box}
.music-score-page{background:var(--music-paper);margin:0 auto 20px;box-shadow:0 1px 2px #2637260a,0 7px 22px #26372608;border:1px solid #e7e5da}
.music-score-page :deep(svg){display:block;width:100%;height:auto;overflow:visible}
.music-score-state{padding:35px;text-align:center;font-size:13px;color:#758371}
.music-guide,.music-focus-menu{background:#fffef9;border:1px solid #d9ddcf;box-shadow:0 12px 36px #263d3626,0 2px 5px #263d3608;border-radius:10px;overflow:auto;overscroll-behavior:contain}
.music-guide{position:absolute;z-index:10;right:18px;top:66px;width:min(420px,calc(100% - 36px));max-height:calc(100% - 160px);padding:20px;box-sizing:border-box}
.music-focus-menu{position:absolute;z-index:7;left:16px;top:39px;width:310px;max-width:calc(100cqw - 32px);max-height:min(520px,65dvh);padding:16px;box-sizing:border-box}
.music-panel-heading{display:flex;align-items:center;justify-content:space-between;gap:10px;font-size:13px;font-weight:600}
.music-panel-heading button{display:grid;place-items:center;border:0;background:transparent;color:#7c8776;padding:4px;width:28px}
.music-focus-hint{font-size:11px;color:#89907e;line-height:1.6;margin:5px 0 14px}
.music-color-mode{display:flex;align-items:center;justify-content:space-between;font-size:11px;color:#858e7c;margin-bottom:10px}
.music-color-mode select{font-family:inherit;font-size:11px;color:#4a6655;border:1px solid #d4ddcc;background:#f4f6ee;border-radius:5px;padding:5px}
.music-part-options,.music-voice-options{display:flex;flex-direction:column;gap:3px}
.music-part-options button,.music-voice-options button{display:flex;align-items:center;gap:9px;min-height:36px;text-align:left;white-space:normal;border:1px solid transparent;background:transparent;padding:7px 9px;color:#5d6d61;font-size:12px}
.music-part-options button>span,.music-voice-options button>span{flex:1;min-width:0}
.music-part-options button[aria-pressed=true],.music-voice-options button[aria-pressed=true]{background:#edf1e5;border-color:#d4ddc9;color:#355a3e}
.music-part-options button:hover,.music-voice-options button:hover{background:#f2f4ea}
.music-part-options i,.music-voice-options i{width:6px;height:6px;border-radius:50%;background:var(--part-color);flex-shrink:0}
.music-part-options small,.music-voice-options small{display:block;font-size:10px;color:#8c967f;margin-top:3px;line-height:1.5}
.music-voice-options{border-top:1px solid #e9ebdf;margin-top:10px;padding-top:8px}
.music-voice-options>p{font-size:10px;color:#8c967f;margin:0 0 4px;padding-left:9px}
.music-role-empty{font-size:12px;color:#8c967f;margin:8px 0}
.music-guide-title{font:500 21px/1.4 Georgia,'Noto Serif SC Variable',SimSun,serif;margin:16px 0 4px;color:#30483a}.music-guide-subtitle{font-size:11px;line-height:1.6;color:#8a927f;margin:0 0 14px}
.music-recording{font-size:12px;line-height:1.7;display:flex;align-items:center;gap:7px;flex-wrap:wrap;margin:15px 0;color:#637363}
.music-recording span{color:#919686;font-size:11px}
.music-sections{display:flex;gap:7px;flex-wrap:wrap}
.music-sections button{display:flex;flex-direction:column;text-align:left;gap:5px;border:1px solid #d8dece;background:#f5f6ed;padding:9px 12px;font-size:12px;color:#304a3d}
.music-sections small{font-size:10px;color:#7c877b}
.music-event-links{display:grid;gap:2px;margin:14px 0}
.music-event-links button{display:flex;align-items:center;justify-content:space-between;gap:14px;text-align:left;border:0;border-bottom:1px solid #e7e9dd;border-radius:0;background:transparent;color:#4d6743;font-size:12px;padding:10px 0}
.music-event-links small{display:flex;align-items:center;gap:5px;color:#8c947e;font-size:10px;white-space:nowrap}
.music-demonstrations{display:flex;flex-direction:column;align-items:stretch;gap:6px;margin-top:16px;font-size:12px}
.music-demonstrations>span{color:#4c6549;margin-bottom:3px}
.music-demonstrations button{display:flex;gap:7px;align-items:center;border:0;background:#eef0e3;color:#536549;padding:9px 10px;font-size:11px;text-align:left}
.music-demonstrations small{color:#939a85;font-size:10px;margin-left:5px}
.music-credits{font-size:11px;color:#7f8b75;margin-top:16px;padding-top:13px;border-top:1px solid #e8eade}
.music-credits summary{cursor:pointer}
.music-credits p{font-size:11px;line-height:1.7;overflow-wrap:anywhere}
.music-credits a{color:#5b7151}
.music-cue-strip{display:flex;align-items:center;gap:15px;padding:10px 24px;flex-shrink:0;border-top:1px solid #e0e3d3;background:#f4f4e9}
.music-cue-kicker{font-size:10px;letter-spacing:.03em;color:#8b764f;flex-shrink:0;max-width:180px;line-height:1.6}
.music-cue-strip>p,.music-cue-strip>div{flex:1;min-width:0}
.music-cue-strip p{font-size:12px;line-height:1.7;color:#596a4c;margin:0}
.music-cue-strip small{display:block;font-size:10px;color:#949c84;margin-top:2px}
.music-cue-strip button{display:grid;place-items:center;border:0;background:transparent;color:#809171;padding:5px;flex-shrink:0}
.music-demo-player{position:absolute;z-index:5;right:18px;bottom:16px;width:min(380px,calc(100% - 36px));max-height:calc(100% - 24px);overflow:auto;box-sizing:border-box;display:grid;grid-template-columns:1fr auto;gap:12px;padding:18px;background:#fbf6e8;border:1px solid #d9c9a8;border-radius:12px;box-shadow:0 12px 35px #39432e29}
.music-demo-heading{display:flex;flex-direction:column;gap:5px;min-width:0}
.music-demo-heading strong{font-size:13px;line-height:1.5;font-weight:600}
.music-demo-heading span{font-size:10px;color:#998a69;line-height:1.6}
.music-demo-heading .music-demo-eyebrow{font-size:9px;letter-spacing:.09em;color:#a07d4f}
.music-demo-player>button{display:grid;place-items:center;align-self:start;border:0;background:transparent;color:#98825c;width:28px;padding:4px}
.music-demo-player .music-play-cue{grid-column:1/-1;border-left:2px solid #bc9a64;padding-left:10px}
.music-demo-player .music-play-cue strong{font-size:10px;color:#8b794f}
.music-demo-player .music-play-cue p{font-size:12px;line-height:1.75;color:#786743;margin:4px 0}
.music-demo-player .music-play-cue small{font-size:10px;color:#9e937b}
.music-demo-player audio{grid-column:1/-1;width:100%;height:34px}
.music-question-anchor{display:flex;gap:8px;align-items:center;padding:6px 24px;background:#e9eee0;font-size:11px;color:#4d6548;border-top:1px solid #dce3cf;flex-shrink:0}
.music-question-anchor span{flex:1}
.music-question-anchor button{border:0;background:transparent;display:grid;place-items:center;color:inherit;min-height:24px;width:24px;padding:4px}
.music-player-bar{flex-shrink:0;padding:15px 22px;background:#263e35;color:#f8f4e9;border-top:1px solid #1c3329;box-sizing:border-box}
.music-player-controls{display:flex;align-items:center;gap:15px;min-width:0}
.music-play{width:42px;height:42px;display:grid;place-items:center;flex-shrink:0;border:1px solid #e9d9b8!important;border-radius:50%!important;background:#f2e9d5;color:#2c4638;padding:0!important;box-shadow:0 2px 8px #112a2420}
.music-play:hover{background:#fff7e6;box-shadow:0 2px 14px #142f252b}
.music-playing-label{display:flex;flex-direction:column;gap:4px;min-width:93px;max-width:150px}
.music-playing-label strong{display:flex;align-items:center;gap:6px;font-size:12px;font-weight:500;white-space:nowrap;color:#f1eddf}
.music-playing-label i{width:4px;height:4px;border-radius:50%;background:#82977d}
.is-playing .music-playing-label i{background:#d8c29a;box-shadow:0 0 6px #c6c69775}
.music-playing-label small{font-size:10px;color:#9dad97;line-height:1.4}
.music-progress{display:flex;align-items:center;gap:10px;flex:1;min-width:60px;font-size:11px;color:#b5c2ad;font-variant-numeric:tabular-nums}
.music-progress span{flex-shrink:0;min-width:26px}
.music-progress input{appearance:none;display:block;flex:1;width:100%;min-width:20px;margin:0;height:22px;min-height:22px;border:0;border-radius:0;box-shadow:none;background:transparent;cursor:pointer;padding:0}
.music-progress input::-webkit-slider-runnable-track{height:3px;border-radius:2px;background:linear-gradient(to right,#d5bc8c var(--progress),#647965 var(--progress))}
.music-progress input::-webkit-slider-thumb{appearance:none;width:10px;height:10px;border-radius:50%;background:#e5ce9e;margin-top:-3.5px;border:2px solid #263e35;box-shadow:0 0 0 1px #d5bc8c}
.music-progress input::-moz-range-track{height:3px;border-radius:2px;background:#647965}
.music-progress input::-moz-range-progress{height:3px;background:#d5bc8c}
.music-progress input::-moz-range-thumb{width:8px;height:8px;border-radius:50%;background:#e5ce9e;border:2px solid #263e35}
.music-rate select{font-family:inherit;font-size:11px;background:#263e35;border:0;color:#d6dec9;cursor:pointer;padding:5px 0;min-width:43px}
.music-loop{display:flex;align-items:center;gap:5px;flex-shrink:0;border:0;background:transparent;color:#abbba3;padding:6px;font-size:11px}
.music-loop.active{color:#ead2a5;background:#385343}
.music-loop span{display:none}
.music-ask{display:flex;align-items:center;gap:7px;flex-shrink:0;border:1px solid #d0b289;background:#e4c49a;color:#354a37;padding:9px 13px;font-size:12px;white-space:nowrap;border-radius:6px!important}
.music-ask:hover{background:#efdab7;border-color:#efdab7}
.music-error{font-size:12px;color:#f3bda1;line-height:1.6;margin:10px 0 0}
.sr-only{position:absolute;width:1px;height:1px;overflow:hidden;clip:rect(0,0,0,0)}
@container music (max-width:760px){.music-playing-label{display:none}.music-follow-toggle span{display:none}.music-heading{padding-inline:18px}.music-score-toolbar{gap:10px;padding-inline:16px}.music-score-sheets{padding:12px 14px 28px}.music-player-controls{gap:12px}.music-player-bar{padding-inline:16px}}
@container music (max-width:480px){.music-heading{padding:12px 14px;gap:10px;min-height:72px}.music-heading-mark{width:29px;height:34px}.music-heading h1{font-size:18px}.music-subtitle{font-size:10px}.music-details-toggle{padding:6px 7px;gap:4px;font-size:10px}.music-score-toolbar{height:40px;gap:8px;padding-inline:12px}.music-focus-toggle{font-size:11px;gap:5px}.music-focus-toggle span{max-width:95px}.music-material-tag{font-size:10px;gap:4px}.music-follow-toggle{display:none}.music-zoom{gap:0;padding-left:6px;font-size:9px}.music-zoom button{width:23px}.music-zoom span{min-width:29px}.music-fullscreen{width:25px;padding:3px}.music-toolbar-spacer{min-width:0}.music-score-sheets{padding:10px 8px 24px;min-width:560px}.music-guide{top:65px;right:8px;width:calc(100% - 16px);padding:16px;max-height:calc(100% - 155px)}.music-focus-menu{left:8px;max-width:calc(100cqw - 16px);max-height:min(480px,60dvh)}.music-cue-strip{padding:9px 14px;gap:6px;align-items:flex-start;flex-wrap:wrap}.music-cue-kicker{font-size:9px;max-width:none}.music-cue-strip>div{flex-basis:100%}.music-cue-strip>p{flex-basis:calc(100% - 30px)}.music-cue-strip p{font-size:11px;line-height:1.6}.music-cue-strip small{font-size:9px}.music-demo-player{right:8px;bottom:10px;width:calc(100% - 16px);padding:14px;gap:9px}.music-demo-player .music-play-cue p{font-size:11px}.music-player-bar{padding:9px 14px 12px}.music-player-controls{display:grid;grid-template-columns:36px 1fr auto auto auto;gap:9px}.music-progress{grid-row:1;grid-column:1/-1;font-size:10px;gap:8px}.music-play{grid-row:2;width:36px;height:36px}.music-playing-label{grid-row:2;display:flex;min-width:0;max-width:none}.music-playing-label strong{font-size:11px}.music-playing-label small{font-size:10px}.music-rate,.music-loop,.music-ask{grid-row:2}.music-rate select{font-size:10px;min-width:38px}.music-loop{padding:5px}.music-ask{padding:8px 9px;font-size:11px;gap:5px}.music-question-anchor{padding:5px 14px;font-size:10px}.music-material-tag.pending .music-precision-label{max-width:62px;overflow:hidden;white-space:nowrap}}
@container music (max-width:480px){
  .music-score-toolbar{gap:6px}
  .music-focus-control{min-width:0;flex-shrink:1}
  .music-focus-toggle{max-width:100%}
  .music-focus-toggle span{min-width:0}
  .music-material-tag{min-width:52px;overflow:hidden}
  .music-material-tag i{flex-shrink:0}
  .music-follow-toggle{display:flex;width:24px;padding:4px;flex-shrink:0}
  .music-follow-toggle:disabled{display:none}
}
@media(prefers-reduced-motion:reduce){.music-reader button{transition:none}}
</style>

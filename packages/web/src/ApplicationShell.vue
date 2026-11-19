<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { pendingNotePlacementLocation } from './note-placements';
import { noteRequest, pendingNoteSource, noteSourceError, notesPanel, type NoteContext } from './personal-notes';
import { openPersonalChat } from './personal-chat';
import PersonalNotes from './components/PersonalNotes.vue';
import PersonalChat from './components/PersonalChat.vue';
watch(pendingNotePlacementLocation, async destination => {
  if (!destination || destination.kind !== 'attachment') return;
  try {
    const context = await noteRequest<NoteContext>('attachment_context', {target:destination.target});
    if (context.book_id) pendingNoteSource.value = context;
    else {
      await openPersonalChat(undefined, 'discuss', undefined, context.session_id!);
      await nextTick();
      if (destination.target.kind === 'message') {
        const key = `${destination.target.session_id}:${destination.target.turn_id}:${destination.target.role}`;
        Array.from(document.querySelectorAll<HTMLElement>('[data-note-message]')).find(el=>el.dataset.noteMessage===key)?.scrollIntoView({block:'center',behavior:'smooth'});
      }
      notesPanel.open = false;
    }
    noteSourceError.value = '';
  } catch (failure) { noteSourceError.value = (failure as Error).message; }
  finally { if (pendingNotePlacementLocation.value === destination) pendingNotePlacementLocation.value = null; }
});
const content = ref<HTMLElement>();
const contentSize = new ResizeObserver(([entry]) => {
  content.value?.style.setProperty('--app-content-height', `${entry.contentRect.height}px`);
  content.value?.style.setProperty('--app-content-width', `${entry.contentRect.width}px`);
});
onMounted(() => { if (content.value) contentSize.observe(content.value); });
onBeforeUnmount(() => contentSize.disconnect());
</script>
<template>
  <div class="application-shell">
    <div ref="content" class="application-content"><div id="application-overlays" /><slot /><PersonalChat /></div>
    <PersonalNotes />
  </div>
</template>

<style scoped>
.application-shell { height: 100%; min-height: 0; }
.application-content { height: 100%; min-width: 0; min-height: 0; overflow: auto; position: relative; contain: layout paint;
  /* Overlays use this content box; #app already follows the keyboard viewport. */
  --app-input-viewport-height: var(--app-content-height);
  --app-input-viewport-top: 0px;
}
</style>

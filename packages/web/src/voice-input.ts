import { shallowRef } from 'vue';

// Multiple question cards and the chat composer share the same microphone.
export const voiceOwner = shallowRef<symbol | null>(null);

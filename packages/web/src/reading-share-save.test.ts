// @vitest-environment happy-dom
import { afterEach, expect, it, vi } from 'vitest';
import { saveShare } from './reading-share';
import { save } from '@tauri-apps/plugin-dialog';
import { invoke } from '@tauri-apps/api/core';
vi.mock('@tauri-apps/plugin-dialog', () => ({ save: vi.fn() }));
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
afterEach(() => { delete (window as any).__TAURI_INTERNALS__; vi.resetAllMocks(); });

it('saves the preview bytes through the desktop command only after an explicit path choice', async () => {
  (window as any).__TAURI_INTERNALS__ = {};
  vi.mocked(save).mockResolvedValue('C:\\chosen.png'); vi.mocked(invoke).mockResolvedValue(undefined);
  expect(await saveShare(new Blob([new Uint8Array([1, 2, 3])]), 'blob:preview', () => true)).toBe(true);
  expect(invoke).toHaveBeenCalledWith('save_reading_share_image', { path: 'C:\\chosen.png', bytes: [1, 2, 3] });
});
it('does not save after cancellation or a scope change while the native dialog is open', async () => {
  (window as any).__TAURI_INTERNALS__ = {};
  vi.mocked(save).mockResolvedValueOnce(null);
  expect(await saveShare(new Blob(['png']), 'blob:preview', () => true)).toBe(false);
  let current = true;
  vi.mocked(save).mockImplementationOnce(async () => { current = false; return 'C:\\chosen.png'; });
  expect(await saveShare(new Blob(['png']), 'blob:preview', () => current)).toBe(false);
  expect(invoke).not.toHaveBeenCalled();
});

import { beforeEach, describe, expect, it, vi } from 'vitest';
const respond = vi.hoisted(() => vi.fn());
vi.mock('./api', () => ({ api: { musicRespond: respond } }));
vi.mock('./network-context', () => ({ sceneKey: () => 'scene' }));
import { musicDispatcher, registerMusicPlayer } from './music-interaction';
import { initialRun } from './agent-run-state';
import type { MusicCommand, MusicPlaybackReceipt } from './music-api';
const command: MusicCommand = { command_id: 'play-1', run_turn_id: 'turn-1', session_id: 'session', request: { operation: 'act', publication: { book_id: 'music', publication_id: 'pub' }, recording: { recording_id: 'take', revision: '1' }, based_on: 'observation', action: { kind: 'play', start_ms: 1000 } } };
const snapshot = () => ({ ...initialRun({ book_id: 'music', session_id: 'session', turn_id: 'turn-1' }), music_command: command });
describe('music command delivery', () => {
  beforeEach(() => { respond.mockReset(); });
  it('executes a claimed command once and reports playback failure instead of success', async () => {
    respond.mockResolvedValue({ claimed: true, accepted: true });
    const receipt = { status: 'failed', error: 'Autoplay denied' } as MusicPlaybackReceipt;
    const execute = vi.fn().mockResolvedValue(receipt), unregister = registerMusicPlayer({ id: 'player', execute });
    const dispatcher = musicDispatcher(); dispatcher.update(snapshot()); dispatcher.update(snapshot());
    await vi.waitFor(() => expect(respond).toHaveBeenCalledTimes(2));
    expect(execute).toHaveBeenCalledTimes(1);
    expect(respond.mock.calls[1][0]).toMatchObject({ operation: 'complete', receipt: { status: 'failed', error: 'Autoplay denied' } });
    unregister();
  });
  it('does not run a command after the player has unmounted', async () => {
    let resolveClaim: (value: unknown) => void = () => {};
    respond.mockImplementation(() => new Promise(resolve => { resolveClaim = resolve; }));
    const execute = vi.fn(), unregister = registerMusicPlayer({ id: 'player', execute });
    const dispatcher = musicDispatcher(); dispatcher.update(snapshot()); await vi.waitFor(() => expect(respond).toHaveBeenCalledOnce()); unregister(); resolveClaim({ claimed: true });
    await Promise.resolve(); await Promise.resolve();
    expect(execute).not.toHaveBeenCalled(); expect(respond).toHaveBeenCalledTimes(1);
  });
  it('does not change playback when another player has claimed the command', async () => {
    respond.mockResolvedValue({ claimed: false });
    const execute = vi.fn(), unregister = registerMusicPlayer({ id: 'player', execute });
    musicDispatcher().update(snapshot());
    await vi.waitFor(() => expect(respond).toHaveBeenCalledOnce());
    expect(execute).not.toHaveBeenCalled(); unregister();
  });
});

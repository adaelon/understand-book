import type { MusicCommand, MusicPlaybackReceipt } from './music-api';
import type { RunSnapshot } from './agent-run-state';
import { api } from './api';
import { sceneKey } from './reader-context';
interface MusicPlayer { id: string; execute(command: MusicCommand): Promise<MusicPlaybackReceipt>; accepted?(receipt: MusicPlaybackReceipt): void }
const players = new Set<MusicPlayer>();
export function registerMusicPlayer(player: MusicPlayer) { players.add(player); return () => { players.delete(player); }; }
/** Claim commands before changing playback; replayed SSE events never repeat an action. */
export function musicDispatcher() {
  const attempted = new Set<string>();
  let generation = 0;
  let work = Promise.resolve();
  function stop() { generation++; }
  function update(snapshot: RunSnapshot) {
    const command = snapshot.music_command;
    const running = snapshot.persistence_state === 'pending' && ['running', 'waiting_model'].includes(snapshot.execution_state);
    if (!running) stop();
    if (!running || !command || command.run_turn_id !== snapshot.descriptor.turn_id || command.session_id !== snapshot.descriptor.session_id || attempted.has(command.command_id)) return;
    attempted.add(command.command_id);
    const expectedGeneration = generation, player = [...players].at(-1), scene = sceneKey();
    const live = () => generation === expectedGeneration && sceneKey() === scene && !!player && players.has(player);
    work = work.then(async () => {
      if (generation !== expectedGeneration || sceneKey() !== scene) return;
      const claim = await api.musicRespond({ operation: 'claim', command_id: command.command_id, run_turn_id: command.run_turn_id,
        publication: command.request.publication, recording: command.request.recording, player_id: player?.id ?? null });
      if (!claim.claimed || !live()) return;
      const receipt = await player!.execute(command);
      if (!live()) return;
      const result = await api.musicRespond({ operation: 'complete', command_id: command.command_id, run_turn_id: command.run_turn_id, player_id: player!.id, receipt });
      if (live() && result.receipt) player!.accepted?.(result.receipt);
    }).catch(() => { /* An uncertain response never re-executes playback. */ });
  }
  return { update, stop };
}

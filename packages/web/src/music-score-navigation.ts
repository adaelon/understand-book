import type { MusicScoreObject } from '../../core/src/music-contract';

/** Stay in the same measure when switching instruments, including resting parts. */
export function scorePartTarget(objects: readonly MusicScoreObject[], partId: string, measure?: string) {
  return objects.find(object => object.part_id === partId && object.measure === measure)
    ?? objects.find(object => object.part_id === partId);
}

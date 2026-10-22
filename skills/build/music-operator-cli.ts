import { runOperatorMediaCli } from './video-operator-cli';
import { runOperatorMusicBuild } from './music-operator-build';

export async function runOperatorMusicCli() {
  return runOperatorMediaCli('music', runOperatorMusicBuild);
}

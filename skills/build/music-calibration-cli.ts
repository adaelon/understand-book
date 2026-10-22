import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { parseArgs } from 'node:util';
import { calibrateMusicWorkspace } from '../../packages/core/src/music-import';
import type { MusicAlignmentRevision } from '../../packages/core/src/music-alignment';

/** Local workbench edits use the same atomic artifact publication as curated builds. */
export function runMusicCalibrationCli(args = process.argv.slice(2)): void {
  try {
    const { values } = parseArgs({ args, strict: true, options: {
      'book-dir': { type: 'string' }, 'revision-file': { type: 'string' },
    } });
    if (!values['book-dir'] || !values['revision-file']) {
      throw new Error('Usage: music-calibrate --book-dir <workspace> --revision-file <revision.json>');
    }
    const revision = JSON.parse(readFileSync(path.resolve(values['revision-file']), 'utf8')) as MusicAlignmentRevision;
    const manifest = calibrateMusicWorkspace(path.resolve(values['book-dir']), revision);
    process.stdout.write(JSON.stringify({ manifest }) + '\n');
  } catch (error) {
    process.stderr.write((error instanceof Error ? error.message : String(error)) + '\n');
    process.exitCode = 1;
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) runMusicCalibrationCli();

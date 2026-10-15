import { copyFileSync, mkdirSync, readFileSync, writeFileSync, existsSync, appendFileSync } from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
import { runVideoTranslation, publishVideoTranslationSnapshot } from '../../skills/build/video-translation';
import { prepareVideoTranslation } from '../../packages/core/src/video-translation';
const executable = process.argv[2];
assert(executable, 'Provide Codex executable');
const root = path.resolve(process.argv[3] ?? 'evals/video/generated/vi10');
const workspace = path.join(root, 'workspace'), service = path.join(root, 'service');
mkdirSync(workspace, { recursive: true });
const original = path.resolve('evals/video/generated/vi2/moved-workspace');
const source = JSON.parse(readFileSync(path.join(original, 'video_source_manifest.json'), 'utf8'));
const map = JSON.parse(readFileSync(path.join(original, source.transcript_map_path), 'utf8'));
if (!existsSync(path.join(workspace, 'video_source_manifest.json'))) {
  for (const file of ['base.json', 'source.txt', 'video_import.json', source.transcript_map_path, map.raw_result_path, source.source.imported_path]) {
    mkdirSync(path.dirname(path.join(workspace, file)), { recursive: true }); copyFileSync(path.join(original, file), path.join(workspace, file));
  }
  copyFileSync(path.join(original, 'video_source_manifest.json'), path.join(workspace, 'video_source_manifest.json'));
}
const started = Date.now(), publisher = path.resolve('target/debug/publish_book.exe');
const publish = async (group: string) => {
  const result = await publishVideoTranslationSnapshot(workspace, publisher, service);
  appendFileSync(path.join(root, 'publications.jsonl'), JSON.stringify({ group, since_start_ms: Date.now() - started, ...result }) + '\n');
  console.log(JSON.stringify({ group, published: result.reference, elapsed_ms: result.elapsed_ms, reused: result.reused }));
};
if (!existsSync(path.join(workspace, 'video_publication.json'))) await publish('english');
await runVideoTranslation(workspace, path.resolve(executable), publish);
const plan = prepareVideoTranslation(workspace);
const link = JSON.parse(readFileSync(path.join(workspace, 'video_publication.json'), 'utf8'));
const directory = path.resolve(workspace, link.directory);
const translations = JSON.parse(readFileSync(path.join(directory, 'transcript_translations.json'), 'utf8'));
assert.equal(translations.length, map.segments.length);
assert.equal(new Set(translations.map((t: any) => t.segment_id)).size, map.segments.length);
for (const t of translations) assert.deepEqual(t.transcript, map.transcript);
assert(readFileSync(path.join(directory, 'source.txt')).equals(readFileSync(path.join(original, 'source.txt'))));
assert.deepEqual(JSON.parse(readFileSync(path.join(directory, 'transcript_map.json'), 'utf8')), map);
const manifest = JSON.parse(readFileSync(path.join(directory, 'video_source_manifest.json'), 'utf8'));
assert.deepEqual(manifest.coverage.find((c: any) => c.capability === 'translation').pending, []);
const result = { status: 'passed', material: plan.material, segments: translations.length, tasks: plan.tasks.length,
  groups: new Set(plan.tasks.map(t => t.group)).size, elapsed_ms: Date.now() - started, publication: link.reference, directory,
  english_unchanged: true, translation_coverage: manifest.coverage.find((c: any) => c.capability === 'translation') };
writeFileSync(path.join(root, 'verification.json'), JSON.stringify(result, null, 2));
console.log(JSON.stringify(result));

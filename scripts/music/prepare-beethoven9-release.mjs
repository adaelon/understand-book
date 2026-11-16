import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import path from 'node:path';
import { synthesizeScoreDemonstration } from '../../packages/core/src/music-demonstrations.ts';
import { validateMusicSourceManifest } from '../../packages/core/src/music-contract.ts';

const root = path.resolve(process.argv[2] ?? '.understand-book/beethoven9-tennstedt');
const write = (file, value) => writeFileSync(file, JSON.stringify(value, null, 2) + '\n');
for (const movement of ['m1', 'm2', 'm3', 'm4']) {
  const workspace = path.join(root, movement);
  const candidate = path.join(workspace, '.build/music/import-candidate.json');
  const manifest = JSON.parse(readFileSync(candidate, 'utf8'));
  manifest.book_id = `music-beethoven9-tennstedt-${movement}-release1`;
  for (const right of manifest.rights) {
    right.distribution = 'allowed';
    if (right.asset_kind === 'recording') right.license = '用户提供视频音轨；用户已选择启用分发，未附独立授权文件';
  }
  const sourceEvents = JSON.parse(readFileSync(path.join(workspace, `music/archive/score-${movement}.json`), 'utf8')).events;
  const sources = new Map(sourceEvents.map(event => [event.source_id, event]));
  const objects = new Map(manifest.score_objects.map(object => [object.source_id, object]));
  const provenanceId = 'release-score-study';
  manifest.provenance = manifest.provenance.filter(item => ![provenanceId, 'release-score-synthesis'].includes(item.provenance_id));
  manifest.provenance.push({ provenance_id: provenanceId, kind: 'score', description: '根据原谱音高、时值和明确声部编写的听辨活动与谱面合成对照，原声听辨效果另行记录。', source_url: null, processor: 'beethoven9 release preparation v1' });
  if (movement === 'm2') {
    const violin = manifest.parts.find(part => part.name === '第一小提琴').part_id;
    const timpani = manifest.parts.find(part => part.name === '定音鼓').part_id;
    manifest.events = manifest.events.filter(event => !event.event_id.startsWith('m2-listening-'));
    const groups = [
      ['strings', '辨认弦乐的附点节奏', '3', violin, '先听较长的第一音，再听接着的短音和中等长度的一音。试着拍出“长—短—中”，再留意后面的停顿。'],
      ['timpani', '辨认定音鼓的节奏回应', '5', timpani, '先回想弦乐的“长—短—中”，再听定音鼓怎样用同样的时值组合回应。比较音色和音高的变化。'],
    ];
    for (const [id, title, measure, partId, prompt] of groups) {
      const notes = manifest.score_objects.filter(note => note.measure === measure && note.part_id === partId && !note.rest).slice(0, 3);
      const occurrence = manifest.occurrences.find(item => item.measure === measure);
      manifest.events.push({ event_id: `m2-listening-${id}`, title, explanation: `源小节 ${measure}，三个音的时值依次为 1.5、0.5、1 个四分音符，第二音比第一音低一个八度。`, listen_for: prompt,
        start_ms: occurrence.start_ms, end_ms: occurrence.end_ms, roles: [{ part_id: partId, role: '附点节奏', voice_ids: [...new Set(notes.map(note => note.voice))], theme_id: 'm2:opening-rhythm' }],
        evidence: [{ provenance_id: provenanceId, score_object_ids: notes.map(note => note.source_id), audio: { recording: manifest.recording.source, start_ms: occurrence.start_ms, end_ms: occurrence.end_ms } }], relations: [], concept_tags: ['导听练习', '谱面依据', '听核待完成'] });
    }
    const first = manifest.events.find(event => event.event_id === 'm2-listening-strings');
    const second = manifest.events.find(event => event.event_id === 'm2-listening-timpani');
    manifest.events.push({ event_id: 'm2-listening-compare', title: '比较弦乐与定音鼓的同一节奏', explanation: '弦乐在源小节 3，定音鼓在源小节 5，中间保留源小节 4 的停顿。', listen_for: '听两次“长—短—中”之间的停顿。第二次换了音色，尝试只拍节奏，再说出哪些地方相同。',
      start_ms: first.start_ms, end_ms: second.end_ms, roles: [...first.roles, ...second.roles], evidence: [{ provenance_id: provenanceId, score_object_ids: [...first.evidence[0].score_object_ids, ...second.evidence[0].score_object_ids], audio: { recording: manifest.recording.source, start_ms: first.start_ms, end_ms: second.end_ms } }], relations: [], concept_tags: ['导听练习', '谱面依据', '听核待完成'] });
  }
  if (movement === 'm2' || movement === 'm4') {
    manifest.provenance.push({ provenance_id: 'release-score-synthesis', kind: 'score_synthesis', description: '按原谱音高和时值以统一音色合成，第二乐章采用每分钟 120 个四分音符，第四乐章采用 180。', source_url: null, processor: 'score-oscillator.v1' });
    manifest.demonstrations = manifest.demonstrations.filter(demo => !demo.demonstration_id.startsWith('release-'));
    mkdirSync(path.join(workspace, 'music/demonstrations'), { recursive: true });
    for (const event of manifest.events.filter(event => event.concept_tags.includes('导听练习'))) {
      const ids = [...new Set(event.evidence.flatMap(item => item.score_object_ids))];
      const notes = ids.map(id => ({ ...objects.get(id), score_beat: sources.get(id).q[0] / sources.get(id).q[1], grace: false }));
      const tempo = movement === 'm2' ? 120 : 180;
      const result = synthesizeScoreDemonstration(notes, tempo);
      const id = `release-${event.event_id}`;
      const mediaPath = `music/demonstrations/${id}.wav`;
      writeFileSync(path.join(workspace, mediaPath), result.wav);
      manifest.demonstrations.push({ demonstration_id: id, title: `${event.title} · 谱面合成`, kind: 'score_synthesis', media_path: mediaPath, content_type: 'audio/wav', duration_ms: result.duration_ms,
        part_ids: [...new Set(notes.map(note => note.part_id))], score_object_ids: ids, provenance_id: 'release-score-synthesis' });
    }
  }
  for (const section of manifest.structure) section.event_ids = manifest.events.filter(event => !event.concept_tags.includes('lyrics') && section.start_ms <= event.start_ms && section.end_ms >= event.end_ms).map(event => event.event_id);
  let guide = readFileSync(path.join(workspace, 'source.txt'), 'utf8').split('<!-- release-study -->')[0].replaceAll('录音仅供本机使用。', '').trimEnd();
  guide += '\n\n<!-- release-study -->\n## 听辨与对照\n\n';
  if (movement === 'm4') guide += '先在作品导览辨认拥抱主题，再辨认欢乐主题，最后尝试同时保留两条线。每次只关注一个特征：拥抱主题的较长同音与下行，欢乐主题的长短交替与上行。如果难以跟住，可先播放对应的谱面合成示范，再回到相同区间的原录音。合成示范使用统一音色帮助比较谱音。\n\n';
  else if (movement === 'm2') guide += '从开头弦乐和定音鼓的“长—短—中”入手。先拍出第一组的节奏，再留意停顿后的回应，比较时值是否相同、音色如何变化。可用三个谱面合成示范分别听、再连起来听。\n\n';
  else guide += '先选曲式段落，再从归档材料中选一个声部，完整回听该段。用谱面核对旋律上行、下行或同音重复，再描述自己在原声中听到的内容。\n\n';
  guide += '学习记录可写下能跟住的声部、辨认依据和仍拿不准的位置，后续讲解沿这些具体反馈继续。\n';
  writeFileSync(path.join(workspace, 'source.txt'), guide);
  write(candidate, validateMusicSourceManifest(manifest));
  console.log(JSON.stringify({ movement, book_id: manifest.book_id, exercises: manifest.events.filter(event => event.concept_tags.includes('导听练习')).length, demonstrations: manifest.demonstrations.length }));
}

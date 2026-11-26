import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { expect, it } from 'vitest';
import { courseTrialCommand } from '../src/course-trial';

it('resumes trial revisions and route decisions without changing the formal course or exporting private notes', () => {
  const root = mkdtempSync(path.join(tmpdir(), 'course-trial-'));
  try {
    const book = path.join(root, '.understand-book', 'guide');
    mkdirSync(book, { recursive: true });
    function save(name: string, value: unknown) {
      const file = path.join(book, name);
      writeFileSync(file, JSON.stringify(value));
      return file;
    }
    const run = (op: string, ...args: string[]) => courseTrialCommand([op, book, ...args]);
    writeFileSync(path.join(book, 'source.txt'), '# Guide\n\nA single example.\n\n# Chapter two\n\nA second example.\n');
    save('base.json', { book_id: 'guide', graph_nodes: [], graph_edges: [] });
    save('source_manifest.json', { book_id: 'guide', canonical_source: { kind: 'markdown', truth_file: 'source.txt' } });
    save('profile_metadata.json', { header: { book_id: 'guide', profile_id: 'technical_learning' } });
    save('discourse_index.json', { items: [] });
    save('formula_semantics.json', []);
    save('book_structure.json', { spine: ['1', '2'].map(lid => ({ lid, summary: { text: 'A chapter' } })), throughlines: [] });
    const existing = save('inquiry_course.json', { previous: 'published content' });
    const trial = run('template', '1');
    if (!('course' in trial) || !trial.course) throw new Error('Expected a trial template');
    const lesson = trial.course.lessons[0];
    lesson.assessment.source_lids = [lesson.summary.source.start_lid];
    lesson.assessment.rule.items[0].source_lids = lesson.assessment.source_lids;
    const candidate = save('candidate.json', trial);
    const report = save('report.json', { selection_reason: 'A representative section', independent_solution: 'Solved the questions',
      source_review: 'Compared the solutions to the original example' });
    expect(run('install', 'p-1', candidate, report)).toMatchObject({ reader_path: '/?course_book=guide&trial=p-1' });
    expect(run('install', 'p-1', candidate, report)).toMatchObject({ revision: 'p-1' });
    const note = save('feedback.json', { activity: 'final task', observation: 'The question omitted the condition', change: 'State the condition' });
    run('feedback', 'p-1', 'note-1', note);
    lesson.task.question = 'Apply the rule under the stated condition.';
    save('candidate.json', trial);
    expect(() => run('install', 'p-1', candidate, report)).toThrow(/identifier/);
    run('install', 'p-2', candidate, report, 'p-1');
    const brief = { title: 'Read examples', question: 'Which examples support the claim?', audience: 'First-time readers',
      final_task: 'Explain a new example', unit_lids: ['1'], example_lessons: ['How do the two cases differ?'] };
    expect(() => run('propose', 'p-2', 'first', save('route.json', { ...brief, unit_lids: ['missing'] }))).toThrow(/missing/);
    run('propose', 'p-2', 'first', save('route.json', brief));
    run('pause', 'p-2', 'first');
    expect(run('state', 'p-2')).toMatchObject({ review: { previous_revision: 'p-1' }, routes: [{ status: 'paused' }] });
    const selected = run('accept', 'p-2', 'first');
    if (!('accepted_route_path' in selected) || !selected.accepted_route_path) throw new Error('Expected a selected route');
    expect(JSON.parse(readFileSync(selected.accepted_route_path, 'utf8'))).toEqual({ ...brief, source_id: trial.source_id,
      source_revision: trial.source_revision, status: 'accepted', trial_revision: 'p-2' });
    expect(run('state', 'p-1')).toMatchObject({ feedback: [{ note: { observation: 'The question omitted the condition' } }], routes: [] });
    expect(existsSync(path.join(book, '.build', 'course-trials', 'p-1', 'inquiry_course.json'))).toBe(true);
    expect(JSON.parse(readFileSync(existing, 'utf8'))).toEqual({ previous: 'published content' });
    writeFileSync(path.join(book, 'source.txt'), '# Guide\n\nRevised example.\n');
    expect(() => run('accept', 'p-2', 'first')).toThrow(/原文版本/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

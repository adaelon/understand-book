import { existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, renameSync, rmSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { isDeepStrictEqual } from 'node:util';
import { z } from 'zod';
import { readTeachingBuildInput, resolveAutomaticBuildTarget } from './build-orchestrator';
import { CourseBlueprintZ, CourseBodyZ, courseGenerationContracts, scopeFromAcceptedRoute, validateCourseLessonBody } from './course-build';
import { AcceptedCourseRouteZ } from './accepted-course-route';

const text = z.string().trim().min(1);
const id = z.string().regex(/^[A-Za-z0-9_-]+$/, 'Expected a revision or identifier using letters, digits, hyphens or underscores');
const reviewSchema = z.object({ selection_reason: text, independent_solution: text, source_review: text }).strict();
const feedbackSchema = z.object({ activity: text, observation: text, change: text }).strict();
const sourceSchema = z.object({
  publication: z.object({ book_id: text, publication_id: text }).strict(), source_revision: text,
  start_lid: text, end_lid: text, label: text, structure_refs: z.array(text).min(1),
}).strict();
const lessonSummary = CourseBlueprintZ.pick({ question: true, objective: true, prerequisites: true,
  estimated_minutes: true, duration_basis: true }).extend({ id: text, unit_id: text, source: sourceSchema });
const trialSchema = z.object({
  version: z.literal('course_artifact.v1'), source_id: text, source_revision: text,
  course: z.object({
    language: text, units: z.tuple([z.object({ id: text, title: text }).strict()]),
    routes: z.tuple([z.object({ id: text, title: text, question: text, structure_ref: text, lesson_ids: z.tuple([text]) }).strict()]),
    lessons: z.tuple([CourseBodyZ.extend({ summary: lessonSummary })]),
  }).strict(),
}).strict();

function readJson(file: string): unknown {
  return JSON.parse(readFileSync(file, 'utf8').replace(/^\uFEFF/, ''));
}

function directories(dir: string): string[] {
  if (!existsSync(dir)) return [];
  return readdirSync(dir, { withFileTypes: true })
    .filter(entry => entry.isDirectory() && !entry.name.startsWith('.')).map(entry => entry.name).sort();
}

// Each install / proposal / decision is an immutable directory. A retry either sees
// the identical completed result or reports that the caller needs a new identifier.
function storeOnce(directory: string, files: Record<string, unknown>) {
  const parent = path.dirname(directory);
  mkdirSync(parent, { recursive: true });
  const temporary = mkdtempSync(path.join(parent, '.prepare-'));
  try {
    for (const [name, value] of Object.entries(files)) {
      writeFileSync(path.join(temporary, name), JSON.stringify(value, null, 2) + '\n', 'utf8');
    }
    try {
      renameSync(temporary, directory);
    } catch (error) {
      if (!existsSync(directory)) throw error;
      for (const [name, value] of Object.entries(files)) {
        if (!isDeepStrictEqual(readJson(path.join(directory, name)), value)) {
          throw new Error('This identifier already has different content. Use a new identifier for the revision.');
        }
      }
    }
  } finally {
    rmSync(temporary, { recursive: true, force: true });
  }
}

function trialState(directory: string) {
  const routes = path.join(directory, 'routes');
  const notes = path.join(directory, 'feedback');
  return {
    revision: path.basename(directory),
    review: readJson(path.join(directory, 'course_build_report.json')),
    feedback: directories(notes).map(name => ({ id: name, note: readJson(path.join(notes, name, 'note.json')) })),
    routes: directories(routes).map(name => {
      const proposed = path.join(routes, name);
      const accepted = path.join(proposed, 'accepted', 'accepted-route.json');
      const status = existsSync(accepted) ? 'accepted' : existsSync(path.join(proposed, 'paused')) ? 'paused' : 'proposed';
      return { id: name, status, route: readJson(path.join(proposed, 'proposal.json')),
        ...(status === 'accepted' ? { accepted_route_path: accepted } : {}) };
    }),
  };
}

export function courseTrialCommand(argv: string[]) {
  const [operation, book, revision, item, inputFile, previous] = argv;
  if (!book) throw new Error('Usage: course-trial catalog|read|template|install|state|feedback|propose|pause|accept BOOK ...');
  const workspace = path.resolve(book);
  const trials = path.join(workspace, '.build', 'course-trials');
  const openSource = () => readTeachingBuildInput(resolveAutomaticBuildTarget(workspace));
  if (operation === 'catalog' || operation === 'read' || operation === 'template') {
    const { structure, source } = openSource();
    if (operation === 'catalog') return { source_id: source.source_id, source_revision: source.source_revision,
      units: structure.spine.map(unit => ({ lid: unit.lid, summary: unit.summary.text })), throughlines: structure.throughlines };
    const passages = source.passages.filter(passage => passage.unit_lid === revision);
    if (!passages.length) throw new Error('Selected unit has no readable passages');
    if (operation === 'template') return { version: 'course_artifact.v1', source_id: source.source_id, source_revision: source.source_revision,
      course: { language: 'zh-CN', units: [{ id: 'unit', title: '所选单元' }],
        routes: [{ id: 'trial', title: '试课标题', question: '本节主问题', structure_ref: revision, lesson_ids: ['lesson'] }],
        lessons: [{ summary: { id: 'lesson', unit_id: 'unit', question: '本节主问题', objective: '可观察的学成目标',
          prerequisites: '读者的起点', estimated_minutes: 10, duration_basis: '实际活动用时估计',
          source: { publication: { book_id: source.source_id, publication_id: 'local' }, source_revision: source.source_revision,
            start_lid: passages[0].lid, end_lid: passages[passages.length - 1].lid, label: '实际使用的原文', structure_refs: [revision] } },
        ...courseGenerationContracts.lesson }] } };
    return { source_id: source.source_id, source_revision: source.source_revision, passages };
  }
  if (operation === 'state' && !revision) return directories(trials).map(name => trialState(path.join(trials, name)));
  const directory = path.join(trials, id.parse(revision));
  const artifactFile = path.join(directory, 'inquiry_course.json');
  if (operation === 'install') {
    const artifact = trialSchema.parse(readJson(item));
    const review = reviewSchema.parse(readJson(inputFile));
    const { source } = openSource();
    const lesson = artifact.course.lessons[0];
    const binding = lesson.summary.source;
    if (artifact.source_id !== source.source_id || binding.publication.book_id !== source.source_id ||
        artifact.source_revision !== source.source_revision || binding.source_revision !== source.source_revision) {
      throw new Error('The trial belongs to a different book or source revision');
    }
    if (lesson.summary.id !== artifact.course.routes[0].lesson_ids[0] || lesson.summary.unit_id !== artifact.course.units[0].id) {
      throw new Error('The trial route must open its single lesson');
    }
    const start = source.passages.findIndex(p => p.lid === binding.start_lid);
    const end = source.passages.findIndex(p => p.lid === binding.end_lid);
    if (start < 0 || end < start) {
      throw new Error('The trial evidence must be drawn from its cited original passages');
    }
    validateCourseLessonBody({ source: { ...source, passages: source.passages.slice(start, end + 1) } }, lesson);
    if (previous && !existsSync(path.join(trials, id.parse(previous), 'inquiry_course.json'))) throw new Error('Unknown previous trial revision');
    storeOnce(directory, { 'inquiry_course.json': artifact,
      'course_build_report.json': { ...review, ...(previous ? { previous_revision: previous } : {}) } });
    return { revision, lesson: lesson.summary.question, reader_path: '/?' + new URLSearchParams({ course_book: source.source_id, trial: revision }) };
  }
  if (!existsSync(artifactFile)) throw new Error('Trial revision not found');
  if (operation === 'state') return trialState(directory);
  const key = id.parse(item);
  if (operation === 'feedback') {
    storeOnce(path.join(directory, 'feedback', key), { 'note.json': feedbackSchema.parse(readJson(inputFile)) });
    return { revision, feedback: key };
  }
  const proposal = path.join(directory, 'routes', key);
  const proposalFile = path.join(proposal, 'proposal.json');
  const acceptedFile = path.join(proposal, 'accepted', 'accepted-route.json');
  if (operation === 'pause') {
    if (!existsSync(proposalFile) || existsSync(acceptedFile)) throw new Error('A pending proposal is required');
    storeOnce(path.join(proposal, 'paused'), { 'decision.json': { status: 'paused' } });
    return { revision, proposal: key, status: 'paused' };
  }
  if (operation !== 'propose' && operation !== 'accept') throw new Error('Unknown course-trial operation');
  const trial = trialSchema.parse(readJson(artifactFile));
  const sourceInput = openSource();
  const fields = operation === 'propose' ? readJson(inputFile) : readJson(proposalFile);
  const editableRoute = AcceptedCourseRouteZ.pick({ title: true, question: true, audience: true, final_task: true, unit_lids: true, example_lessons: true }).parse(fields);
  const route = AcceptedCourseRouteZ.parse({ ...editableRoute, source_id: trial.source_id, source_revision: trial.source_revision,
    trial_revision: revision, status: 'accepted' });
  scopeFromAcceptedRoute(sourceInput, route);
  if (operation === 'propose') {
    storeOnce(proposal, { 'proposal.json': editableRoute });
    return { revision, proposal: key, route: editableRoute };
  }
  storeOnce(path.dirname(acceptedFile), { 'accepted-route.json': route });
  return { revision, proposal: key, accepted_route_path: acceptedFile };
}

import { describe, expect, it } from 'vitest';
import { learningCollections, learningCollectionUrl, readLearningCollection, type LearningCollectionTarget } from './learning-collections';
import type { SharedBook, SharedRelease } from './shared-workspaces';

const first = { book_id: 'book', publication_id: 'edition-one' }, second = { book_id: 'book', publication_id: 'edition-two' }, other = { book_id: 'other-book', publication_id: 'published' };
function release(id: string, publications = [first]): SharedRelease {
  return { release_id: id, version: 1, latest_version: 1, archived: false, created_at: 1,
    readiness_at_publication: { status: 'ready', materials: [], audience: [] }, availability: { status: 'ready', materials: [], audience: [] },
    manifest: { name: id, public_goal: 'Understand the material', publisher: 'Author', applicability: '', prerequisites: '', materials: [], topic_links: [], suggested_route: [],
      inquiry_course: { language: 'en', units: [{ id: 'unit', title: 'Unit' }], routes: [], lessons: publications.map((publication, index) => ({ id: `lesson-${index}`, unit_id: 'unit', question: 'Why?', objective: 'Explain it', prerequisites: '', estimated_minutes: 20, duration_basis: 'Reading and practice', source: { publication, source_revision: '1', start_lid: '1', end_lid: '2', label: 'Source', structure_refs: [] } })) } } };
}
describe('learning collection addresses', () => {
  it.each<LearningCollectionTarget>([
    { kind: 'book', publication: first },
    { kind: 'book', publication: second, learningWorkspaceId: 'private scope' },
    { kind: 'course', releaseId: 'course/with spaces', version: 2, learningWorkspaceId: 'saved-scope' },
  ])('round trips an exact collection while preserving the reader and language context: %j', target => {
    const url = learningCollectionUrl('https://example.test/index.html?book_id=reader-book&publication_id=reader-version&lang=en&view=courses#position', target);
    expect(readLearningCollection(url.search)).toEqual(target);
    expect(url.searchParams.get('view')).toBe('learning');
    expect(url.searchParams.get('book_id')).toBe('reader-book');
    expect(url.searchParams.get('publication_id')).toBe('reader-version');
    expect(url.searchParams.get('lang')).toBe('en');expect(url.hash).toBe('#position');
  });
  it('replaces and clears only collection parameters', () => {
    const original = learningCollectionUrl('https://example.test/?book_id=reader', { kind: 'book', publication: first, learningWorkspaceId: 'old' });
    const next = learningCollectionUrl(original.href, { kind: 'course', releaseId: 'course', version: 4 });
    expect(next.searchParams.has('collection_book')).toBe(false);expect(next.searchParams.has('collection_workspace')).toBe(false);
    const home = learningCollectionUrl(next.href, null);expect(readLearningCollection(home.search)).toBeNull();
    expect([...home.searchParams.keys()]).toEqual(['book_id', 'view']);expect(home.searchParams.get('view')).toBe('learning');
  });
  it.each(['?collection_book=book', '?collection_release=course', '?collection_release=course&collection_version=0', '?collection_release=course&collection_version=1.5'])('ignores an incomplete collection address: %s', search => { expect(readLearningCollection(search)).toBeNull(); });
});
it('groups the same book across publications and routes while retaining exact lesson publications', () => {
  const books: SharedBook[] = [{published_book_ref:first,title:'Book'},{published_book_ref:second,title:'Book, revised'}];
  const routes = [release('basics'), release('advanced',[second]), release('versions-together',[first,second])];
  const grouped = learningCollections(books,routes);
  expect(grouped).toHaveLength(1);expect(grouped[0]).toMatchObject({kind:'book',bookId:'book',publications:books,releases:routes});
  expect(routes[1].manifest.inquiry_course!.lessons[0].source.publication).toEqual(second);
});
it('keeps a multi-book course as one whole course and includes readable books without courses', () => {
  const cross = release('cross-book',[first,other]);
  const books: SharedBook[] = [{published_book_ref:first,title:'Book'},{published_book_ref:other,title:'Other book'}];
  const grouped=learningCollections(books,[cross]);
  expect(grouped).toHaveLength(3);expect(grouped[0]).toMatchObject({kind:'book',releases:[]});expect(grouped[1]).toMatchObject({kind:'book',releases:[]});expect(grouped[2]).toEqual({kind:'course',release:cross});
});
it('does not turn video, music or empty lesson releases into book courses', () => {
  const video = {book_id:'video',publication_id:'v1'}, music = {book_id:'music',publication_id:'m1'};
  const grouped=learningCollections([{published_book_ref:video,media_kind:'video'},{published_book_ref:music,media_kind:'music'},{published_book_ref:first,title:'Book'}],[release('video',[video]),release('empty',[])]);
  expect(grouped).toHaveLength(1);expect(grouped[0]).toMatchObject({kind:'book',bookId:'book',releases:[]});
});

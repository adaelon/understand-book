import type { PublishedBookRef } from './reader-context';
import type { SharedBook, SharedRelease } from './shared-workspaces';

export type LearningCollectionTarget =
  | { kind: 'book'; publication: PublishedBookRef; learningWorkspaceId?: string }
  | { kind: 'course'; releaseId: string; version: number; learningWorkspaceId?: string };

const collectionKeys = ['collection_book', 'collection_publication', 'collection_release', 'collection_version', 'collection_workspace'];

export function readLearningCollection(search: string): LearningCollectionTarget | null {
  const params = new URLSearchParams(search);
  const learningWorkspaceId = params.get('collection_workspace') || undefined;
  const book = params.get('collection_book'), publication = params.get('collection_publication');
  if (book && publication) return { kind: 'book', publication: { book_id: book, publication_id: publication }, ...(learningWorkspaceId ? { learningWorkspaceId } : {}) };
  const releaseId = params.get('collection_release'), version = Number(params.get('collection_version'));
  return releaseId && Number.isInteger(version) && version > 0
    ? { kind: 'course', releaseId, version, ...(learningWorkspaceId ? { learningWorkspaceId } : {}) } : null;
}

export function learningCollectionUrl(href: string, target: LearningCollectionTarget | null): URL {
  const url = new URL(href);
  collectionKeys.forEach(key => url.searchParams.delete(key));
  url.searchParams.set('view', 'learning');
  if (target?.kind === 'book') {
    url.searchParams.set('collection_book', target.publication.book_id);
    url.searchParams.set('collection_publication', target.publication.publication_id);
  } else if (target?.kind === 'course') {
    url.searchParams.set('collection_release', target.releaseId);
    url.searchParams.set('collection_version', String(target.version));
  }
  if (target?.learningWorkspaceId) url.searchParams.set('collection_workspace', target.learningWorkspaceId);
  return url;
}

export type LearningCollection =
  | { kind: 'book'; bookId: string; book: SharedBook; publications: SharedBook[]; releases: SharedRelease[] }
  | { kind: 'course'; release: SharedRelease };

/** A collection groups what the current public page actually contains. It does not fetch hidden pages or private instances. */
export function learningCollections(books: SharedBook[], releases: SharedRelease[]): LearningCollection[] {
  const entries: LearningCollection[] = [];
  const byBook = new Map<string, Extract<LearningCollection, { kind: 'book' }>>();
  const addBook = (book: SharedBook) => {
    let entry = byBook.get(book.published_book_ref.book_id);
    if (!entry) {
      entry = { kind: 'book', bookId: book.published_book_ref.book_id, book, publications: [], releases: [] };
      byBook.set(entry.bookId, entry); entries.push(entry);
    }
    if (!entry.publications.some(item => item.published_book_ref.publication_id === book.published_book_ref.publication_id)) entry.publications.push(book);
    return entry;
  };
  for (const book of books) if (!book.media_kind || book.media_kind === 'book') addBook(book);
  for (const release of releases) {
    const lessons = release.manifest.inquiry_course?.lessons ?? [];
    if (!lessons.length) continue;
    const bookIds = new Set(lessons.map(lesson => lesson.source.publication.book_id));
    if (bookIds.size > 1) { entries.push({ kind: 'course', release }); continue; }
    const publication = lessons[0]!.source.publication;
    const known = books.find(book => book.published_book_ref.book_id === publication.book_id && book.published_book_ref.publication_id === publication.publication_id)
      ?? books.find(book => book.published_book_ref.book_id === publication.book_id);
    if (known?.media_kind && known.media_kind !== 'book') continue;
    const entry = addBook(known ?? { published_book_ref: publication });
    for (const lesson of lessons) addBook(books.find(book => book.published_book_ref.book_id === lesson.source.publication.book_id && book.published_book_ref.publication_id === lesson.source.publication.publication_id)
      ?? { ...known, published_book_ref: lesson.source.publication });
    entry.releases.push(release);
  }
  return entries;
}

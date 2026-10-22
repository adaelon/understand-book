/** Display names are independent of the ID used to open a book or its version. */
export function bookTitle(book: { title?: string | null; name?: string; book_id: string }): string {
  const title = book.title?.trim();
  if (title) return title;
  const name = book.name?.trim() || book.book_id;
  // Imported publications can use opaque UUIDs or content IDs as their identity.
  // They are useful for opening the material, but not as a reader-facing title.
  if (/^(?:[a-f\d]{8}(?:-[a-f\d]{4}){3}-[a-f\d]{12}|[a-f\d]{32,64})$/i.test(name)) return '未命名资料';
  return name.replace(/[-_]+/g, ' ');
}

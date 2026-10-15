/** Display names are independent of the ID used to open a book or its version. */
export function bookTitle(book: { title?: string | null; name?: string; book_id: string }): string {
  return book.title?.trim() || (book.name?.trim() || book.book_id).replace(/[-_]+/g, ' ');
}

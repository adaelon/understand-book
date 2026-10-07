CREATE TABLE book_defaults (
    book_id TEXT PRIMARY KEY NOT NULL,
    publication_id TEXT NOT NULL,
    FOREIGN KEY(book_id, publication_id) REFERENCES book_publications(book_id, publication_id)
);

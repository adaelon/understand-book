-- A committed online import and its grants share one transaction with its receipt.
CREATE TABLE publication_operations (
    operation_id TEXT PRIMARY KEY NOT NULL,
    parameters_json TEXT NOT NULL CHECK(json_valid(parameters_json)),
    book_id TEXT NOT NULL,
    publication_id TEXT NOT NULL,
    FOREIGN KEY(book_id, publication_id) REFERENCES book_publications(book_id, publication_id)
) STRICT;

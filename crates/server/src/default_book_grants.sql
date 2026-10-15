-- Explicit publication opt-in. Ordinary publications remain individually granted.
CREATE TABLE default_book_grants (
    book_id TEXT NOT NULL,
    publication_id TEXT NOT NULL,
    PRIMARY KEY(book_id, publication_id),
    FOREIGN KEY(book_id, publication_id) REFERENCES book_publications(book_id, publication_id)
);
-- Both verified registration and trusted account provisioning insert into users.
-- The grant participates in the same transaction as account creation.
CREATE TRIGGER grant_default_books_to_new_user AFTER INSERT ON users
BEGIN
    INSERT INTO book_grants(owner_user_id, book_id, publication_id)
    SELECT NEW.user_id, book_id, publication_id FROM default_book_grants;
END;

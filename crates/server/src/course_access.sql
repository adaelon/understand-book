-- A course's material license is tied to the sold account period, never copied to book_grants.
CREATE TABLE course_release_access (
    release_id TEXT NOT NULL,
    version INTEGER NOT NULL,
    allowed_tiers_json TEXT NOT NULL,
    PRIMARY KEY(release_id,version),
    FOREIGN KEY(release_id,version) REFERENCES workspace_releases(release_id,version)
);

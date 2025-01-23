-- sqlfluff:dialect:sqlite

CREATE TABLE urls (
    shorturl TEXT PRIMARY KEY COLLATE nocase,
    url TEXT NOT NULL
    CHECK (LENGTH(shorturl) >= 2)
    CHECK (LENGTH(shorturl) <= 16)
    CHECK (url LIKE 'https://%' OR url LIKE 'http://%')
) STRICT;

CREATE TABLE quotations (
    collection TEXT NOT NULL COLLATE nocase,
    quote TEXT NOT NULL
) STRICT;

CREATE UNIQUE INDEX collection_quote ON quotations (collection, quote);

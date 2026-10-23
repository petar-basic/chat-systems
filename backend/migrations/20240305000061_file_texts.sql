CREATE TYPE file_text_status AS ENUM (
    'pending', 'done', 'empty', 'unsupported', 'too_large', 'failed'
);

CREATE TABLE file_texts (
    file_id UUID PRIMARY KEY REFERENCES files (id) ON DELETE CASCADE,
    status file_text_status NOT NULL DEFAULT 'pending',
    content TEXT,
    search_vector TSVECTOR GENERATED ALWAYS AS (public.search_vector_of(content)) STORED,
    attempts INT NOT NULL DEFAULT 1,
    claimed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    extracted_at TIMESTAMPTZ
);

CREATE INDEX idx_file_texts_search ON file_texts USING GIN (search_vector);
CREATE INDEX idx_file_texts_pending ON file_texts (claimed_at) WHERE status = 'pending';

CREATE TEXT SEARCH CONFIGURATION public.search_headline (COPY = pg_catalog.simple);
ALTER TEXT SEARCH CONFIGURATION public.search_headline
    ALTER MAPPING FOR asciiword, word, numword, asciihword, hword, numhword,
                      hword_asciipart, hword_part, hword_numpart
    WITH public.unaccent, pg_catalog.simple;

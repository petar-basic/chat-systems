-- Phase 1 of the search rewrite. Every step here is chosen so the table is never
-- held under an ACCESS EXCLUSIVE lock: adding a nullable column is a catalog
-- change and the indexes are built concurrently. Replacing the generated column
-- in place would have rewritten `messages` under an exclusive lock instead,
-- which on a large instance is measured in minutes of downtime.

CREATE EXTENSION IF NOT EXISTS unaccent;
CREATE EXTENSION IF NOT EXISTS pg_trgm;

-- One function, called by both the trigger that writes the vector and the query
-- that reads it, so the stored vector and the query configuration cannot drift
-- apart. Changing the language is a migration that replaces this function and
-- rebuilds the vectors -- it is deliberately not an environment variable,
-- because a setting that has to agree with a stored column is a setting that
-- will eventually disagree with it.
CREATE OR REPLACE FUNCTION search_text_config() RETURNS regconfig
    LANGUAGE sql IMMUTABLE PARALLEL SAFE AS
$$ SELECT 'simple'::regconfig $$;

-- `unaccent(text)` is only STABLE because it resolves the default dictionary at
-- call time; naming the dictionary makes it immutable, which is what an index
-- and a generated value require.
CREATE OR REPLACE FUNCTION search_normalize(input TEXT) RETURNS TEXT
    LANGUAGE sql IMMUTABLE STRICT PARALLEL SAFE AS
$$ SELECT unaccent('unaccent'::regdictionary, input) $$;

CREATE OR REPLACE FUNCTION search_vector_of(input TEXT) RETURNS tsvector
    LANGUAGE sql IMMUTABLE PARALLEL SAFE AS
$$ SELECT to_tsvector(search_text_config(), search_normalize(COALESCE(input, ''))) $$;

ALTER TABLE messages ADD COLUMN IF NOT EXISTS search_vector TSVECTOR;
ALTER TABLE conversation_messages ADD COLUMN IF NOT EXISTS search_vector TSVECTOR;

CREATE OR REPLACE FUNCTION set_search_vector() RETURNS trigger
    LANGUAGE plpgsql AS
$$
BEGIN
    NEW.search_vector := search_vector_of(NEW.content);
    RETURN NEW;
END
$$;

DROP TRIGGER IF EXISTS messages_search_vector ON messages;
CREATE TRIGGER messages_search_vector
    BEFORE INSERT OR UPDATE OF content ON messages
    FOR EACH ROW EXECUTE FUNCTION set_search_vector();

DROP TRIGGER IF EXISTS conversation_messages_search_vector ON conversation_messages;
CREATE TRIGGER conversation_messages_search_vector
    BEFORE INSERT OR UPDATE OF content ON conversation_messages
    FOR EACH ROW EXECUTE FUNCTION set_search_vector();

-- The backfill is not here. A migration runs as one implicit transaction, so a
-- loop inside it could not commit between batches, and a single UPDATE over
-- every row would hold its locks and its snapshot for the whole run. The worker
-- does it in committed batches instead (`search::backfill`); until it finishes,
-- old messages are still found through the trigram index, so search degrades to
-- substring matching rather than going blank.

CREATE OR REPLACE FUNCTION public.search_text_config() RETURNS regconfig
    LANGUAGE sql IMMUTABLE PARALLEL SAFE AS
$$ SELECT 'pg_catalog.simple'::regconfig $$;

CREATE OR REPLACE FUNCTION public.search_normalize(input TEXT) RETURNS TEXT
    LANGUAGE sql IMMUTABLE STRICT PARALLEL SAFE AS
$$ SELECT public.unaccent('public.unaccent'::regdictionary, input) $$;

CREATE OR REPLACE FUNCTION public.search_vector_of(input TEXT) RETURNS tsvector
    LANGUAGE sql IMMUTABLE PARALLEL SAFE AS
$$ SELECT pg_catalog.to_tsvector(public.search_text_config(), public.search_normalize(COALESCE(input, ''))) $$;

CREATE OR REPLACE FUNCTION public.set_search_vector() RETURNS trigger
    LANGUAGE plpgsql AS
$$
BEGIN
    NEW.search_vector := public.search_vector_of(NEW.content);
    RETURN NEW;
END
$$;

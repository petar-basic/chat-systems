CREATE TABLE polls (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    message_id      UUID NOT NULL UNIQUE REFERENCES messages(id) ON DELETE CASCADE,
    question        TEXT NOT NULL,
    multiple_choice BOOLEAN NOT NULL DEFAULT FALSE,
    anonymous       BOOLEAN NOT NULL DEFAULT FALSE,
    closes_at       TIMESTAMPTZ,
    closed_at       TIMESTAMPTZ,
    created_by      UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_polls_due ON polls (closes_at) WHERE closed_at IS NULL AND closes_at IS NOT NULL;

CREATE TABLE poll_options (
    id       UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    poll_id  UUID NOT NULL REFERENCES polls(id) ON DELETE CASCADE,
    position SMALLINT NOT NULL,
    label    TEXT NOT NULL,
    UNIQUE (poll_id, position),
    UNIQUE (poll_id, id)
);

CREATE TABLE poll_votes (
    poll_id       UUID NOT NULL,
    option_id     UUID NOT NULL,
    user_id       UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    single_choice BOOLEAN NOT NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (option_id, user_id),
    FOREIGN KEY (poll_id, option_id) REFERENCES poll_options(poll_id, id) ON DELETE CASCADE
);

CREATE UNIQUE INDEX poll_votes_one_per_person ON poll_votes (poll_id, user_id) WHERE single_choice;
CREATE INDEX idx_poll_votes_poll ON poll_votes (poll_id, user_id);

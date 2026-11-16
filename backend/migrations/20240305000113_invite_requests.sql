CREATE TYPE invite_request_status AS ENUM ('pending', 'approved', 'declined', 'expired');

CREATE TABLE invite_requests (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    requested_by UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    emails TEXT[] NOT NULL,
    role workspace_role NOT NULL,
    guest_kind guest_kind,
    guest_until TIMESTAMPTZ,
    reason TEXT,
    status invite_request_status NOT NULL DEFAULT 'pending',
    decided_by UUID REFERENCES users(id) ON DELETE SET NULL,
    decided_at TIMESTAMPTZ,
    note TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT invite_requests_role CHECK (role IN ('member', 'guest')),
    CONSTRAINT invite_requests_emails CHECK (cardinality(emails) BETWEEN 1 AND 20),
    CONSTRAINT invite_requests_guest_kind CHECK ((role = 'guest') = (guest_kind IS NOT NULL)),
    CONSTRAINT invite_requests_guest_until CHECK (guest_until IS NULL OR role = 'guest'),
    CONSTRAINT invite_requests_decided CHECK ((status = 'pending') = (decided_at IS NULL))
);

CREATE INDEX invite_requests_pending ON invite_requests (workspace_id, created_at)
    WHERE status = 'pending';

CREATE TABLE invite_request_channels (
    request_id UUID NOT NULL REFERENCES invite_requests(id) ON DELETE CASCADE,
    channel_id UUID NOT NULL REFERENCES channels(id) ON DELETE CASCADE,
    PRIMARY KEY (request_id, channel_id)
);

CREATE INDEX invite_request_channels_channel ON invite_request_channels (channel_id);

ALTER TABLE workspace_invites
    ADD COLUMN requested_by UUID REFERENCES users(id) ON DELETE SET NULL;

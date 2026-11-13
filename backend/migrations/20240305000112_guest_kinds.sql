CREATE TYPE guest_kind AS ENUM ('single', 'multi');

ALTER TABLE workspace_members
    ADD COLUMN guest_kind guest_kind,
    ADD COLUMN guest_until TIMESTAMPTZ,
    ADD COLUMN guest_invited_by UUID REFERENCES users(id) ON DELETE SET NULL,
    ADD COLUMN guest_warned_at TIMESTAMPTZ;

UPDATE workspace_members SET guest_kind = 'multi' WHERE role = 'guest';

ALTER TABLE workspace_members
    ADD CONSTRAINT workspace_members_guest_kind CHECK ((role = 'guest') = (guest_kind IS NOT NULL)),
    ADD CONSTRAINT workspace_members_guest_until CHECK (guest_until IS NULL OR role = 'guest');

CREATE INDEX workspace_members_guest_until ON workspace_members (guest_until)
    WHERE guest_until IS NOT NULL;

ALTER TABLE workspace_invites
    ADD COLUMN guest_kind guest_kind,
    ADD COLUMN guest_until TIMESTAMPTZ;

UPDATE workspace_invites SET guest_kind = 'multi' WHERE role = 'guest';

ALTER TABLE workspace_invites
    ADD CONSTRAINT workspace_invites_guest_kind CHECK ((role = 'guest') = (guest_kind IS NOT NULL)),
    ADD CONSTRAINT workspace_invites_guest_until CHECK (guest_until IS NULL OR role = 'guest');

CREATE TABLE workspace_invite_channels (
    invite_id  UUID NOT NULL REFERENCES workspace_invites(id) ON DELETE CASCADE,
    channel_id UUID NOT NULL REFERENCES channels(id) ON DELETE CASCADE,
    PRIMARY KEY (invite_id, channel_id)
);

CREATE INDEX workspace_invite_channels_channel ON workspace_invite_channels (channel_id);

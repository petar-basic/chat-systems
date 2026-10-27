ALTER TABLE scheduled_messages
    ADD COLUMN thread_parent_id UUID,
    ADD COLUMN also_in_channel BOOLEAN NOT NULL DEFAULT FALSE;

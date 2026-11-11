ALTER TABLE command_invocations ADD COLUMN thread_parent_id UUID REFERENCES messages(id) ON DELETE SET NULL;

ALTER TABLE workflow_run_steps
    ADD COLUMN list_item_id UUID REFERENCES list_items(id) ON DELETE SET NULL;

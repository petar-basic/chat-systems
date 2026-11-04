ALTER TABLE users
    ADD COLUMN title    VARCHAR(100),
    ADD COLUMN pronouns VARCHAR(30);

ALTER TABLE workspace_members
    ADD COLUMN title    VARCHAR(100),
    ADD COLUMN pronouns VARCHAR(30);

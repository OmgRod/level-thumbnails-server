ALTER TABLE users ADD COLUMN discord_id BIGINT NULL;
ALTER TABLE users DROP CONSTRAINT users_username_key;
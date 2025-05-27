CREATE TABLE IF NOT EXISTS users
(
    id         BIGSERIAL PRIMARY KEY NOT NULL,
    account_id BIGINT                NOT NULL, -- Geometry Dash account ID
    username   TEXT                  NOT NULL UNIQUE,
    role       TEXT                  NOT NULL CHECK (role IN ('user', 'verified', 'moderator', 'admin')) DEFAULT 'user'
);

CREATE TABLE IF NOT EXISTS uploads
(
    id            BIGSERIAL PRIMARY KEY,
    user_id       BIGINT                 NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    level_id      BIGINT                 NOT NULL,
    upload_time   TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    accepted_time TIMESTAMP DEFAULT NULL,
    accepted_by   BIGINT    DEFAULT NULL REFERENCES users (id) ON DELETE SET NULL,
    image_path    TEXT                   NOT NULL,
    accepted      BOOLEAN   DEFAULT FALSE,
    reason        TEXT      DEFAULT NULL
);

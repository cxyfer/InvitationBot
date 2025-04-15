-- Create private_invite_usages table to track usage of the anonymous invite command
CREATE TABLE IF NOT EXISTS private_invite_usages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id TEXT NOT NULL,
    guild_id TEXT NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP NOT NULL
);
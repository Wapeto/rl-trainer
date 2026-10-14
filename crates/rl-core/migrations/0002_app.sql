-- Plan 2: single active profile, crash-recovery heartbeat, settings, read indexes.
ALTER TABLE profile ADD COLUMN is_active INTEGER NOT NULL DEFAULT 0;
UPDATE profile SET is_active = 1
 WHERE primary_id = (SELECT primary_id FROM profile ORDER BY created_at, primary_id LIMIT 1);
CREATE UNIQUE INDEX profile_one_active ON profile(is_active) WHERE is_active = 1;

ALTER TABLE session ADD COLUMN last_seen_at INTEGER;

CREATE TABLE setting (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE INDEX match_record_block ON match_record(block_id);
CREATE INDEX mmr_entry_team_time ON mmr_entry(team_size, recorded_at);

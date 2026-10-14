-- Matches deleted from the log are still in the game's match history: their start times keep
-- the places of older matches right (see HISTORY_PLACE).
CREATE TABLE deleted_match (
    id         INTEGER PRIMARY KEY,
    started_at INTEGER NOT NULL
);
CREATE INDEX deleted_match_started_at ON deleted_match (started_at);

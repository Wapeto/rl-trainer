-- Every player's replay stats in a match, for the match page. `stats` is JSON of
-- PlayerReplayStats (rl-core store::replays); a field added later reads as missing in older rows.
CREATE TABLE replay_player (
    match_id INTEGER NOT NULL REFERENCES match_record(id) ON DELETE CASCADE,
    slot     INTEGER NOT NULL,
    team     INTEGER NOT NULL CHECK (team IN (0, 1)),
    name     TEXT NOT NULL,
    is_me    INTEGER NOT NULL,
    is_bot   INTEGER NOT NULL,
    stats    TEXT NOT NULL,
    PRIMARY KEY (match_id, slot)
);

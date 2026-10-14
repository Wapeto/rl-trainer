-- Replay files (Plan 6): every file looked at, and the coach's stats of each matched match.
CREATE TABLE replay_file (
    id          INTEGER PRIMARY KEY,
    path        TEXT NOT NULL UNIQUE,
    modified_at INTEGER NOT NULL,
    size        INTEGER NOT NULL,
    match_guid  TEXT,
    status      TEXT NOT NULL CHECK (status IN ('imported', 'unmatched', 'not_mine', 'error')),
    error       TEXT,
    imported_at INTEGER NOT NULL
);

CREATE TABLE replay_stats (
    match_id             INTEGER PRIMARY KEY REFERENCES match_record(id) ON DELETE CASCADE,
    imported_at          INTEGER NOT NULL,
    parser_version       TEXT NOT NULL,
    tracked_seconds      REAL NOT NULL,
    pct_behind_ball      REAL NOT NULL,
    pct_in_front_of_ball REAL NOT NULL,
    pct_defensive_third  REAL NOT NULL,
    pct_neutral_third    REAL NOT NULL,
    pct_offensive_third  REAL NOT NULL,
    secs_first_man       REAL,
    secs_second_man      REAL,
    secs_third_man       REAL,
    became_first_man     INTEGER,
    big_pads             INTEGER NOT NULL,
    small_pads           INTEGER NOT NULL,
    pads_stolen          INTEGER NOT NULL,
    touches              INTEGER NOT NULL,
    touches_low_boost    INTEGER NOT NULL,
    touches_zero_boost   INTEGER NOT NULL,
    fifty_wins           INTEGER NOT NULL,
    fifty_losses         INTEGER NOT NULL,
    fifty_low_boost      INTEGER NOT NULL,
    kickoff_wins         INTEGER NOT NULL,
    kickoff_losses       INTEGER NOT NULL,
    whiffs               INTEGER NOT NULL
);

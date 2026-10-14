CREATE TABLE profile (
    primary_id   TEXT PRIMARY KEY,
    display_name TEXT NOT NULL,
    created_at   INTEGER NOT NULL
);

CREATE TABLE session (
    id         INTEGER PRIMARY KEY,
    started_at INTEGER NOT NULL,
    ended_at   INTEGER,
    notes      TEXT NOT NULL DEFAULT ''
);

CREATE TABLE pack (
    id          INTEGER PRIMARY KEY,
    code        TEXT NOT NULL UNIQUE,
    name        TEXT NOT NULL,
    creator     TEXT NOT NULL DEFAULT '',
    tags        TEXT NOT NULL DEFAULT '[]',
    shot_count  INTEGER,
    target_rate REAL NOT NULL DEFAULT 0.65 CHECK (target_rate > 0 AND target_rate <= 1),
    notes       TEXT NOT NULL DEFAULT ''
);

CREATE TABLE block (
    id               INTEGER PRIMARY KEY,
    session_id       INTEGER NOT NULL REFERENCES session(id) ON DELETE CASCADE,
    kind             TEXT NOT NULL CHECK (kind IN ('freeplay', 'training', 'match', 'other')),
    playlist_id      INTEGER,
    pack_id          INTEGER REFERENCES pack(id) ON DELETE SET NULL,
    started_at       INTEGER NOT NULL,
    ended_at         INTEGER,
    data_gap_seconds INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX block_session ON block(session_id);

CREATE TABLE training_attempt (
    id               INTEGER PRIMARY KEY,
    block_id         INTEGER NOT NULL REFERENCES block(id) ON DELETE CASCADE,
    seq              INTEGER NOT NULL,
    started_at       INTEGER NOT NULL,
    duration_ms      INTEGER NOT NULL,
    outcome          TEXT NOT NULL CHECK (outcome IN ('make', 'miss', 'abandoned')),
    retry_after_make INTEGER NOT NULL DEFAULT 0,
    skipped          INTEGER NOT NULL DEFAULT 0,
    goal_speed       REAL,
    UNIQUE (block_id, seq)
);

CREATE TABLE freeplay_stats (
    block_id       INTEGER PRIMARY KEY REFERENCES block(id) ON DELETE CASCADE,
    touches        INTEGER NOT NULL,
    hard_hits      INTEGER NOT NULL,
    avg_hit_speed  REAL NOT NULL,
    max_hit_speed  REAL NOT NULL,
    goals          INTEGER NOT NULL,
    crossbar_hits  INTEGER NOT NULL,
    pct_supersonic REAL NOT NULL,
    pct_on_wall    REAL NOT NULL,
    pct_airborne   REAL NOT NULL
);

CREATE TABLE match_record (
    id              INTEGER PRIMARY KEY,
    block_id        INTEGER NOT NULL REFERENCES block(id) ON DELETE CASCADE,
    match_guid      TEXT NOT NULL UNIQUE,
    playlist_id     INTEGER,
    ranked          INTEGER NOT NULL,
    team_size       INTEGER,
    arena           TEXT NOT NULL,
    result          TEXT NOT NULL CHECK (result IN ('win', 'loss', 'unknown')),
    team_score      INTEGER NOT NULL,
    opponent_score  INTEGER NOT NULL,
    overtime        INTEGER NOT NULL,
    score           INTEGER NOT NULL,
    goals           INTEGER NOT NULL,
    assists         INTEGER NOT NULL,
    saves           INTEGER NOT NULL,
    shots           INTEGER NOT NULL,
    touches         INTEGER NOT NULL,
    demos           INTEGER NOT NULL,
    epic_saves      INTEGER NOT NULL,
    avg_boost       REAL NOT NULL,
    pct_zero_boost  REAL NOT NULL,
    pct_supersonic  REAL NOT NULL,
    metrics_partial INTEGER NOT NULL,
    scoreboard_json TEXT NOT NULL
);

CREATE TABLE mmr_entry (
    id          INTEGER PRIMARY KEY,
    team_size   INTEGER NOT NULL CHECK (team_size IN (1, 2, 3)),
    mmr         INTEGER NOT NULL CHECK (mmr BETWEEN 0 AND 3000),
    recorded_at INTEGER NOT NULL,
    session_id  INTEGER REFERENCES session(id) ON DELETE SET NULL
);

INSERT INTO pack (code, name, creator, tags) VALUES
    ('5CB2-6D82-1B54-47B7', 'Uncomfortable Saves', '', '["defence","saves"]'),
    ('5CCE-FB29-7B05-A0B1', 'Shadow Defence', 'SunlessKhan', '["defence","shadow"]'),
    ('9274-D4A2-967B-0D7A', 'Wall Clears', 'jstn', '["defence","wall"]'),
    ('07E1-81BC-DD2E-BF8C', 'Backboard Reads', 'Rizzo', '["defence","backboard"]'),
    ('F1D5-36FA-257C-80A8', 'Backboard Defence', 'WayProtein', '["defence","backboard"]'),
    ('55C9-36FE-613D-7F12', 'Consistency', 'Biddles', '["shooting"]'),
    ('7E96-B9C7-3AC0-9B70', 'Progression', 'Biddles', '["shooting"]'),
    ('5BFE-60D6-0D59-79F2', 'Aerial Off Wall', '', '["aerial","wall"]'),
    ('9F6D-4387-4C57-2E4B', 'Wall Shots', '', '["shooting","wall"]'),
    ('0E07-7E2F-EBE6-1346', 'Closer', 'Lethamyr', '["closer"]');

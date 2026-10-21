-- v0.3.0: where a match's result came from, and sessions made from replay files.
-- A replay's header has the final score, so it overrides the live feed's (a forfeit or a
-- quit looked like a loss or an unknown result); a result set by hand is never touched.
ALTER TABLE match_record ADD COLUMN result_source TEXT NOT NULL DEFAULT 'live'
    CHECK (result_source IN ('live', 'replay', 'manual'));
-- 'replay' sessions were built from replay files ("Import past games").
ALTER TABLE session ADD COLUMN source TEXT NOT NULL DEFAULT 'live'
    CHECK (source IN ('live', 'replay'));

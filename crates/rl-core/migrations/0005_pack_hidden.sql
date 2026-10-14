-- Plan 4: a hidden pack leaves the phone's picker and suggestions but keeps its history.
ALTER TABLE pack ADD COLUMN hidden INTEGER NOT NULL DEFAULT 0;

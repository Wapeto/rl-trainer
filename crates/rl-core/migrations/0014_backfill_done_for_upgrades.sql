-- A log from before 0.3 already has games (or had them, then deleted): skip the automatic
-- "Import past games", which could bring back a match deleted before 0013 kept its GUID.
-- The import stays one click away in Settings.
INSERT OR IGNORE INTO setting (key, value)
SELECT 'backfill_done', 'true'
WHERE EXISTS (SELECT 1 FROM match_record) OR EXISTS (SELECT 1 FROM deleted_match);

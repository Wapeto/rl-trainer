-- A deleted match keeps its GUID, so "Import past games" does not bring it back from its replay.
ALTER TABLE deleted_match ADD COLUMN match_guid TEXT;
CREATE INDEX deleted_match_guid ON deleted_match (match_guid);

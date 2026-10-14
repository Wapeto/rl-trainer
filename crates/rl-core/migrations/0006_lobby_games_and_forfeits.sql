-- Plan 4 follow-up (2026-10-09).
-- 1. Between two games of a private lobby, the next game's id showed for a few seconds over the
--    last scoreboard. That short block took the next game's record (same id), leaving the real
--    game with none. Move the record to the game that follows and drop the short block.
CREATE TEMP TABLE between_games AS
SELECT b.id AS short_id,
       (SELECT n.id FROM block n
        WHERE n.session_id = b.session_id AND n.kind = 'match'
          AND n.started_at >= b.ended_at AND n.started_at - b.ended_at < 15000
          AND NOT EXISTS (SELECT 1 FROM match_record x WHERE x.block_id = n.id)
        ORDER BY n.started_at, n.id LIMIT 1) AS game_id
FROM block b
WHERE b.kind = 'match' AND b.ended_at IS NOT NULL AND b.ended_at - b.started_at < 15000
  AND EXISTS (SELECT 1 FROM match_record m WHERE m.block_id = b.id);
DELETE FROM between_games WHERE game_id IS NULL;
UPDATE match_record
SET block_id = (SELECT game_id FROM between_games WHERE short_id = match_record.block_id),
    metrics_partial = 1
WHERE block_id IN (SELECT short_id FROM between_games);
DELETE FROM block WHERE id IN (SELECT short_id FROM between_games);
DROP TABLE between_games;
-- 2. A match with no reported winner (a forfeit) goes to the team that was ahead.
UPDATE match_record
SET result = CASE WHEN team_score > opponent_score THEN 'win' ELSE 'loss' END
WHERE result = 'unknown' AND team_score <> opponent_score;

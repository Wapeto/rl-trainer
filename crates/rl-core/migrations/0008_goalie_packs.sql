-- Goalie packs (2026-10-09): a shot is made unless the ball goes in. Every existing pack is a
-- striker pack until the player says otherwise.
ALTER TABLE pack ADD COLUMN goalie INTEGER NOT NULL DEFAULT 0;

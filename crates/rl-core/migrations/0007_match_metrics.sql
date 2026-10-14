-- Plan 5 (2026-10-09): my own play per match, from the live feed. NULL = not recorded
-- (every match before Plan 5); readers never treat NULL as 0.
ALTER TABLE match_record ADD COLUMN play_seconds REAL;
ALTER TABLE match_record ADD COLUMN hit_avg_speed REAL;
ALTER TABLE match_record ADD COLUMN hard_hits INTEGER;
ALTER TABLE match_record ADD COLUMN aerial_touches INTEGER;
ALTER TABLE match_record ADD COLUMN touches_def INTEGER;
ALTER TABLE match_record ADD COLUMN touches_mid INTEGER;
ALTER TABLE match_record ADD COLUMN touches_att INTEGER;
ALTER TABLE match_record ADD COLUMN pct_on_ground REAL;
ALTER TABLE match_record ADD COLUMN pct_on_wall REAL;
ALTER TABLE match_record ADD COLUMN pct_airborne REAL;
ALTER TABLE match_record ADD COLUMN avg_speed REAL;
ALTER TABLE match_record ADD COLUMN pct_boosting REAL;
ALTER TABLE match_record ADD COLUMN pct_full_boost REAL;
ALTER TABLE match_record ADD COLUMN boost_used REAL;
ALTER TABLE match_record ADD COLUMN big_pads INTEGER;
ALTER TABLE match_record ADD COLUMN goal_speed_avg REAL;
ALTER TABLE match_record ADD COLUMN demos_taken INTEGER;
ALTER TABLE match_record ADD COLUMN pct_possession REAL;

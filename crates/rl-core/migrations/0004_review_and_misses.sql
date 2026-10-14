-- Plan 3: the end-of-session review, and the make/miss rule for training attempts.
ALTER TABLE session ADD COLUMN reviewed_at INTEGER;
-- History does not ask for a review; a session still open (crash) is reviewed after recovery.
UPDATE session SET reviewed_at = ended_at WHERE ended_at IS NOT NULL;
-- "abandoned" (reset before the timer ran out) is now a miss. The CHECK constraint from 0001
-- still lists 'abandoned'; SQLite cannot alter it and the code no longer writes it.
UPDATE training_attempt SET outcome = 'miss' WHERE outcome = 'abandoned';

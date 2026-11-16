-- Reversal of 0012: back to 0011's exact schema. Written first. Data lost on reversal: the learned junk table, which accounts ran their
-- first-run training, the owner's per-message junk lessons, and each arrival's junk score and state (the messages themselves stay).
DROP TABLE IF EXISTS junk_learned;
DROP TABLE IF EXISTS junk_bootstrap;
DROP TABLE IF EXISTS junk_model;
ALTER TABLE message DROP COLUMN junk_state;
ALTER TABLE message DROP COLUMN junk_score;

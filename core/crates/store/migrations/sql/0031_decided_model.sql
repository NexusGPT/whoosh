-- 0031 (lane fix-aisort round 8; the lead's ruling 2026-11-12: additive only, no table rebuild): which on-device model judged. Apple's
-- on-device language model judges the Labels AI step; its filings and its kept "no" keep `decided_by = 'laya'` (the on-device decider the
-- CHECKs already allow) and name the model here: 'apple'. NULL for every existing row and for every other decider (Laya stays NULL).
--   label_assignment.decided_model   'apple' when Apple's model filed it; NULL otherwise
--   ai_label_no.decided_model        'apple' when Apple's model said no; NULL otherwise
-- Reading: decided_by 'laya' with decided_model 'apple' is contracts `DecidedBy::Apple`; with NULL it stays `DecidedBy::Laya`.
-- Reversal: 0031_decided_model.down.sql (drops the two columns: an Apple filing then reads as Laya's).
ALTER TABLE label_assignment ADD COLUMN decided_model TEXT;
ALTER TABLE ai_label_no ADD COLUMN decided_model TEXT;

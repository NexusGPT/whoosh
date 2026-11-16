-- 0017 (contracts delta 23, addition): the owner's stored consent to send the label AI's mail text to a HOSTED decision model (contracts decision_consent.rs).
-- One row per provider that can hold a consent (only 'jev' today). The default is NOT GIVEN: no row = no consent; withdrawing deletes the row. A table and not a
-- column on `decision_model` because that row (id = 1) does not exist until the first SaveDecisionModel, and the owner gives consent BEFORE saving a hosted model.
-- Additive; every existing row is kept. Reversal: 0017_decision_consent.down.sql.
CREATE TABLE decision_consent (
  provider TEXT PRIMARY KEY NOT NULL CHECK (provider IN ('jev')),
  given_at_ms INTEGER NOT NULL
);

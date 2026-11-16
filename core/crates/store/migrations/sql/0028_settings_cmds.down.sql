-- Reversal of 0028: back to 0027's exact schema. DATA LOST: the notch's list choice, every account's delivery choice, every given mail consent.
ALTER TABLE ai_provider DROP COLUMN mail_consent_at_ms;
ALTER TABLE notification_setting DROP COLUMN delivery;
ALTER TABLE notch_setting DROP COLUMN list_shows;

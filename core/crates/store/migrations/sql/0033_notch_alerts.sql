-- 0033: the notch's alert settings (owner's picks of 14 Nov 2026, docs/surfaces/mac-notch.md "1B 2A 3A"): one style per kind of mail, how long an
-- alert stays, sound, and whether the notch's full mail loads its pictures. One row (id 1), written whole by Command::SetNotchAlerts; before the first
-- save the engine serves contracts::notch_alerts_default(). Additive: no existing table or row changes. Reversal: 0033_notch_alerts.down.sql.
-- The column of the Primary kind is primary_mail (PRIMARY is an SQL word).
CREATE TABLE notch_alerts (
  id INTEGER PRIMARY KEY NOT NULL CHECK (id = 1),
  vip TEXT NOT NULL CHECK (vip IN ('nothing','light','line','card','opened')),
  needs_reply TEXT NOT NULL CHECK (needs_reply IN ('nothing','light','line','card','opened')),
  primary_mail TEXT NOT NULL CHECK (primary_mail IN ('nothing','light','line','card','opened')),
  updates TEXT NOT NULL CHECK (updates IN ('nothing','light','line','card','opened')),
  promotions TEXT NOT NULL CHECK (promotions IN ('nothing','light','line','card','opened')),
  sign_in_codes TEXT NOT NULL CHECK (sign_in_codes IN ('nothing','light','line','card','opened')),
  stay TEXT NOT NULL CHECK (stay IN ('5s','10s','until-acted')),
  sound INTEGER NOT NULL CHECK (sound IN (0,1)),
  full_mail_loads_pictures INTEGER NOT NULL CHECK (full_mail_loads_pictures IN (0,1))
);

-- 0028 (lane real-settings): the settings the real engine keeps that 0001's tables had no column for. ADDITIVE, COLUMNS ONLY (no table is added, so the
-- table lists of the older migration tests stay exact).
--   notch_setting.list_shows           what the notch's short list holds: 'latest' | 'unread' (contracts NotchSetting.list_shows)
--   notification_setting.delivery      where an arriving mail announces: 'notch' | 'notifications' | 'both' (contracts NotificationSetting.delivery; the default is both)
--   ai_provider.mail_consent_at_ms     the owner's "Allow" on the consent question for this provider (GiveAgentConsent); NULL = not given
-- Reversal: 0028_settings_cmds.down.sql.
ALTER TABLE notch_setting ADD COLUMN list_shows TEXT NOT NULL DEFAULT 'latest' CHECK (list_shows IN ('latest','unread'));
ALTER TABLE notification_setting ADD COLUMN delivery TEXT NOT NULL DEFAULT 'both' CHECK (delivery IN ('notch','notifications','both'));
ALTER TABLE ai_provider ADD COLUMN mail_consent_at_ms INTEGER;

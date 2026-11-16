-- Reversal of 0013: back to 0012's exact schema. Written first. DATA LOST on reversal: every label's "AI reads the rest of the thread" tick, every AI-replies setting (the two uses' switches, the filter and its
-- ticked labels, the on-demand switches, retrieval settings, the voice text and tones, every prompt version and fixed note, every source's switch and count),
-- every AI draft, and every pass (so the counts of the last 7 days restart from zero). No mail, label, account or owner draft is touched.
-- reply_pass first (it points at reply_draft), reply_use before reply_tone and reply_prompt (its triggers read them).
DROP TABLE IF EXISTS reply_pass;
DROP TABLE IF EXISTS reply_draft;
DROP TABLE IF EXISTS fixed_note;
DROP TABLE IF EXISTS reply_voice;
DROP TABLE IF EXISTS retrieval_settings;
DROP TABLE IF EXISTS reply_source;
DROP TABLE IF EXISTS reply_on_demand;
DROP TABLE IF EXISTS reply_filter_label;
DROP TABLE IF EXISTS reply_filter;
DROP TABLE IF EXISTS reply_use;
DROP TABLE IF EXISTS reply_tone;
DROP TABLE IF EXISTS reply_prompt;
ALTER TABLE label DROP COLUMN ai_reads_thread;

-- Reversal of 0011: back to 0010's exact schema objects. Written first. Data lost on reversal: the server write-backs not yet taken
-- (the local changes stay; the servers keep their old state until the owner repeats the action).
DROP INDEX IF EXISTS pending_action_due;
DROP TABLE IF EXISTS pending_action;

#!/usr/bin/env python3
"""par-actions: the GotMail / Superhuman parity rows send again, mark all read, empty Trash and Spam, mute, star, trash, spam,
List-Unsubscribe one-click (research/gotmail.md:33,61,108; research/superhuman.md:44), each judged by its own named cargo tests.

    python3 checks/parity/par-actions.py            run; exit 1 when F > 0 or U > 0
    python3 checks/parity/par-actions.py --selftest every check must go RED on its mutant

Shares the harness of checks/core-actions-check.py (same cargo runner, same mutant restore)."""
import importlib.util
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
spec = importlib.util.spec_from_file_location("core_actions_check", os.path.join(os.path.dirname(HERE), "core-actions-check.py"))
base = importlib.util.module_from_spec(spec)
spec.loader.exec_module(base)

base.CHECKS.update({
    "send-again": ("send again: a sent message becomes a draft with the same recipients, subject, text and attachments", ["send_again_makes_a_draft_with_the_same_recipients_subject_and_text", "send_again_refuses_a_message_the_owner_did_not_send"]),
    "mark-all-read": ("mark all read: every unread row of the view, counts go to zero", ["mark_all_read_marks_every_unread_row_of_the_view"]),
    "empty-trash-and-spam": ("empty Trash and Spam: deleted after the server agrees, a refusing account keeps its mail", ["empty_trash_and_spam_deletes_after_the_server_agrees_and_only_for_that_account"]),
    "mute": ("mute: the whole thread is labelled and leaves the inbox, a refusal changes nothing", ["mute_labels_and_archives_the_whole_thread_and_unmute_lifts_only_the_label", "a_refused_mute_leaves_the_thread_in_the_inbox_with_no_label"]),
    "star": ("star: the system label, no undo entry", ["star_adds_the_system_label_and_unstar_removes_it_without_an_undo_entry"]),
    "trash": ("trash: out of the inbox into Trash, a refusal returns it", ["trash_and_spam_move_the_message_out_of_the_inbox_and_a_refusal_returns_it"]),
    "spam": ("spam: out of the inbox into Spam", ["trash_and_spam_move_the_message_out_of_the_inbox_and_a_refusal_returns_it"]),
    "list-unsubscribe": ("List-Unsubscribe one-click: server first, nothing changes on a refusal, none without the header", ["unsubscribe_asks_the_server_first_and_changes_nothing_on_a_refusal", "unsubscribe_on_a_message_without_a_list_header_is_refused_in_words"]),
})
base.MUTANTS.update({
    "send-again": ("usecases/send_again.rs", "ctx.draft_from_sent(&message_id, &new_id(\"draft\"), now)", "ctx.draft_from_sent(&new_id(\"draft\"), &new_id(\"draft\"), now)", "the draft is built from no sent message, so it has none of its recipients, subject or text"),
    "mark-all-read": ("usecases/mark_all_read.rs", ".execute(unread, true)", ".execute(unread, false)", "marks the unread rows unread instead of read"),
    "empty-trash-and-spam": ("usecases/empty_trash_and_spam.rs", "ctx.delete_folder_messages(&account, &[TRASH_FOLDER, SPAM_FOLDER])", "ctx.delete_folder_messages(&account, &[TRASH_FOLDER, TRASH_FOLDER])", "Spam is not emptied"),
    "mute": ("usecases/mute.rs", "let members = ctx.thread_member_ids(&message_ids)?;", "let members = message_ids.clone();", "only the named message is muted, not the thread"),
    "star": ("usecases/star.rs", "assign(ctx, &rows, STARRED_LABEL, true, now, false)", "assign(ctx, &rows, STARRED_LABEL, true, now, true)", "star pushes an undo entry"),
    "trash": ("usecases/trash.rs", "move_to_folder(ctx, &message_ids, crate::constants::TRASH_FOLDER)", "move_to_folder(ctx, &message_ids, crate::constants::SPAM_FOLDER)", "trash files into Spam"),
    "spam": ("usecases/spam.rs", "move_to_folder(ctx, &message_ids, crate::constants::SPAM_FOLDER)", "move_to_folder(ctx, &message_ids, crate::constants::TRASH_FOLDER)", "spam files into Trash"),
    "list-unsubscribe": ("usecases/unsubscribe.rs", ".map_err(|r| self.env.failure(&r))?;\n        local_only", ".ok();\n        local_only", "a refused unsubscribe still archives and labels"),
})

PARITY = ["send-again", "mark-all-read", "empty-trash-and-spam", "mute", "star", "trash", "spam", "list-unsubscribe"]


def main():
    if "--selftest" in sys.argv:
        return base.report(base.selftest(PARITY))
    return base.report(base.run_group(PARITY))


if __name__ == "__main__":
    sys.exit(main())

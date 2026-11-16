#!/usr/bin/env python3
"""The real core, slice 1: the installed-shape app finds a Microsoft 365 tenant (owner@m365-check.test, answered by the LOCAL network fake) WITHOUT the fixture core.

    python3 checks/realcore-check.py <bundle>            launch the bundle hidden (no --fixture-core), drive Add account, judge
    python3 checks/realcore-check.py <bundle> --mutant   the same, with WHOOSH_CORE_MUTANT=no-microsoft (Autodiscover refused,
                                                          no mail record): every Microsoft check must go red (exit 0 when
                                                          they did)
    python3 checks/realcore-check.py <bundle> --slice2   slice 2: a local IMAP and SMTP server (checks/realcore_mailfake.py,
                                                          clear text on 127.0.0.1) holds 3 inbox messages; the sheet adds
                                                          its account; mac-main.list must show them and the counts be > 0
    python3 checks/realcore-check.py <bundle> --slice2 --mutant   the same with WHOOSH_CORE_MUTANT=sync-never-stores: the
                                                          list and counts checks must go red (exit 0 when they did)
    python3 checks/realcore-check.py <bundle> --slice2b  slice 2b: the browser sign-in. The local IMAP/SMTP server accepts XOAUTH2
                                                          and a local authority (checks/realcore_authfake.py) plays Microsoft;
                                                          the sheet finds owner@whoosh-check.invalid as a Microsoft 365 account,
                                                          Sign in, the core signs in at `common` (the multi-tenant default) by
                                                          PKCE and the loopback (playing the
                                                          browser itself: no window), adds it by XOAUTH2, the list shows 3 rows
    python3 checks/realcore-check.py <bundle> --slice2b --mutant[=<name>]   the same with WHOOSH_CORE_MUTANT=<name> (default
                                                          oauth-never-exchanges): the sign-in checks must go red (exit 0 when
                                                          they did). Names: oauth-never-waiting oauth-never-exchanges
                                                          oauth-refresh-not-stored oauth-common-tenant oauth-no-xoauth2-imap
                                                          oauth-add-refuses
    python3 checks/realcore-check.py <bundle> --slice3   slice 3 (delta 17): the labelling order on mail the sync stores. The local server holds
                                                          3 inbox messages (one whose subject names a contract, one nothing matches,
                                                          one that follows a message the owner wrote, in Sent, EARLIER in its thread:
                                                          "already answered by me" = I wrote earlier) and the rig copy seeds two labels
                                                          (WHOOSH_CORE_CHECK_FLOW=1): the list's chips and `core state message/<id>/filing`
                                                          must show the contract mail filed by its entry rule, the Sent message, stored after
                                                          the inbox message, flipping it into its label, and the rest left in Primary with "no provider"
    python3 checks/realcore-check.py <bundle> --slice3 --mutant   the same with WHOOSH_CORE_MUTANT=flow-never-files: the two filing
                                                          checks must go red (exit 0 when they did)
    python3 checks/realcore-check.py <bundle> --slice3b  slice 3b (delta 17, by meaning): the same rig core with one AI provider armed
                                                          (WHOOSH_CORE_CHECK_AI=<port>: the providers table's LM Studio at a local
                                                          fake, checks/realcore_aifake.py, a dummy key in a memory keychain). The
                                                          local server holds two inbox messages (the contract mail an entry rule
                                                          catches, and one whose subject no rule catches but whose body names what
                                                          the Renewals label means; A140: Contracts has its AI card off, Renewals is the AI-on label). `core state message/<id>/filing` and `.../why/<label>`
                                                          must show the second filed by the AI card, `laya` deciding, and the fake
                                                          must have seen ONE request carrying the label's meaning, the subject and
                                                          the body, no attachments, and the dummy key as a bearer
    python3 checks/realcore-check.py <bundle> --slice3b --mutant   the same with WHOOSH_CORE_MUTANT=flow-never-asks-ai: the core arms no
                                                          provider; the by-meaning checks must go red (exit 0 when they did)
    python3 checks/realcore-check.py <bundle> --slice4   slice 4 (delta 18): the mail import. A fixture Apple Mail tree of three messages stands in
                                                          for the home folder (WHOOSH_CORE_CHECK_IMPORT=<folder>, rig only); the rig core scans it,
                                                          imports it into Imported and scans again: `core state import/sources` must show the source
                                                          readable with 3 messages and 3 imported, and the list must hold the three subjects
    python3 checks/realcore-check.py <bundle> --slice4 --mutant   the same with WHOOSH_CORE_MUTANT=import-never-stores: the stored-count and
                                                          list checks must go red (exit 0 when they did)
    python3 checks/realcore-check.py <bundle> --slice-bye   engine-sync-m365: the local server ends the first FETCH after one
                                                          message with "* BYE Connection closed. 14" (Microsoft 365's words)
                                                          and closes the connection; the core must reconnect, the list show
                                                          all 3 messages and no account read as failing
    python3 checks/realcore-check.py <bundle> --slice-bye --refuse   the local server answers every UID SEARCH with
                                                          "BAD Command Argument Error. 11": the status line must name the
                                                          account's failed step and the server's words, the list stay empty
    python3 checks/realcore-check.py <bundle> --slice5   engine-act: the real core acts on mail. The local server (act mode: Archive and
                                                         Junk folders, MOVE, every write recorded) serves three inbox messages; the rig
                                                         copy archives the top one (E), snoozes the next (H, tomorrow morning), stars the
                                                         last (S), replies to it through the quick reply (R, Cmd-Enter) and lets the undo
                                                         window end, then replies again and undoes inside the window (Z). Judged on what
                                                         the IMAP and SMTP sides RECEIVED and on the list and thread the app shows.
                                                         engine-act-r4: after the replies, E-archived mail is brought back (G E, then ⇧E on the Archive
                                                         view) and the last inbox row is marked junk (⇧!); the server's BOXES are read back: the
                                                         unarchived message is in INBOX again, the junk one in Junk.
                                                         engine-act-r6: the Archive view must LIST the archived message (judged, from the local store,
                                                         no sync wait), and the junk one is brought back: G ⇧! opens the Junk view, the palette's Not
                                                         spam (`click mac-shortcuts.action.not-spam`) sends MarkNotSpam; the server's boxes show it in
                                                         INBOX again and the fake recorded the MOVE from Junk.
    python3 checks/realcore-check.py <bundle> --slice5 --mutant   the same with WHOOSH_CORE_MUTANT=act-never-writes-back: the
                                                         archive, the star, the junk move and the Not spam move never reach the server and must be seen red.
    python3 checks/realcore-check.py <bundle> --slice6   junk-r2: the local spam filter. The local server (act mode) holds a first-run
                                                         corpus (ordinary mail in INBOX, junk in Junk); once the rig copy holds IDLE, a
                                                         phish (SPF, DKIM and DMARC fail, a display name carrying a bank's address) and
                                                         a plain message arrive. Judged on the MOVE the server received and the list.
    python3 checks/realcore-check.py <bundle> --slice6 --mutant   the same with WHOOSH_CORE_MUTANT=junk-never-moves: the phish
                                                         stays in INBOX and in the list and must be seen red.
    python3 checks/realcore-check.py <bundle> --slice-search   lane search-checks (SPEC A141 to A143): the REAL engine answers a search, before and after a restart. The rig copy
                                                         (WHOOSH_CORE_CHECK_SEARCH=1) stores the engine's seeded mail pair (search-check:consultants holds only "consultants",
                                                         search-check:advisor is near it in meaning) beside the local server's three decoys and one mail the owner sent to Fay.
                                                         The check types in the real search field and reads the engine's own state (`search/last/*`, `people/*`): "consultant" puts
                                                         the consultants mail in the exact group; the advisor mail ranks first in the meaning group when a model answers (NOT JUDGED
                                                         with the words when the copy has none: pass WHOOSH_EMBED_WEIGHTS=<folder> in the check's environment to give it one); the
                                                         stage ends ranked, or no_model with the word hits then date order; `from:` and `to:` keep only the right mails; a person
                                                         seen only as the recipient of sent mail is listed with to_count above 0 and people/ready is true; then the copy is stopped
                                                         and started again on the same data folder (scripts/launch-one.sh --relaunch) and the word search, both filters and the people
                                                         are asked again with the same answers; an engine refusal reaches the owner in words (NOT JUDGED while the screen cannot send
                                                         empty words: the hook is named in docs/lanes/search-checks.md). gap-search-cases adds three cases: the reader (search "example", at
                                                         least two matches; after the first result is selected and after Down, Down, Up the reader's mail is the selected result's and one of
                                                         the listed results), the first search typed on the restarted copy (with the model folder given: stages words, then ranked, never
                                                         no_model or the missing-model sentence on the way; the copy is started with WHOOSH_RIG_EMBED_HANDOVER_DELAY_MS=3000 so the window exists in every run: a search typed
                                                         inside it that finds no `words` stage is a FAIL, one typed over 2 s after the copy answered is NOT JUDGED with its measured time) and, on a copy given no model, stage no_model with the engine's own sentence (read from crawl.rs)
    python3 checks/realcore-check.py <bundle> --slice-search --mutant   the same with WHOOSH_CORE_MUTANT=search-never-answers (the engine hook this slice needs; a build without it
                                                         answers the search normally and the run says SURVIVED): the search cases must go red (exit 0 when they did)
    python3 checks/realcore-check.py <bundle> --slice-labelclick   gap-engine-state (item f31ec0ea): a sidebar label click drives the list and the reader on the REAL core (the real-core cell of
                                                          par-inboxlist J1.55). The slice-3 mail and labels (Contracts, Answered, Renewals, pinned); the check clicks Contracts (its tab), unpins
                                                          Answered and Renewals through their own pin doors, clicks Answered (the list holds only its message, the reader shows it), Renewals (empty
                                                          words, the reader cleared), then All inboxes; one PASS line per case, `labelclick/<case>`
    python3 checks/realcore-check.py <bundle> --slice-labelclick --mutant   the same with WHOOSH_CORE_MUTANT=flow-never-files: the two cases that read a label's own messages must go red (exit 0 when they did)
    python3 checks/realcore-check.py <bundle> --slice-accountmark   gap-engine-state (item 33270f65): the account mark on every surface on the REAL core (the real-core cell of checks/seams/account-mark.py).
                                                          One account is added; `PASS accountmark/icon@<surface>` and `colour@<surface>` for the sidebar ring, the list row, the reader meta and the search
                                                          result against the sidebar ring, and the ring's colour against the store's first palette colour; the notch (an arrival) is NOT JUDGED, said so
    python3 checks/realcore-check.py <bundle> --slice-detect   the detector on recorded answers (checks/realcore_netfake.py: DNS and HTTP on 127.0.0.1): Microsoft 365 by its mail
                                                          record, Google by its MX, a provider-database entry, RFC 6186 SRV records, a domain's own Autodiscover, a plain
                                                          IMAP host by the guess, a name that does not exist
    python3 checks/realcore-check.py <bundle> --slice-mailview   lane gap-mailview-checks: the mail view on the REAL engine. The local server holds three mails (a written one, an HTML newsletter, and one with two
                                                          files and an inline logo; the files are not PDF or plain text, which the core fetches by itself). J selects each mail with no Enter:
                                                          the reader holds the whole body, longer than the list preview, and `measure mac-main.interaction.commit` is under INTERACTION_COMMIT_MS.
                                                          The file mail is opened: the gallery lists the two files and not the logo; Space on the first file and the Quick Look button on the
                                                          second are driven through the app's own DownloadAttachment (tile absent, fetching with no figure, present; `core state
                                                          attachment/<id>/transfer` not_on_this_mac, downloading, on_this_mac; the preview waits, then opens); the server saw one numbered part
                                                          fetch per file and none for the logo. One line per case: PASS, FAIL or NOT JUDGED with the values.
    python3 checks/realcore-check.py <bundle> --slice-labels   the four built-in labels (Needs Reply, Updates, Promotions, Social) on a new installation: the label inspector, opened on each,
                                                          shows a description (Needs Reply: the approved sentence) and its entry rules (Promotions one, the others none); where the label has no
                                                          rule the AI card's words say why it cannot sort (NOT JUDGED while the inspector serves no such row)
    python3 checks/realcore-check.py <bundle> --slice-sources   lane sources-slice (item 021cf2d6): the account card on Settings, Replies, Sources on the REAL engine. The local IMAP account is added as
                                                          --slice2 adds it (a server listing INBOX, Sent and Archive), Settings, Replies, Sources is opened (`mac-settings-window.open`, `mac-settings-replies.page.sources`),
                                                          and `rows mac-settings-replies.sources` is read once the folders are listed: one row of kind account, mail_folder rows with enabled true for Inbox and Sent only (Archive listed and off),
                                                          the footnote row `mac-settings-replies.no-account` absent, and rows that hold counts, kinds, folder names and flags only (no mail text)
    python3 checks/realcore-check.py <bundle> --slice-vip   lane sources-slice (item cd640430): mark a sender as VIP through the palette on the REAL engine. The local IMAP account is added as --slice2 does (three unread mails);
                                                          the VIPs count (sidebar row `mac-main.sidebar.mailbox.vips`, absent = 0) is read, one unread inbox mail is focused and its sender's unread inbox mails counted, then the palette line
                                                          `command:Mark as VIP` is clicked: vip-count-follows-the-mark (before plus those mails), vip-mail-leaves-primary (none of that sender left in the list), palette-offers-remove-after-the-mark
                                                          (on the VIPs view the palette lists `command:Remove from VIPs` and no `command:Mark as VIP`), vip-count-is-back-after-remove (after Remove the count is the one before). A build with no
                                                          mark line is NOT JUDGED, never a pass. Ids, counts and door ids only: no mail text and no address is printed
    python3 checks/realcore-check.py <bundle> --slice-moved   lane gap-moved-slice (SPEC A144): a reply to a mail the server moved away, on the REAL engine. The local server (plain: no CONDSTORE, no change list)
                                                          holds three mails with known Message-ID and References. The check waits until the engine serves their stored headers (`core state message/<id>/thread_headers`);
                                                          the server moves mail 2 to Archive and the sync takes it out of the list (`a-mail-moved-away-on-the-server-leaves-the-apps-inbox`: it is gone and the count fell by one);
                                                          mail 1 is opened, the server moves it WITHOUT telling the app, and the quick reply (R, ⌘Enter) answers it
                                                          (`a-reply-to-a-stored-mail-whose-server-copy-moved-is-queued-with-in-reply-to-and-references`: the SMTP side received one message with In-Reply-To the id of mail 1 and
                                                          References the stored chain then that id, and the server saw no header fetch for that UID after the move). Only headers are parsed, no mail text is printed.
    python3 checks/realcore-check.py <bundle> --slice-moved --mutant   the same with WHOOSH_CORE_MUTANT=reply-ignores-store (the reply skips the store and asks the server): the reply case must go red (exit 0 when it did)
    python3 checks/realcore-check.py <bundle> --slice-body   lane tail-realcore (item d99929d0): a multipart quoted-printable mail is shown as decoded text, on the REAL engine. The local server holds a sweetgreen-shaped mail
                                                          (quoted-printable text and html with soft line breaks inside words) and a Soyo-World-shaped one (base64 text part); J and K select each, and the reader's body rows and the list
                                                          row's preview are read: per mail `body-has-no-boundary`, `body-has-no-content-type`, `body-has-no-soft-break`, `body-holds-the-decoded-words`, `preview-is-decoded` (counts and
                                                          booleans only, no mail text is printed; a negative case on an empty body is NOT JUDGED)
    python3 checks/realcore-check.py <bundle> --slice-body --mutant   the same with WHOOSH_CORE_MUTANT=body-stored-raw (the fetched message kept whole as the text): the boundary, header and preview cases must go red
    python3 checks/realcore-check.py <bundle> --slice-rulecount   lane tail-realcore (item f2dd23de): the label rule preview's count is real on the REAL engine. The slice-3 mailbox and labels; the labels screen's new-label
                                                          inspector gets 'unread' and 'replied by me' (count 1: the mail that follows the owner's Sent mail), then 'replied by me' no (count 2); the count is the inspector's own
    python3 checks/realcore-check.py <bundle> --slice-rulecount --mutant   the same with WHOOSH_CORE_MUTANT=rule-counts-zero: the three count cases must go red
    python3 checks/realcore-check.py <bundle> --slice-crawl   lane tail-realcore (items f4e1fbe8, 0c272b3c): every body is downloaded and stored in the background, and every stored mail is indexed, on the REAL engine. The local
                                                          server holds INBOX 25, Archive 65, Sent 10 and records each whole-message fetch with its connection and folder; the engine's figures come from `core state reply/progress`.
                                                          Cases: every-body-of-every-folder-is-stored, inbox-first-newest-first, fetches-come-in-batches, one-connection, bodies-survive-a-relaunch (the copy is restarted and the figures
                                                          read at once), crawled-mail-found-by-a-word (a word in one Archive body only), crawled-mail-found-by-meaning and index-progress-reaches-every-message (both NOT JUDGED without
                                                          WHOOSH_EMBED_WEIGHTS=<the lead's model folder> in the check's environment)
    python3 checks/realcore-check.py <bundle> --slice-crawl --mutant[=<name>]   the same with WHOOSH_CORE_MUTANT=<name> (default crawl-never-runs; also bodies-never-stored, index-never-writes): the stored-count, relaunch and word cases
                                                          (the word case only, for index-never-writes) must go red
    python3 checks/realcore-check.py <bundle> --slice-crawl --paused[=battery:<percent>|heat]   the held crawl: the copy is launched with WHOOSH_CORE_CHECK_CONDITIONS (default battery:15) and watched for 20 s: no whole-message
                                                          fetch of another folder, fewer bodies stored than mails (the plain run is its control)
    python3 checks/realcore-check.py <bundle> --slice-aikey   lane tail-realcore (item ee20d7cc): a provider key is saved through the app's Settings against a fake provider on 127.0.0.1
                                                          (checks/realcore_aifake.py, WHOOSH_CORE_CHECK_PROVIDERS=<its port>; a build without that string is NOT JUDGED and launches nothing) and the quick reply gets a draft written by it: no key = refused
                                                          in words, then the key (a made-up stand-in, never printed) = `connected`, stored, and `reply/draft/<thread>` holds the text the fake answered; the fake saw an accepted completion
    python3 checks/realcore-check.py <bundle> --slice-aisort   lane fix-aisort: two launches, labels seeded as in slice 3b plus "Later"
                                                          (WHOOSH_CORE_CHECK_AISORT=1): the check's AI accepts one of the two mails Renewals' step 1
                                                          picks (its card counts 1 accepted), then with the AI on a closed loopback port the card
                                                          counts 0 accepted, 2 waiting (no "no" kept) and `ai_refusal` holds the catalogue sentence
    python3 checks/realcore-check.py <bundle> --slice-aisort --mutant   the yes run with WHOOSH_CORE_MUTANT=flow-never-asks-ai: 1 accepted must go red
    python3 checks/realcore-check.py <bundle> --slice-sendfail   fix-realcore round 15 (owner main115): the local SMTP server resets the connection at the first
                                                          MAIL FROM; a reply sent from the full composer shows `retrying` (step mail_from, attempts 1, its next try) in
                                                          `outbox/<id>/send_state`, then `sent` by itself with attempts 2, and the server holds one mail after two MAIL FROM
    python3 checks/realcore-check.py --selftest          the judges against model answers and their mutants, no app

EVERY slice ends with one more case, `no-outbound-refused`: the rig core counts the outbound connections its guard refused (`core state rig/refused-outbound`),
and a check that needed the outside is caught by its own run (count above 0 = FAIL; a build that does not serve the path = NOT JUDGED, said once).
Slice 1 and --slice-detect need the build's detector seam, WHOOSH_CORE_CHECK_DETECT=<dns port>:<http port> (spec in checks/realcore_netfake.py, owner lane engine-act-r6):
a build without it prints NOT JUDGED and launches nothing.

The launch is scripts/launch-one.sh: the rig copy, WHOOSH_RIG_HIDDEN=1, opened in the background, no window ever on screen
(owner 2026-11-08 16:30). The core is the Rust one (CoreHost kind .rust): only the real detector reads the network's mail
record and Autodiscover, and only the real core honours the mutant. Detection reads public DNS and the provider lists over HTTPS; no
password is typed, nothing is sent and no account is added. Prints 'N checks, F failed, U not judged' as its last line."""
import os
import re
import subprocess
import sys
import time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
A = "mac-add-account"
# Slice 1 detects a Microsoft 365 tenant. It used to ask for assem@nexus-example.com through PUBLIC DNS and HTTPS, which a hidden test copy must never do ("nothing
# leaves this Mac except the relay"; core/crates/netguard refuses it). The tenant is now a reserved name the local network fake answers (checks/realcore_netfake.py).
ADDRESS = "owner@m365-check.test"
LAST = {"out": ""}      # the text of the last launch, so the one wrapper in main() can read what every slice's inner run printed


def launch(cmd, env):
    """Runs the launcher and keeps its text for the outbound-refusal case (every slice, main())."""
    r = subprocess.run(cmd, cwd=ROOT, env=env, capture_output=True, text=True)
    LAST["out"] = r.stdout + r.stderr
    return r
DETECT_WAIT_S = 90   # every step has its own network timeout; six steps in a row stay under this


def judge(answer):
    """answer: the rows read after detection (a dict). Returns [(name, ok, detail)]."""
    found = (answer.get("found") or [{}])[0]
    steps = answer.get("detection.step") or []
    note = (answer.get("provider-note") or [{}])[0]
    oauth = (answer.get("oauth") or [{}])[0]
    field = answer.get("app_id_field") or {}
    marks = {m.get("placement"): m.get("kind") for m in answer.get("marks") or []}
    last = steps[-1] if steps else {}
    return [
        ("detection-finished", answer.get("stage") == "summary", "stage %r" % answer.get("stage")),
        ("incoming-server-is-microsoft-365", found.get("imap") == "outlook.office365.com:993", "found %r" % found.get("imap")),
        ("sending-server-is-microsoft-365", found.get("smtp") == "smtp.office365.com:587", "found %r" % found.get("smtp")),
        # on the real network nexus-example.com answers at Autodiscover (autodiscover.nexus-example.com is a CNAME of autodiscover.outlook.com,
        # dig 2026-11-09), before the mail record; either Microsoft signal is a pass, the guess or a list is not
        ("found-by-a-microsoft-signal", last.get("id") in ("autodiscover", "mx") and last.get("outcome") == "found", "answered at %r" % last.get("id")),
        ("provider-mark-is-microsoft", marks.get("found") == "microsoft", "marks %r" % marks),
        ("microsoft-says-why-no-password", note.get("note") == "microsoft-no-password", "note %r" % note.get("note")),
        ("app-id-field-shown", oauth.get("kind") == "microsoft" and oauth.get("client_saved") is False and field.get("focusable") is True,
         "oauth %r, field %r" % ({k: oauth.get(k) for k in ("kind", "client_saved", "phase")}, field)),
        ("no-account-added", answer.get("added") == 0, "added %r" % answer.get("added")),
    ]


def drive():
    """Inside launch-one: open the sheet, type the address, Continue, wait for the summary, read the rows."""
    sys.path.insert(0, os.path.join(ROOT, "journeys"))
    from lib.ctl import Ctl  # noqa: E402
    c = Ctl().connect()
    c.click(A + ".open")
    c.click(A + ".address"); c.type(ADDRESS)
    c.click(A + ".continue")
    t0 = time.time(); stage = None
    while time.time() - t0 < DETECT_WAIT_S:
        st = c.rows(A + ".state")
        stage = st[0].get("stage") if st else None
        if stage not in ("detecting", "entry", None):
            break
        time.sleep(0.5)
    answer = {"stage": stage, "seconds": round(time.time() - t0, 1)}
    # the app-ID field takes the focus only when it is on the sheet (AddAccountFeature: oauth.field); no text is typed into it
    try:
        # A88: the shipped default hides the field behind "Use your organisation's own ID"; open it first (absent = refused, fine)
        try: c.click(A + ".oauth.own-id")
        except Exception: pass
        c.click(A + ".oauth.field"); answer["app_id_field"] = {"focusable": True}
    except Exception as e:
        answer["app_id_field"] = {"focusable": False, "why": str(e)}
    for region in ("found", "detection.step", "provider-note", "oauth", "marks"):
        try:
            answer[region] = c.rows("%s.%s" % (A, region))
        except Exception as e:   # a region the screen does not serve is an absent answer, judged red below
            answer[region] = []; answer.setdefault("unreadable", []).append("%s: %s" % (region, e))
    st = c.rows(A + ".state"); answer["added"] = st[0].get("added") if st else None
    try:
        c.click(A + ".cancel")
    except Exception:
        pass
    return answer


CHECK_ADDRESS = "owner@whoosh-check.invalid"   # the core's rig-only seam finds this reserved domain on the local servers
SYNC_WAIT_S = 40


def judge2(answer):
    """Slice 2: the account added through the sheet, its inbox synced from the local server into the list and the counts."""
    rows = answer.get("list") or []
    subjects = sorted(r.get("subject", "") for r in rows)
    counts = answer.get("counts") or {}
    unread = sum(v for v in counts.values() if isinstance(v, int))
    return [
        ("account-added", answer.get("stage") == "added", "stage %r, error %r" % (answer.get("stage"), answer.get("error"))),
        ("core-is-rust", answer.get("core") in (None, "rust"), "uistate core %r" % answer.get("core")),
        ("list-shows-the-synced-inbox", subjects == ["Check message 1", "Check message 2", "Check message 3"], "subjects %r" % subjects),
        ("rows-belong-to-the-added-account", bool(rows) and len({r.get("account") for r in rows}) == 1, "accounts %r" % sorted({str(r.get("account")) for r in rows})),
        ("counts-above-zero", unread > 0, "counts %r" % counts),
        # All inboxes (the sidebar row, the menu bar's number) is the All tab's unread, the inbox's own figure. The store filled no "all inboxes" key, so the
        # row drew no count and the menu bar summed every tab (the lead's read of counts.rs `build`, 14 Nov 2026).
        ("all-inboxes-is-the-all-tabs-unread", isinstance(counts.get("all"), int) and counts.get("all") > 0 and counts.get("all inboxes") == counts.get("all"),
         "all inboxes %r, all %r" % (counts.get("all inboxes"), counts.get("all"))),
    ]


def drive2(before_done=None):
    """Inside launch-one: add the local account through the sheet, Done, then wait for the list and the counts.
    `before_done(c, answer)` runs while the sheet still shows the added account (the step under it and its Details are served only then)."""
    sys.path.insert(0, os.path.join(ROOT, "journeys"))
    from lib.ctl import Ctl  # noqa: E402
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import realcore_mailfake as fake  # noqa: E402
    c = Ctl().connect()
    c.click(A + ".open")
    c.click(A + ".address"); c.type(CHECK_ADDRESS)
    c.click(A + ".password"); c.type(fake.PASSWORD)
    c.click(A + ".continue")
    answer = {}
    t0 = time.time()
    while time.time() - t0 < DETECT_WAIT_S:
        st = c.rows(A + ".state")
        if st and st[0].get("stage") not in ("detecting", "entry"):
            break
        time.sleep(0.3)
    c.click(A + ".add")
    t0 = time.time()
    while time.time() - t0 < DETECT_WAIT_S:
        st = c.rows(A + ".state")
        stage = st[0].get("stage") if st else None
        if stage in ("added", "summary"):
            break
        time.sleep(0.3)
    answer["stage"] = stage
    try:
        err = c.rows(A + ".error"); answer["error"] = err[0].get("text") if err else None
    except Exception:
        answer["error"] = None
    if before_done is not None:
        before_done(c, answer)
    try:
        c.click(A + ".done")
    except Exception as e:
        answer["done"] = str(e)
    t0 = time.time()
    while time.time() - t0 < SYNC_WAIT_S:
        try:
            rows = c.rows("mac-main.list")
        except Exception:
            rows = []
        if len(rows) >= 3:
            break
        time.sleep(0.5)
    answer["seconds"] = round(time.time() - t0, 1)
    answer["list"] = [{k: r.get(k) for k in ("subject", "account", "unread", "sender")} for r in rows]
    ui = c.uistate()
    answer["counts"] = ui.get("counts") or {}
    answer["core"] = ui.get("core")
    return answer


OAUTH_WAIT_S = 40


def judge2b(answer):
    """Slice 2b: the Microsoft-shaped account signed in by the browser flow at `common` (the multi-tenant default ID; the
    fallback to the address's tenant is the cargo suite's), added by XOAUTH2, synced."""
    rows = answer.get("list") or []
    subjects = sorted(r.get("subject", "") for r in rows)
    counts = answer.get("counts") or {}
    unread = sum(v for v in counts.values() if isinstance(v, int))
    oauth = answer.get("oauth") or {}
    auth = answer.get("authority_log") or []
    mail = answer.get("mail_log") or []
    tenant = answer.get("tenant", "common")
    return [
        ("sheet-offers-the-sign-in", oauth.get("kind") == "microsoft", "oauth row %r" % {k: oauth.get(k) for k in ("kind", "phase", "default_client", "field_shown")}),
        ("sign-in-at-common", "GET /%s/oauth2/v2.0/authorize" % tenant in auth, "authority saw %r" % auth),
        ("code-exchanged-for-tokens", "POST /%s/oauth2/v2.0/token authorization_code" % tenant in auth, "authority saw %r" % auth),
        ("account-added", answer.get("stage") == "added", "stage %r, phase %r, error %r" % (answer.get("stage"), answer.get("phase"), answer.get("error"))),
        ("imap-signed-in-by-xoauth2", any(l.startswith("AUTHENTICATE XOAUTH2") for l in mail) and not any(l.startswith("LOGIN") for l in mail),
         "imap commands %r" % sorted({l.split(" ")[0] for l in mail})),
        ("smtp-signed-in-by-xoauth2", answer.get("smtp_auths") == ["XOAUTH2"] or "XOAUTH2" in (answer.get("smtp_auths") or []), "smtp auth %r" % answer.get("smtp_auths")),
        ("list-shows-the-synced-inbox", subjects == ["Check message 1", "Check message 2", "Check message 3"], "subjects %r" % subjects),
        ("counts-above-zero", unread > 0, "counts %r" % counts),
    ]


SIGN_IN_CHECKS = ("code-exchanged-for-tokens", "account-added", "imap-signed-in-by-xoauth2", "list-shows-the-synced-inbox")


def drive2b():
    """Inside launch-one: the address alone, Continue, Sign in (the default client, or a stand-in ID when the field shows),
    wait for the account, Done, then the list."""
    sys.path.insert(0, os.path.join(ROOT, "journeys"))
    from lib.ctl import Ctl  # noqa: E402
    c = Ctl().connect()
    c.click(A + ".open")
    c.click(A + ".address"); c.type(CHECK_ADDRESS)
    c.click(A + ".continue")
    answer = {}
    t0 = time.time()
    while time.time() - t0 < DETECT_WAIT_S:
        st = c.rows(A + ".state")
        if st and st[0].get("stage") not in ("detecting", "entry"):
            break
        time.sleep(0.3)
    o = c.rows(A + ".oauth")
    answer["oauth"] = dict(o[0]) if o else {}
    if answer["oauth"].get("field_shown"):
        # a stand-in of the Microsoft shape, built at run time (the repository holds no client ID but the one default)
        try: c.click(A + ".oauth.own-id")
        except Exception: pass
        c.click(A + ".oauth.field"); c.type("-".join("c" * n for n in (8, 4, 4, 4, 12)))
    try:
        c.click(A + ".oauth.sign-in")
    except Exception as e:
        answer["sign_in_click"] = str(e)
    t0 = time.time(); stage = None; phase = None
    while time.time() - t0 < OAUTH_WAIT_S:
        st = c.rows(A + ".state"); stage = st[0].get("stage") if st else None
        o = c.rows(A + ".oauth"); phase = o[0].get("phase") if o else phase
        if stage == "added" or phase == "refused":
            break
        time.sleep(0.3)
    answer["stage"], answer["phase"] = stage, phase
    try:
        err = c.rows(A + ".error"); answer["error"] = err[0].get("text") if err else None
    except Exception:
        answer["error"] = None
    try:
        c.click(A + ".done")
    except Exception as e:
        answer["done"] = str(e)
    rows = []
    t0 = time.time()
    while time.time() - t0 < SYNC_WAIT_S:
        try:
            rows = c.rows("mac-main.list")
        except Exception:
            rows = []
        if len(rows) >= 3:
            break
        time.sleep(0.5)
    answer["list"] = [{k: r.get(k) for k in ("subject", "account", "unread", "sender")} for r in rows]
    ui = c.uistate()
    answer["counts"] = ui.get("counts") or {}
    answer["core"] = ui.get("core")
    return answer


FLOW_SUBJECTS = ("Contract renewal for Orbit", "Lunch on Friday?", "Budget question")
FLOW_WAIT_S = 40


def flow_raw(sender, to, subject, mid, minute, threading="", body=None):
    """One raw message of the slice-3 mailbox: `minute` is both the Date header's and the server's INTERNALDATE minute (the fake
    derives it from the UID), so the reply sorts after the message it answers. `body` replaces the default "Body of <subject>."."""
    return ("From: %s\r\nTo: %s\r\nSubject: %s\r\nMessage-ID: <%s@whoosh-check.invalid>\r\n%sDate: Mon, 09 Nov 2026 10:%02d:00 +0000\r\n\r\n%s\r\n"
            % (sender, to, subject, mid, threading, minute, body or "Body of %s." % subject)).encode()


def load_flow_mail(fake, mail):
    """INBOX uids 1 to 3 (minutes 1 to 3), Sent uid 2 (minute 2): the owner's message to Kim, which Kim's INBOX message (the third) follows in
    the same thread (its References name the Sent message's Message-ID). "Already answered by me" = the owner wrote EARLIER in the thread
    (owner ruling 2026-11-10), so the third message reads yes; the sync stores INBOX first, so the Sent message arrives AFTER the inbox
    message it precedes and the core has to flip that newer message when it is stored."""
    owner = "Owner <%s>" % fake.ADDRESS
    refs = "In-Reply-To: <flow-ask@whoosh-check.invalid>\r\nReferences: <flow-ask@whoosh-check.invalid>\r\n"
    mail.boxes["INBOX"] = [
        (1, flow_raw("Boss <boss@example.test>", fake.ADDRESS, FLOW_SUBJECTS[0], "flow-1", 1)),
        (2, flow_raw("Pat <pat@example.test>", fake.ADDRESS, FLOW_SUBJECTS[1], "flow-2", 2)),
        (3, flow_raw("Kim <kim@example.test>", fake.ADDRESS, FLOW_SUBJECTS[2], "flow-3", 3, refs)),
    ]
    mail.boxes["Sent"] = [(2, flow_raw(owner, "kim@example.test", "Budget numbers", "flow-ask", 2))]


def judge3(answer):
    """Slice 3: what the list's chips and the core's own record say about each message the sync stored."""
    rows = answer.get("rows") or {}
    contract, lunch, budget = (rows.get(s) or {} for s in FLOW_SUBJECTS)
    def filing(r): return r.get("filing") or {}
    return [
        ("account-added", answer.get("stage") == "added", "stage %r, error %r" % (answer.get("stage"), answer.get("error"))),
        ("core-is-rust", answer.get("core") in (None, "rust"), "uistate core %r" % answer.get("core")),
        ("list-shows-the-three-inbox-messages", sorted(rows) == sorted(FLOW_SUBJECTS), "subjects %r" % sorted(rows)),
        ("entry-rule-files-the-contract-mail", contract.get("chips") == ["Contracts"] and filing(contract).get("card") == "entry" and filing(contract).get("ai") == "not_reached",
         "chips %r, filing %r" % (contract.get("chips"), filing(contract))),
        ("a-reply-the-sync-stored-files-the-answered-mail", budget.get("chips") == ["Answered"] and filing(budget).get("card") == "entry",
         "chips %r, filing %r" % (budget.get("chips"), filing(budget))),
        # engine-act-r6 round 7: the filing's `ai` word for mail the flow left in Primary is what the label AI can do NOW, never assumed. A rig
        # can now hold a decision model (the shell hands Laya's port at open): "configured" = it can answer, "refused" = it is chosen but cannot
        # answer (its words in `ai_refusal`: Laya without its weights), "no_provider" = none is chosen. The old check required "no_provider".
        ("unmatched-mail-stays-in-primary-and-says-what-the-ai-could-do", lunch.get("chips") == [] and filing(lunch).get("place") == "primary" and filing(lunch).get("decided_by") == "default"
         and (filing(lunch).get("ai") in ("no_provider", "configured") or (filing(lunch).get("ai") == "refused" and bool((filing(lunch).get("ai_refusal") or "").strip()))),
         "chips %r, filing %r" % (lunch.get("chips"), filing(lunch))),
    ]


FLOW_CHECKS = ("entry-rule-files-the-contract-mail", "a-reply-the-sync-stored-files-the-answered-mail")


def drive3():
    return flow_drive(FLOW_SUBJECTS, (FLOW_SUBJECTS[0], FLOW_SUBJECTS[2]))


def flow_drive(subjects, chip_subjects, after=None):
    """Inside launch-one: add the local account through the sheet, wait for the rows, give the worker time to file (until `chip_subjects`
    carry a chip), read each row's chips and the core's filing record; `after(ctl, answer)` reads more on the same connection."""
    import json
    sys.path.insert(0, os.path.join(ROOT, "journeys"))
    from lib.ctl import Ctl  # noqa: E402
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import realcore_mailfake as fake  # noqa: E402
    c = Ctl().connect()
    c.click(A + ".open")
    c.click(A + ".address"); c.type(CHECK_ADDRESS)
    c.click(A + ".password"); c.type(fake.PASSWORD)
    c.click(A + ".continue")
    answer = {}
    t0 = time.time()
    while time.time() - t0 < DETECT_WAIT_S:
        st = c.rows(A + ".state")
        if st and st[0].get("stage") not in ("detecting", "entry"):
            break
        time.sleep(0.3)
    c.click(A + ".add")
    stage = None
    t0 = time.time()
    while time.time() - t0 < DETECT_WAIT_S:
        st = c.rows(A + ".state"); stage = st[0].get("stage") if st else None
        if stage in ("added", "summary"):
            break
        time.sleep(0.3)
    answer["stage"] = stage
    try:
        err = c.rows(A + ".error"); answer["error"] = err[0].get("text") if err else None
    except Exception:
        answer["error"] = None
    try:
        c.click(A + ".done")
    except Exception as e:
        answer["done"] = str(e)
    read = {}
    t0 = time.time()
    while time.time() - t0 < FLOW_WAIT_S:   # the filing runs on its own worker after each stored page: read until it shows or the wait ends
        try:
            rows = c.rows("mac-main.list")
        except Exception:
            rows = []
        read = {r.get("subject"): r for r in rows}
        done = len(read) >= len(subjects) and all((read.get(s) or {}).get("chips") for s in chip_subjects)
        if done:
            break
        time.sleep(0.5)
    out = {}
    for subject, r in read.items():
        try:
            v = c.core("message/%s/filing" % r.get("id"))
            filing = json.loads(v) if isinstance(v, str) else (v or {})
        except Exception as e:
            filing = {"unreadable": str(e)}
        out[subject] = {"id": r.get("id"), "chips": r.get("chips") or [], "filing": filing}
    answer["rows"] = out
    ui = c.uistate()
    answer["core"] = ui.get("core")
    if after:
        after(c, answer)
    return answer


def main_slice3(bundle, mutant, ai=False):
    """The local servers live in THIS process (threads) for the whole launch; the app reaches them on 127.0.0.1 only. `ai` (slice 3b): the
    local AI provider too, armed in the rig core by WHOOSH_CORE_CHECK_AI, and the judge reads what it saw."""
    import json
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import realcore_mailfake as fake
    try:
        mail, imap, smtp = fake.start(0, 0, inbox=0)
        provider = None
        if ai:
            import realcore_aifake
            provider = realcore_aifake.start(0)
    except OSError as e:
        print("the local servers did not start: %s" % e)
        print("1 checks, 0 failed, 1 not judged"); return 1
    (load_ai_mail if ai else load_flow_mail)(fake, mail)
    ports = "%d:%d" % (imap.server_address[1], smtp.server_address[1])
    mutant_name = "flow-never-asks-ai" if ai else "flow-never-files"
    extra = (["WHOOSH_CORE_PLAIN_LOOPBACK=1", "WHOOSH_CORE_CHECK_SERVERS=" + ports, "WHOOSH_CORE_CHECK_FLOW=1"] + (["WHOOSH_CORE_CHECK_AI=%d" % provider.server_address[1]] if ai else [])
             + (["WHOOSH_CORE_MUTANT=" + mutant_name] if mutant else []))
    env = dict(os.environ, WHOOSH_RIG_HIDDEN="1", LAUNCH_ENV=" ".join(["WHOOSH_RIG_HIDDEN=1"] + extra))
    env.pop("WHOOSH_FIXTURE_CORE", None)
    cmd = ["sh", os.path.join(ROOT, "scripts", "launch-one.sh"), bundle, "--", "--rig-hidden",
           "sh", "-c", 'WHOOSH_SOCKET_DIR="$D" python3 "$0" --inner --slice3' + ("b" if ai else ""), os.path.abspath(__file__)]
    r = launch(cmd, env)
    imap.shutdown(); smtp.shutdown()
    if provider is not None:
        provider.shutdown()
    out = r.stdout + r.stderr
    line = next((l for l in out.splitlines() if l.startswith("ANSWER ")), None)
    if line is None:
        print(out.rstrip()[-3000:])
        print("launcher exit %d" % r.returncode)
        print("1 checks, 0 failed, 1 not judged"); return 1
    answer = json.loads(line[len("ANSWER "):])
    print("launcher exit %d; the local server saw: %s; %s" % (r.returncode, sorted({l.split(" ")[0] for l in mail.log}), next((l for l in out.splitlines() if l.startswith("instances:")), "")))
    if ai:
        print("the local AI provider saw %d request(s)" % len(provider.seen))
    results = judge3b(answer, provider.seen) if ai else judge3(answer)
    tag = "seeded-slice3b" if ai else "seeded-slice3"
    lines = ["%s: the labels%s are SEEDED by the check (WHOOSH_CORE_CHECK_FLOW%s): this proves the engine path, NOT a first run of the installed app" % (tag.upper(), " and the AI port" if ai else "", "/AI" if ai else "")]
    lines += ["%-10s %s/%s  %s" % ("PASS" if ok else ("NOT JUDGED" if ok is None else "FAIL"), tag, n, d) for n, ok, d in results]
    failed = sum(1 for _, ok, _ in results if ok is False)
    if mutant:
        must_be_red = AI_CHECKS if ai else FLOW_CHECKS
        red = all(ok is False for n, ok, _ in results if n in must_be_red)
        lines.append("mutant %s: %s" % (mutant_name, ("RED as it must be (the flow never asked the AI: the by-meaning mail stays in Primary, the provider saw nothing)" if ai else "RED as it must be (the flow filed nothing, the chips stay empty)") if red
                                         else ("SURVIVED: a message was filed by meaning by a flow that never asks the AI" if ai else "SURVIVED: a message was filed by a flow that files nothing")))
        lines.append("%d checks, %d failed, 0 not judged (mutant run)" % (len(results), failed))
        print("\n".join(lines)); return 0 if red else 1
    lines.append("%d checks, %d failed, 0 not judged" % (len(results), failed))
    print("\n".join(lines)); return 0 if failed == 0 else 1


AI_SUBJECTS = ("Contract renewal for Orbit", "Orbit renewal terms")
AI_BODY = "Hi, I went through the new contracts today and they look right. Please countersign."
AI_LABEL = "Renewals"                                    # A140: the one seeded label with its AI card ON (core/crates/engine/src/check_flow.rs RENEWALS; id generated, found by name)
AI_MEANING_LINE = "Renewals: mail about contract renewals to read and countersign"   # its name and its meaning (check_flow.rs); Contracts and Answered have their AI card off
AI_REASONS = ("laya", "jev")                            # the words `decided_by` carries for a filing the decision model made


def load_ai_mail(fake, mail):
    """INBOX uids 1 and 2 (minutes 1 and 2) and an empty Sent. A140: the first is filed by the Contracts entry rule alone (its SUBJECT says contract; the label's AI card
    is off, so the AI is never asked about it, though its subject also says renewal); the second names no contract in its subject, so Contracts does not take it, but
    says "renewal": step 1 of Renewals (AI on) picks it and the AI rules on it from the body ("contracts"). Nothing else is in the mailbox, so a request the
    provider sees is for this message alone."""
    mail.boxes["INBOX"] = [
        (1, flow_raw("Boss <boss@example.test>", fake.ADDRESS, AI_SUBJECTS[0], "ai-1", 1, body="Please sign.")),
        (2, flow_raw("Pat <pat@example.test>", fake.ADDRESS, AI_SUBJECTS[1], "ai-2", 2, body=AI_BODY)),
    ]
    mail.boxes["Sent"] = []


def drive3b():
    """Slice 3's drive on the two messages of load_ai_mail, then the why-here read of each message under the place the flow put it."""
    import json
    def why_here(c, answer):
        for subject, row in (answer.get("rows") or {}).items():
            place = (row.get("filing") or {}).get("place")
            if not (row.get("id") and place):
                continue
            try:
                v = c.core("message/%s/why/%s" % (row["id"], place))
                row["why"] = json.loads(v) if isinstance(v, str) else (v or {})
            except Exception as e:
                row["why"] = {"unreadable": str(e)}
    return flow_drive(AI_SUBJECTS, AI_SUBJECTS, after=why_here)


def judge3b(answer, seen):
    """Slice 3b: the flow's record of each message, the why-here read, and what the local AI provider received (`seen`, one dict per chat
    request: path, bearer, state, instructions, options)."""
    rows = answer.get("rows") or {}
    rule, meaning = (rows.get(s) or {} for s in AI_SUBJECTS)
    def filing(r): return r.get("filing") or {}
    def why(r): return r.get("why") or {}
    one = seen[0] if len(seen) == 1 else {}
    # A140: the by-meaning mail is under Renewals (AI on, its step 1 picked it), the rule mail under Contracts (AI off): two labels, two places
    by_ai = (meaning.get("chips") == [AI_LABEL] and filing(meaning).get("card") == "ai" and filing(meaning).get("ai") == "answered"
             and filing(meaning).get("decided_by") in AI_REASONS and filing(meaning).get("place") not in (None, "primary", "junk") and filing(meaning).get("place") != filing(rule).get("place"))
    # fix-aisort round 3: the labels offered and their meaning lines, in either form of the Labels question (realcore_aifake asked_labels);
    # a hand-made request without them falls back on the one choice's options and instructions
    options = one.get("labels") or one.get("options") or []
    meanings = "\n".join(one.get("meanings") or []) or one.get("instructions", "")
    return [
        ("account-added", answer.get("stage") == "added", "stage %r, error %r" % (answer.get("stage"), answer.get("error"))),
        ("core-is-rust", answer.get("core") in (None, "rust"), "uistate core %r" % answer.get("core")),
        ("list-shows-the-two-inbox-messages", sorted(rows) == sorted(AI_SUBJECTS), "subjects %r" % sorted(rows)),
        ("entry-rule-still-files-the-contract-mail", rule.get("chips") == ["Contracts"] and filing(rule).get("card") == "entry" and filing(rule).get("ai") == "not_reached",
         "chips %r, filing %r" % (rule.get("chips"), filing(rule))),
        ("by-meaning-mail-is-filed-by-the-ai-card", by_ai, "chips %r, filing %r" % (meaning.get("chips"), filing(meaning))),
        # A140: the AI card's read carries the AI's yes and step 1's own read (chain 0 and its held conditions); the rule mail keeps the entry card
        ("why-here-names-the-card-and-chain", why(meaning).get("card") == "ai" and why(meaning).get("chain_id") is None and why(meaning).get("decided_by") == filing(meaning).get("decided_by")
         and why(meaning).get("ai") == "yes" and (why(meaning).get("entry") or {}).get("chain_id") == 0 and bool((why(meaning).get("entry") or {}).get("matched"))
         and why(rule).get("card") == "entry" and why(rule).get("chain_id") == 0,
         "meaning why %r (want card ai, ai yes, entry chain 0 with held conditions), rule why %r" % (why(meaning), why(rule))),
        ("an-ai-off-label-is-not-offered-to-the-ai", bool(one) and AI_LABEL in options and "Contracts" not in options and "Answered" not in options,
         "options %r (want %s only: Contracts and Answered have their AI card off)" % (options, AI_LABEL)),
        ("the-provider-saw-exactly-one-request", len(seen) == 1 and one.get("path") == "/chat/completions", "%d request(s): %r" % (len(seen), [s.get("path") for s in seen])),
        ("the-request-carries-the-label-meaning-and-the-message", bool(one) and AI_MEANING_LINE in meanings and AI_LABEL in options
         and ("subject: " + AI_SUBJECTS[1]) in one.get("state", "") and AI_BODY in one.get("state", ""),
         "instructions %r, options %r, state %r" % (one.get("instructions"), one.get("options"), one.get("state"))),
        ("the-request-leaves-attachments-out", bool(one) and "attachments" not in one.get("state", ""), "state %r" % one.get("state")),
        ("the-request-carries-the-dummy-key-as-a-bearer", one.get("bearer") is True, "bearer %r" % one.get("bearer")),
    ]


AI_CHECKS = ("by-meaning-mail-is-filed-by-the-ai-card", "why-here-names-the-card-and-chain", "an-ai-off-label-is-not-offered-to-the-ai", "the-provider-saw-exactly-one-request",
             "the-request-carries-the-label-meaning-and-the-message", "the-request-leaves-attachments-out", "the-request-carries-the-dummy-key-as-a-bearer")


def selftest3b():
    """The judge against a model answer and the never-asks model, and the fixture mailbox against Python's own IMAP client (no app)."""
    import imaplib
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import realcore_mailfake as fake
    def row(chips, id, **f): return {"id": id, "chips": chips, "filing": f}
    request = {"path": "/chat/completions", "bearer": True, "state": "from: pat@example.test\nsubject: %s\n\n%s" % (AI_SUBJECTS[1], AI_BODY),
               "instructions": "Which label?\n" + AI_MEANING_LINE, "options": [AI_LABEL]}
    good = {"stage": "added", "core": "rust", "rows": {
        AI_SUBJECTS[0]: dict(row(["Contracts"], "m1", card="entry", chain_id=0, ai="not_reached", decided_by="rule:r1", place="l-1"), why={"card": "entry", "chain_id": 0, "decided_by": "rule:r1"}),
        AI_SUBJECTS[1]: dict(row([AI_LABEL], "m2", card="ai", chain_id=None, ai="answered", decided_by="laya", place="l-3"),
                             why={"card": "ai", "chain_id": None, "decided_by": "laya", "ai": "yes", "entry": {"chain_id": 0, "matched": [{"kind": "subject", "words": "renewal"}]}})}}
    never = dict(good, rows={AI_SUBJECTS[0]: good["rows"][AI_SUBJECTS[0]],
                             AI_SUBJECTS[1]: dict(row([], "m2", card="hand", chain_id=None, ai="no_provider", decided_by="default", place="primary"), why={"card": "hand", "chain_id": None, "decided_by": "default"})})
    wrong_label = dict(good, rows=dict(good["rows"], **{AI_SUBJECTS[1]: dict(good["rows"][AI_SUBJECTS[1]], chips=["Answered"], filing=dict(good["rows"][AI_SUBJECTS[1]]["filing"], place="l-2"))}))
    no_entry = dict(good, rows=dict(good["rows"], **{AI_SUBJECTS[1]: dict(good["rows"][AI_SUBJECTS[1]], why={"card": "ai", "chain_id": None, "decided_by": "laya"})}))   # the pre-A140 read
    ok_why = not dict((n, x) for n, x, _ in judge3b(no_entry, [request]))["why-here-names-the-card-and-chain"]
    ok_offered = not dict((n, x) for n, x, _ in judge3b(good, [dict(request, options=["Contracts", AI_LABEL])]))["an-ai-off-label-is-not-offered-to-the-ai"]
    ok_good = all(x for _, x, _ in judge3b(good, [request]))
    ok_never = all(not x for n, x, _ in judge3b(never, []) if n in AI_CHECKS)
    ok_twice = not dict((n, x) for n, x, _ in judge3b(good, [request, request]))["the-provider-saw-exactly-one-request"]
    ok_wrong = not dict((n, x) for n, x, _ in judge3b(wrong_label, [request]))["by-meaning-mail-is-filed-by-the-ai-card"]
    ok_body = not dict((n, x) for n, x, _ in judge3b(good, [dict(request, state="from: a\nsubject: %s\n\n" % AI_SUBJECTS[1])]))["the-request-carries-the-label-meaning-and-the-message"]
    ok_att = not dict((n, x) for n, x, _ in judge3b(good, [dict(request, state=request["state"] + "\nattachments: yes")]))["the-request-leaves-attachments-out"]
    ok_key = not dict((n, x) for n, x, _ in judge3b(good, [dict(request, bearer=False)]))["the-request-carries-the-dummy-key-as-a-bearer"]
    mail, imap, smtp = fake.start(0, 0, inbox=0)
    try:
        load_ai_mail(fake, mail)
        c = imaplib.IMAP4("127.0.0.1", imap.server_address[1])
        c.login(fake.ADDRESS, fake.PASSWORD)
        c.select("INBOX")
        typ, data = c.uid("FETCH", "1:2", "(UID BODY.PEEK[])")
        text = b"".join(x[1] for x in data if isinstance(x, tuple))
        served = all(s.encode() in text for s in AI_SUBJECTS) and AI_BODY.encode() in text and b"Please sign." in text
        c.select("Sent")
        typ, data = c.uid("SEARCH", None, "ALL")
        empty_sent = data == [b""]
        c.logout()
    finally:
        imap.shutdown(); smtp.shutdown()
    import realcore_aifake
    print("selftest slice 3b: model %s, never-asks model %s, a twice-asked, wrong-label, bodiless, attachment and keyless request each %s, fixture mailbox %s" %
          ("all pass" if ok_good else "FAILS", "red" if ok_never else "SURVIVES", "red" if ok_twice and ok_wrong and ok_body and ok_att and ok_key and ok_why and ok_offered else "SURVIVES", "served" if served and empty_sent else "WRONG"))
    return ok_good and ok_never and ok_twice and ok_wrong and ok_body and ok_att and ok_key and ok_why and ok_offered and served and empty_sent and realcore_aifake.selftest()

# ---------------------------------------------------------------- lane fix-aisort: the label AI that accepted no mail (owner 2026-11-12 15:02)
# Two launches of the rig copy, labels seeded as in slice 3b plus "Later" (WHOOSH_CORE_CHECK_AISORT=1, check_flow.rs LATER): a second AI-on label whose
# step 1 picks the same "renewal" subjects. Step 1 of Renewals picks both mails. YES run: the check's AI (realcore_aifake, loopback) accepts the mail
# whose body shares a word with Renewals' meaning ("countersign") and answers Later for the other: Renewals' card counts 1 accepted, 0 waiting.
# FAILING run: the AI port points at a closed loopback port (every call fails): no "no" is kept (both mails wait: waiting_for_ai 2, 0 accepted)
# and `label/<Renewals>/flow/ai_refusal` holds the one catalogue sentence (code ai_failed). `--mutant`: the yes run with flow-never-asks-ai.
AISORT_SUBJECTS = ("Orbit renewal", "Webinar renewal reminder")
AISORT_BODIES = ("Please countersign the attached pages.", "Seats are limited.")
AISORT_FAILED_CODE = "ai_failed"                                  # core/crates/engine/src/label_ai_failure.rs AI_FAILED_CODE
AISORT_FAILED_WORDS = "The AI could not judge the mail of this label. It tries again in the background; core.log says why."   # AI_FAILED_WORDS there
AISORT_YES_CHECKS = ("the-card-shows-1-accepted",)
AISORT_FAIL_CHECKS = ("the-failing-model-keeps-no-no-both-mails-wait", "the-card-says-why-in-one-sentence")
# round 11 (owner 23:44, "Why here doesn't"): a third run on the rig's scripted Apple judge (WHOOSH_RIG_APPLE_JUDGE=stand-in, rig only;
# AppleLabelPrompt.standIn): a mail whose SUBJECT holds the refuse mark is refused twice (with and without its body) and set aside; the others are judged
AISORT_REFUSED_SUBJECT = "Renewal unsafesample notice"          # AppleLabelPrompt.refuseMark is "unsafesample"
AISORT_REFUSE_CHECKS = ("the-refused-mail-is-set-aside", "the-others-are-judged-and-nothing-waits", "a-set-aside-mail-shows-no-failure")


def load_aisort_mail(fake, mail, refusing=False):
    """INBOX uids 1 and 2; both subjects say "renewal" (step 1 of Renewals and of Later picks both; no subject says "contract", so Contracts files neither).
    `refusing` (round 11): uid 3 too, whose subject holds the stand-in judge's refuse mark."""
    mail.boxes["INBOX"] = [
        (1, flow_raw("Pat <pat@example.test>", fake.ADDRESS, AISORT_SUBJECTS[0], "aisort-1", 1, body=AISORT_BODIES[0])),
        (2, flow_raw("Pat <pat@example.test>", fake.ADDRESS, AISORT_SUBJECTS[1], "aisort-2", 2, body=AISORT_BODIES[1])),
    ] + ([(3, flow_raw("Pat <pat@example.test>", fake.ADDRESS, AISORT_REFUSED_SUBJECT, "aisort-3", 3, body="See you there."))] if refusing else [])
    mail.boxes["Sent"] = []


def drive_aisort():
    """Slice 3's drive, then Renewals' card as the inspector reads it: its id from `inbox/tabs` (it is pinned), `label/<id>/flow/counts` and
    `label/<id>/flow/ai_refusal`, read until the run shows its outcome or FLOW_WAIT_S ends."""
    import json
    failing = "--failing" in sys.argv
    refusing = "--refusing" in sys.argv
    def card(c, answer):
        def read(path):
            try:
                v = c.core(path)
                return json.loads(v) if isinstance(v, str) else v
            except Exception as e:
                return {"unreadable": str(e)}
        tabs = read("inbox/tabs") or []
        rid = next((t.get("id") for t in tabs if isinstance(t, dict) and t.get("name") == AI_LABEL), None)
        got = {"id": rid}
        t0 = time.time()
        while rid and time.time() - t0 < FLOW_WAIT_S:
            got["counts"] = read("label/%s/flow/counts" % rid) or {}
            got["ai_refusal"] = read("label/%s/flow/ai_refusal" % rid)
            got["ai_set_aside"] = read("label/%s/flow/ai_set_aside" % rid)
            done = (got["counts"].get("sorted_by_ai") and str(got["ai_set_aside"]) == "1" and got["counts"].get("waiting_for_ai") == 0) if refusing \
                else (got["ai_refusal"] if failing else got["counts"].get("sorted_by_ai"))
            if done:
                break
            time.sleep(0.5)
        answer["renewals"] = got
    return flow_drive(AISORT_SUBJECTS + ((AISORT_REFUSED_SUBJECT,) if refusing else ()), () if failing else (AISORT_SUBJECTS[0],), after=card)


def judge_refusing(ref):
    """Round 11: the run on the rig's scripted Apple judge, which refuses mail 3 for its content (twice: with and without the body)."""
    def card(a): return (a or {}).get("renewals") or {}
    c = card(ref).get("counts") or {}
    refusal = card(ref).get("ai_refusal")
    return [
        ("the-refused-mail-is-set-aside", str(card(ref).get("ai_set_aside")) == "1", "ai_set_aside %r (want 1: mail 3 kept so it is not asked again)" % card(ref).get("ai_set_aside")),
        ("the-others-are-judged-and-nothing-waits", c.get("caught_by_entry") == 3 and c.get("sorted_by_ai") == 1 and c.get("waiting_for_ai") == 0,
         "counts %r (want caught_by_entry 3, sorted_by_ai 1, waiting_for_ai 0)" % c),
        # round 12: no refusal at all (main110 passed this case with {'code': 'ai_no_model', 'words': 'unsafe'}: the mail's reason id had leaked
        # into the label's refusal); read as `the-yes-run-says-no-failure` would be too loose: any refusal here is a failure shown to the owner
        ("a-set-aside-mail-shows-no-failure", refusal is None, "ai_refusal %r (want none: a set-aside mail never sets the label's refusal)" % refusal),
    ]


def judge_aisort(yes, fail, refusing=None):
    """The yes run's and the failing run's answers (`fail` None in a mutant run: its cases are not judged); `refusing` the round-11 run."""
    def card(a): return (a or {}).get("renewals") or {}
    def counts(a): return card(a).get("counts") or {}
    out = [
        ("account-added", yes.get("stage") == "added", "stage %r, error %r" % (yes.get("stage"), yes.get("error"))),
        ("core-is-rust", yes.get("core") in (None, "rust"), "uistate core %r" % yes.get("core")),
        ("renewals-is-found-by-name", bool(card(yes).get("id")), "card %r" % card(yes)),
        ("the-card-shows-1-accepted", counts(yes).get("caught_by_entry") == 2 and counts(yes).get("sorted_by_ai") == 1 and counts(yes).get("waiting_for_ai") == 0,
         "counts %r (want caught_by_entry 2, sorted_by_ai 1, waiting_for_ai 0: one yes, one kept no)" % counts(yes)),
        # the general refusal of the saved choice may stand (the check's port is not the saved model); a failure of the AI may not
        ("the-yes-run-says-no-failure", (card(yes).get("ai_refusal") or {}).get("code") != AISORT_FAILED_CODE if isinstance(card(yes).get("ai_refusal"), (dict, type(None))) else False,
         "ai_refusal %r" % card(yes).get("ai_refusal")),
    ]
    if fail is None:
        return out + [(n, None, "the failing run is not made in a mutant run") for n in AISORT_FAIL_CHECKS + AISORT_REFUSE_CHECKS]
    refusal = card(fail).get("ai_refusal") if isinstance(card(fail).get("ai_refusal"), dict) else {}
    extra = judge_refusing(refusing) if refusing is not None else [(n, None, "the refusing run was not made") for n in AISORT_REFUSE_CHECKS]
    return extra + out + [
        ("the-failing-model-keeps-no-no-both-mails-wait", counts(fail).get("caught_by_entry") == 2 and counts(fail).get("sorted_by_ai") == 0 and counts(fail).get("waiting_for_ai") == 2,
         "counts %r (want caught_by_entry 2, sorted_by_ai 0, waiting_for_ai 2: no \"no\" row for a failed judgement)" % counts(fail)),
        ("the-card-says-why-in-one-sentence", refusal.get("code") == AISORT_FAILED_CODE and refusal.get("words") == AISORT_FAILED_WORDS,
         "ai_refusal %r (want code %s and the catalogue sentence)" % (card(fail).get("ai_refusal"), AISORT_FAILED_CODE)),
    ]


def main_slice_aisort(bundle, mutant):
    import json
    import socket
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import realcore_mailfake as fake
    import realcore_aifake
    answers = {}
    for run in (("yes",) if mutant else ("yes", "failing", "refusing")):
        try:
            mail, imap, smtp = fake.start(0, 0, inbox=0)
            provider = realcore_aifake.start(0) if run == "yes" else None
        except OSError as e:
            print("the local servers did not start: %s" % e)
            print("1 checks, 0 failed, 1 not judged"); return 1
        if provider is not None:
            ai_port = provider.server_address[1]
        else:   # a loopback port nobody listens on: every call of the check's AI fails (connection refused)
            sock = socket.socket(); sock.bind(("127.0.0.1", 0)); ai_port = sock.getsockname()[1]; sock.close()
        load_aisort_mail(fake, mail, refusing=run == "refusing")
        ports = "%d:%d" % (imap.server_address[1], smtp.server_address[1])
        # round 11: the refusing run has NO check port, so the label flow asks the shell's own router, armed with the scripted Apple judge
        judge_env = ["WHOOSH_RIG_APPLE_JUDGE=stand-in"] if run == "refusing" else ["WHOOSH_CORE_CHECK_AI=%d" % ai_port]
        extra = (["WHOOSH_CORE_PLAIN_LOOPBACK=1", "WHOOSH_CORE_CHECK_SERVERS=" + ports, "WHOOSH_CORE_CHECK_FLOW=1", "WHOOSH_CORE_CHECK_AISORT=1"] + judge_env
                 + (["WHOOSH_CORE_MUTANT=flow-never-asks-ai"] if mutant else []))
        env = dict(os.environ, WHOOSH_RIG_HIDDEN="1", LAUNCH_ENV=" ".join(["WHOOSH_RIG_HIDDEN=1"] + extra))
        env.pop("WHOOSH_FIXTURE_CORE", None)
        cmd = ["sh", os.path.join(ROOT, "scripts", "launch-one.sh"), bundle, "--", "--rig-hidden",
               "sh", "-c", 'WHOOSH_SOCKET_DIR="$D" python3 "$0" --inner --slice-aisort' + {"failing": " --failing", "refusing": " --refusing"}.get(run, ""), os.path.abspath(__file__)]
        r = launch(cmd, env)
        imap.shutdown(); smtp.shutdown()
        if provider is not None:
            provider.shutdown()
        out = r.stdout + r.stderr
        line = next((l for l in out.splitlines() if l.startswith("ANSWER ")), None)
        if line is None:
            print(out.rstrip()[-3000:])
            print("launcher exit %d (%s run)" % (r.returncode, run))
            print("1 checks, 0 failed, 1 not judged"); return 1
        answers[run] = json.loads(line[len("ANSWER "):])
        print("%s run: launcher exit %d; the local AI provider saw %s request(s)" % (run, r.returncode, len(provider.seen) if provider is not None else "no (closed port)"))
    results = judge_aisort(answers["yes"], answers.get("failing"), answers.get("refusing"))
    lines = ["SEEDED-SLICE-AISORT: the labels and the AI port are SEEDED by the check (WHOOSH_CORE_CHECK_FLOW/AI/AISORT): this proves the engine path, NOT the owner's Laya"]
    lines += ["%-10s slice-aisort/%s  %s" % ("PASS" if ok else ("NOT JUDGED" if ok is None else "FAIL"), n, d) for n, ok, d in results]
    failed = sum(1 for _, ok, _ in results if ok is False)
    unjudged = sum(1 for _, ok, _ in results if ok is None)
    if mutant:
        red = all(ok is False for n, ok, _ in results if n in AISORT_YES_CHECKS)
        lines.append("mutant flow-never-asks-ai: %s" % ("RED as it must be (no model asked: 0 accepted)" if red else "SURVIVED: the card counted a mail the AI accepted while no AI was asked"))
        lines.append("%d checks, %d failed, %d not judged (mutant run)" % (len(results), failed, unjudged))
        print("\n".join(lines)); return 0 if red else 1
    lines.append("%d checks, %d failed, %d not judged" % (len(results), failed, unjudged))
    print("\n".join(lines)); return 0 if failed == 0 and unjudged == 0 else 1


def selftest_aisort():
    """The judge against a model answer and its broken models (no app): the code before lane fix-aisort (a failed judgement kept as a "no":
    nothing waits, no sentence), a flow that never asks the AI (0 accepted), and a card that says nothing; and the fixture mailbox through
    the fake AI's own pick (the first mail goes to Renewals, the second to Later)."""
    import realcore_aifake
    def answer(counts, refusal): return {"stage": "added", "core": "rust", "renewals": {"id": "l-3", "counts": counts, "ai_refusal": refusal}}
    good_yes = answer({"caught_by_entry": 2, "kept_by_exit": 0, "sent_to_fallback": 0, "sorted_by_ai": 1, "waiting_for_ai": 0}, None)
    good_fail = answer({"caught_by_entry": 2, "kept_by_exit": 0, "sent_to_fallback": 0, "sorted_by_ai": 0, "waiting_for_ai": 2}, {"code": AISORT_FAILED_CODE, "words": AISORT_FAILED_WORDS})
    before_fix = answer({"caught_by_entry": 2, "kept_by_exit": 0, "sent_to_fallback": 0, "sorted_by_ai": 0, "waiting_for_ai": 0}, None)
    never_asks = answer({"caught_by_entry": 2, "kept_by_exit": 0, "sent_to_fallback": 0, "sorted_by_ai": 0, "waiting_for_ai": 2}, None)
    no_model_words = dict(good_fail, renewals=dict(good_fail["renewals"], ai_refusal={"code": "ai_no_model", "words": "No model is set for sorting mail: choose one in Settings, Decisions."}))
    # round 11: the refusing run: mail 3 set aside, the others judged, no failure; before round 11 it waited for ever behind the failure sentence
    good_ref = dict(answer({"caught_by_entry": 3, "kept_by_exit": 0, "sent_to_fallback": 0, "sorted_by_ai": 1, "waiting_for_ai": 0}, None))
    good_ref["renewals"]["ai_set_aside"] = 1
    before_r11 = dict(answer({"caught_by_entry": 3, "kept_by_exit": 0, "sent_to_fallback": 0, "sorted_by_ai": 1, "waiting_for_ai": 1}, {"code": AISORT_FAILED_CODE, "words": AISORT_FAILED_WORDS}))
    before_r11["renewals"]["ai_set_aside"] = 0
    verdict = lambda y, f, r=good_ref: dict((n, x) for n, x, _ in judge_aisort(y, f, r))
    ok_refuse = all(verdict(good_yes, good_fail, before_r11)[n] is False for n in AISORT_REFUSE_CHECKS) and all(verdict(good_yes, good_fail)[n] for n in AISORT_REFUSE_CHECKS)
    # round 12: main110's line, the mail's reason id leaked into the label's refusal, is red now
    leaked = dict(good_ref, renewals=dict(good_ref["renewals"], ai_refusal={"code": "ai_no_model", "words": "unsafe"}))
    ok_refuse = ok_refuse and verdict(good_yes, good_fail, leaked)["a-set-aside-mail-shows-no-failure"] is False
    ok_good = all(verdict(good_yes, good_fail).values()) and ok_refuse
    ok_before = all(verdict(good_yes, before_fix)[n] is False for n in AISORT_FAIL_CHECKS)
    ok_never = all(verdict(never_asks, good_fail)[n] is False for n in AISORT_YES_CHECKS)
    ok_words = verdict(good_yes, no_model_words)["the-card-says-why-in-one-sentence"] is False
    ok_mutant_run = all(x is None for n, x in verdict(good_yes, None).items() if n in AISORT_FAIL_CHECKS)
    # the fake's own pick on the two mails, as the engine asks it (options sorted: Later, Renewals; the meanings ride in the instructions)
    instructions = "Which label?\nLater: kept for someday\nRenewals: mail about contract renewals to read and countersign"
    def picked(subject, body):
        ask = {"state": "from: pat@example.test\nsubject: %s\n\n%s" % (subject, body), "questions": {"labels/model": {"type": "choice", "instructions": instructions, "criteria": {"Later": "Later", AI_LABEL: AI_LABEL}}}}
        return realcore_aifake.pick(ask)["labels/model"]["choice"]
    ok_pick = picked(AISORT_SUBJECTS[0], AISORT_BODIES[0]) == AI_LABEL and picked(AISORT_SUBJECTS[1], AISORT_BODIES[1]) == "Later"
    # round 3: the per-label form (LABEL_AI_QUESTION per-label): the same pick, as a yes for that label and a no for the other
    def per_label(subject, body):
        qs = {"labels/model#l-later": {"type": "noul", "instructions": 'This email belongs under the label "Later": kept for someday', "criteria": {"true": "yes", "false": "no"}},
              "labels/model#l-3": {"type": "noul", "instructions": 'This email belongs under the label "%s": mail about contract renewals to read and countersign' % AI_LABEL, "criteria": {"true": "yes", "false": "no"}}}
        out = realcore_aifake.pick({"state": "from: pat@example.test\nsubject: %s\n\n%s" % (subject, body), "questions": qs})
        return [k for k, v in out.items() if v.get("noul", 0) >= 0.5], realcore_aifake.asked_labels({"questions": qs})
    yes_a, offered = per_label(AISORT_SUBJECTS[0], AISORT_BODIES[0])
    yes_b, _ = per_label(AISORT_SUBJECTS[1], AISORT_BODIES[1])
    # round 8 (main100, 18:40): the engine's question ids carry generated label ids; when Renewals' id sorts first, the mail that shares no word
    # must still go to Later (the first label by NAME, as the one choice's sorted options): the same pick with the questions in the other order
    def per_label_reversed(subject, body):
        qs = {"labels/model#a-renewals": {"type": "noul", "instructions": 'This email belongs under the label "%s": mail about contract renewals to read and countersign' % AI_LABEL, "criteria": {"true": "yes", "false": "no"}},
              "labels/model#z-later": {"type": "noul", "instructions": 'This email belongs under the label "Later": kept for someday', "criteria": {"true": "yes", "false": "no"}}}
        out = realcore_aifake.pick({"state": "from: pat@example.test\nsubject: %s\n\n%s" % (subject, body), "questions": qs})
        return [k for k, v in out.items() if v.get("noul", 0) >= 0.5]
    ok_order = per_label_reversed(AISORT_SUBJECTS[0], AISORT_BODIES[0]) == ["labels/model#a-renewals"] and per_label_reversed(AISORT_SUBJECTS[1], AISORT_BODIES[1]) == ["labels/model#z-later"]
    ok_pick = ok_pick and ok_order and yes_a == ["labels/model#l-3"] and yes_b == ["labels/model#l-later"] and offered[0] == ["Later", AI_LABEL] and AI_MEANING_LINE in offered[1]
    print("selftest slice aisort: model %s, the code before the fix %s, a flow that never asks %s, the general no-model words %s, a mutant run leaves the failing cases %s, the fake's pick %s" % (
        "all pass" if ok_good else "FAILS", "red" if ok_before else "SURVIVES", "red" if ok_never else "SURVIVES", "red" if ok_words else "SURVIVE",
        "not judged" if ok_mutant_run else "WRONG", "Renewals then Later, as a choice and per label" if ok_pick else "WRONG"))
    return ok_good and ok_before and ok_never and ok_words and ok_mutant_run and ok_pick



def selftest3():
    """The judge against a model answer and the never-files model, and the fixture mailbox against Python's own IMAP client (no app)."""
    import imaplib
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import realcore_mailfake as fake
    def row(chips, **f): return {"chips": chips, "filing": f}
    good = {"stage": "added", "core": "rust", "rows": {
        FLOW_SUBJECTS[0]: row(["Contracts"], card="entry", chain_id=0, ai="not_reached", decided_by="rule:r1", place="l-1"),
        FLOW_SUBJECTS[1]: row([], card="hand", chain_id=None, ai="no_provider", decided_by="default", place="primary"),
        FLOW_SUBJECTS[2]: row(["Answered"], card="entry", chain_id=0, ai="not_reached", decided_by="rule:r2", place="l-2")}}
    never = dict(good, rows={FLOW_SUBJECTS[0]: row([], card="hand", ai="no_provider", decided_by="default", place="primary"),
                             FLOW_SUBJECTS[1]: good["rows"][FLOW_SUBJECTS[1]], FLOW_SUBJECTS[2]: row([], card="hand", ai="no_provider", decided_by="default", place="primary")})
    ok = all(x for _, x, _ in judge3(good)) and all(not x for n, x, _ in judge3(never) if n in FLOW_CHECKS)
    # engine-act-r6 round 7: the three truthful words pass; "refused" without the refusal's words, and a word the flow does not say, are red
    lunch_with = lambda **f: dict(good, rows=dict(good["rows"], **{FLOW_SUBJECTS[1]: row([], **dict(good["rows"][FLOW_SUBJECTS[1]]["filing"], **f))}))
    judged = lambda answer: dict((n, x) for n, x, _ in judge3(answer))["unmatched-mail-stays-in-primary-and-says-what-the-ai-could-do"]
    ok = ok and judged(lunch_with(ai="configured", ai_refusal=None)) and judged(lunch_with(ai="refused", ai_refusal="The on-device decision model is not on this Mac yet."))
    ok = ok and not judged(lunch_with(ai="refused", ai_refusal=None)) and not judged(lunch_with(ai="answered")) and not judged(lunch_with(place="l-1"))
    mail, imap, smtp = fake.start(0, 0, inbox=0)
    try:
        load_flow_mail(fake, mail)
        c = imaplib.IMAP4("127.0.0.1", imap.server_address[1])
        c.login(fake.ADDRESS, fake.PASSWORD)
        c.select("INBOX")
        typ, data = c.uid("FETCH", "1:3", "(UID BODY.PEEK[HEADER])")
        heads = b"".join(x[1] for x in data if isinstance(x, tuple))
        subjects_ok = all(s.encode() in heads for s in FLOW_SUBJECTS)
        c.select("Sent")
        typ, data = c.uid("FETCH", "2", "(UID BODY.PEEK[HEADER])")
        asked = b"".join(x[1] for x in data if isinstance(x, tuple))
        c.select("INBOX")
        typ, data = c.uid("FETCH", "3", "(UID BODY.PEEK[HEADER])")
        follow = b"".join(x[1] for x in data if isinstance(x, tuple))
        threaded = b"References: <flow-ask@whoosh-check.invalid>" in follow and b"Message-ID: <flow-ask@whoosh-check.invalid>" in asked and b"From: Owner <" + fake.ADDRESS.encode() in asked
        c.logout()
    finally:
        imap.shutdown(); smtp.shutdown()
    print("selftest slice 3: model %s, never-files model %s, fixture mailbox %s" % ("all pass" if all(x for _, x, _ in judge3(good)) else "FAILS", "red" if ok else "SURVIVES", "served" if subjects_ok and threaded else "WRONG"))
    return ok and subjects_ok and threaded


IMPORT_SUBJECTS = ("Imported: quarterly report", "Imported: holiday photos", "Imported: invoice 4471")
IMPORT_SOURCE = "apple-mail:~/Library/Mail"
IMPORT_WAIT_S = 60
IMPORT_LIST_WAIT_S = 5


def import_emlx(message):
    """One Apple Mail message file: the byte count, the message, the plist Mail appends (read, received 2 Jan 2024)."""
    body = message.encode()
    plist = (b'<?xml version="1.0" encoding="UTF-8"?>\n<plist version="1.0">\n<dict>\n\t<key>date-received</key>\n\t<integer>1704190272</integer>\n'
             b'\t<key>flags</key>\n\t<integer>1</integer>\n</dict>\n</plist>\n')
    return str(len(body)).encode() + b"\n" + body + plist


def build_import_fixture(home):
    """`<home>/Library/Mail/V10/acct/Inbox.mbox/GUID/Data/1/Messages/<n>.emlx`: three synthetic messages, no real mail."""
    folder = os.path.join(home, "Library", "Mail", "V10", "acct", "Inbox.mbox", "GUID", "Data", "1", "Messages")
    os.makedirs(folder, exist_ok=True)
    senders = ("boss@example.test", "pat@example.test", "kim@example.test")
    for n, (subject, sender) in enumerate(zip(IMPORT_SUBJECTS, senders), 1):
        msg = ("Message-ID: <import-%d@whoosh-check.invalid>\nFrom: %s\nTo: stranger@whoosh-check.invalid\nSubject: %s\n"
               "Date: Tue, 02 Jan 2024 10:11:12 +0000\n\nBody of %s.\n" % (n, sender, subject, subject))
        with open(os.path.join(folder, "%d.emlx" % n), "wb") as f:
            f.write(import_emlx(msg))
    return folder


def judge4(answer):
    """Slice 4: what the core's own record and the list say about the import the rig copy ran."""
    import json
    rows = answer.get("rows")
    sources = answer.get("sources")
    try:
        found = next((x for x in (json.loads(sources) if isinstance(sources, str) else (sources or [])) if x.get("id") == IMPORT_SOURCE), {})
    except Exception:
        found = {}
    return [
        ("core-is-rust", answer.get("core") in (None, "rust"), "uistate core %r" % answer.get("core")),
        ("source-is-listed-readable-with-its-count", found.get("access") == "readable" and found.get("message_count") == len(IMPORT_SUBJECTS) and found.get("kind") == "apple-mail",
         "source %r" % found),
        ("run-stored-every-message-and-the-history-says-so", found.get("imported_count") == len(IMPORT_SUBJECTS) and found.get("last_run_ms") is not None,
         "imported_count %r, last_run_ms %r" % (found.get("imported_count"), found.get("last_run_ms"))),
        ("list-shows-the-imported-messages", None if rows is None else sorted(rows) == sorted(IMPORT_SUBJECTS),
         "subjects %r%s" % (sorted(rows) if rows is not None else None, "" if "rows_before" not in answer else "; the list on the driver's first read, before any wait: %r" % (answer["rows_before"],))),
    ]


IMPORT_CHECKS = ("run-stored-every-message-and-the-history-says-so", "list-shows-the-imported-messages")


def drive4():
    """Inside launch-one: the rig core imports by itself at open. Read `import/sources` until it says 3 imported (or the wait ends), then the list."""
    import json
    sys.path.insert(0, os.path.join(ROOT, "journeys"))
    from lib.ctl import Ctl  # noqa: E402
    c = Ctl().connect()
    answer = {}
    sources = None
    t0 = time.time()
    while time.time() - t0 < IMPORT_WAIT_S:
        try:
            sources = c.core("import/sources")
            found = next((x for x in (json.loads(sources) if isinstance(sources, str) else (sources or [])) if x.get("id") == IMPORT_SOURCE), {})
            if found.get("imported_count") == len(IMPORT_SUBJECTS):
                break
        except Exception:
            pass
        time.sleep(0.5)
    answer["sources"] = sources
    # The app refreshes its own list when the core stores mail (shell-refresh: Sources/App/CoreChanges.swift subscribes to the core's
    # streams). No click, no command, no driver re-read: the driver only READS `mac-main.list` and waits up to IMPORT_LIST_WAIT_S for it to
    # fill. The list as it stood on the first read is kept beside the final one, so a list that fills late shows in the answer.
    try:
        answer["rows_before"] = sorted(r.get("subject") for r in c.rows("mac-main.list"))
    except Exception:
        answer["rows_before"] = None
    t0 = time.time()
    while True:
        try:
            answer["rows"] = {r.get("subject"): True for r in c.rows("mac-main.list")}
        except Exception:
            answer["rows"] = None   # the list is not on screen (no account row to open the window on): not judged, never guessed
        if answer["rows"] is None or len(answer["rows"]) >= len(IMPORT_SUBJECTS) or time.time() - t0 > IMPORT_LIST_WAIT_S:
            break
        time.sleep(0.3)
    answer["core"] = c.uistate().get("core")
    return answer


def main_slice4(bundle, mutant):
    """A folder stands in for the home folder; the rig core reads it through WHOOSH_CORE_CHECK_IMPORT."""
    import json, shutil, tempfile
    home = tempfile.mkdtemp(prefix="whoosh-import-check-")
    try:
        build_import_fixture(home)
        extra = ["WHOOSH_CORE_CHECK_IMPORT=" + home] + (["WHOOSH_CORE_MUTANT=import-never-stores"] if mutant else [])
        env = dict(os.environ, WHOOSH_RIG_HIDDEN="1", LAUNCH_ENV=" ".join(["WHOOSH_RIG_HIDDEN=1"] + extra))
        env.pop("WHOOSH_FIXTURE_CORE", None)
        cmd = ["sh", os.path.join(ROOT, "scripts", "launch-one.sh"), bundle, "--", "--rig-hidden",
               "sh", "-c", 'WHOOSH_SOCKET_DIR="$D" python3 "$0" --inner --slice4', os.path.abspath(__file__)]
        r = launch(cmd, env)
    finally:
        shutil.rmtree(home, ignore_errors=True)
    out = r.stdout + r.stderr
    line = next((l for l in out.splitlines() if l.startswith("ANSWER ")), None)
    if line is None:
        print(out.rstrip()[-3000:])
        print("launcher exit %d" % r.returncode)
        print("1 checks, 0 failed, 1 not judged"); return 1
    answer = json.loads(line[len("ANSWER "):])
    print("launcher exit %d; %s" % (r.returncode, next((l for l in out.splitlines() if l.startswith("instances:")), "")))
    results = judge4(answer)
    lines = ["%-10s %s  %s" % ("PASS" if ok else ("NOT JUDGED" if ok is None else "FAIL"), n, d) for n, ok, d in results]
    failed = sum(1 for _, ok, _ in results if ok is False)
    unjudged = sum(1 for _, ok, _ in results if ok is None)
    if mutant:
        red = all(ok is False for n, ok, _ in results if n == IMPORT_CHECKS[0]) and all(ok is not True for n, ok, _ in results if n == IMPORT_CHECKS[1])
        lines.append("mutant import-never-stores: %s" % ("RED as it must be (nothing stored, the history stays at 0, the list stays empty)" if red else "SURVIVED: the run showed messages an import that stores nothing never wrote"))
        lines.append("%d checks, %d failed, %d not judged (mutant run)" % (len(results), failed, unjudged))
        print("\n".join(lines)); return 0 if red else 1
    lines.append("%d checks, %d failed, %d not judged" % (len(results), failed, unjudged))
    print("\n".join(lines)); return 0 if failed == 0 else 1


def selftest4():
    """The judge against a model answer and the never-stores model, and the fixture tree against its own reader (no app)."""
    import json, shutil, tempfile
    src = {"id": IMPORT_SOURCE, "kind": "apple-mail", "access": "readable", "message_count": 3, "imported_count": 3, "last_run_ms": 1}
    good = {"core": "rust", "sources": json.dumps([src]), "rows": {s: True for s in IMPORT_SUBJECTS}}
    never = {"core": "rust", "sources": json.dumps([dict(src, imported_count=0, last_run_ms=None)]), "rows": {}}
    ok = all(x for _, x, _ in judge4(good)) and all(x is False for n, x, _ in judge4(never) if n in IMPORT_CHECKS)
    home = tempfile.mkdtemp(prefix="whoosh-import-selftest-")
    try:
        folder = build_import_fixture(home)
        names = sorted(os.listdir(folder))
        sizes_ok = True
        for n in names:
            data = open(os.path.join(folder, n), "rb").read()
            head, _, rest = data.partition(b"\n")
            sizes_ok = sizes_ok and rest[:int(head)].endswith(b".\n") and rest[int(head):].startswith(b"<?xml")
    finally:
        shutil.rmtree(home, ignore_errors=True)
    print("selftest slice 4: model %s, never-stores model %s, fixture tree %s" % ("all pass" if all(x for _, x, _ in judge4(good)) else "FAILS", "red" if ok else "SURVIVES", "3 files, sizes right" if names == ["1.emlx", "2.emlx", "3.emlx"] and sizes_ok else "WRONG"))
    return ok and names == ["1.emlx", "2.emlx", "3.emlx"] and sizes_ok


def main_slice2b(bundle, mutant):
    """The local mail servers and the local authority live in THIS process (threads) for the whole launch."""
    import json
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import realcore_mailfake as fake
    import realcore_authfake as authority
    try:
        mail, imap, smtp = fake.start(0, 0)
        auth_state, auth = authority.start(0)
    except OSError as e:
        print("the local servers did not start: %s" % e)
        print("1 checks, 0 failed, 1 not judged"); return 1
    ports = "%d:%d" % (imap.server_address[1], smtp.server_address[1])
    extra = ["WHOOSH_CORE_PLAIN_LOOPBACK=1", "WHOOSH_CORE_CHECK_SERVERS=" + ports, "WHOOSH_CORE_CHECK_AUTHORITY=%d" % auth.server_address[1]]
    if mutant:
        extra.append("WHOOSH_CORE_MUTANT=" + mutant)
    env = dict(os.environ, WHOOSH_RIG_HIDDEN="1", LAUNCH_ENV=" ".join(["WHOOSH_RIG_HIDDEN=1"] + extra))
    env.pop("WHOOSH_FIXTURE_CORE", None)
    cmd = ["sh", os.path.join(ROOT, "scripts", "launch-one.sh"), bundle, "--", "--rig-hidden",
           "sh", "-c", 'WHOOSH_SOCKET_DIR="$D" python3 "$0" --inner --slice2b', os.path.abspath(__file__)]
    r = launch(cmd, env)
    imap.shutdown(); smtp.shutdown(); auth.shutdown()
    out = r.stdout + r.stderr
    line = next((l for l in out.splitlines() if l.startswith("ANSWER ")), None)
    if line is None:
        print(out.rstrip()[-3000:])
        print("launcher exit %d" % r.returncode)
        print("1 checks, 0 failed, 1 not judged"); return 1
    answer = json.loads(line[len("ANSWER "):])
    answer["authority_log"], answer["mail_log"], answer["smtp_auths"] = list(auth_state["log"]), list(mail.log), list(smtp.auths)
    print("launcher exit %d; %s" % (r.returncode, next((l for l in out.splitlines() if l.startswith("instances:")), "")))
    return report2b(judge2b(answer), mutant)


def report2b(results, mutant):
    lines = ["%-10s %s  %s" % ("PASS" if ok else "FAIL", n, d) for n, ok, d in results]
    failed = sum(1 for _, ok, _ in results if not ok)
    if mutant:
        red = any(not ok for n, ok, _ in results if n in SIGN_IN_CHECKS)
        lines.append("mutant %s: %s" % (mutant, "RED as it must be (a sign-in step removed shows on the sheet or the list)" if red else "SURVIVED: every sign-in check passed with a step removed"))
        lines.append("%d checks, %d failed, 0 not judged (mutant run)" % (len(results), failed))
        print("\n".join(lines)); return 0 if red else 1
    lines.append("%d checks, %d failed, 0 not judged" % (len(results), failed))
    print("\n".join(lines)); return 0 if failed == 0 else 1


def selftest2b():
    t = "common"
    good = {"stage": "added", "phase": "done", "core": "rust", "oauth": {"kind": "microsoft"}, "counts": {"primary": 3},
            "list": [{"subject": "Check message %d" % n, "account": "acc-1"} for n in (3, 2, 1)],
            "authority_log": ["GET /%s/oauth2/v2.0/authorize" % t, "POST /%s/oauth2/v2.0/token authorization_code" % t],
            "mail_log": ["CAPABILITY ", "AUTHENTICATE XOAUTH2 (hidden)", "LIST \"\" \"*\""], "smtp_auths": ["XOAUTH2"]}
    never_exchanges = dict(good, stage="summary", phase="refused", list=[], counts={}, mail_log=[], smtp_auths=[],
                           authority_log=good["authority_log"][:1])
    elsewhere = dict(good, authority_log=[l.replace(t, "whoosh-check.invalid") for l in good["authority_log"]], stage="summary", list=[], counts={})
    password = dict(good, mail_log=["LOGIN (hidden)"], stage="summary", list=[], counts={})
    g = judge2b(good)
    ok = all(x for _, x, _ in g)
    for bad in (never_exchanges, elsewhere, password):
        ok = ok and any(not x for n, x, _ in judge2b(bad) if n in SIGN_IN_CHECKS)
    print("selftest slice 2b: model %s, mutant models %s" % ("all pass" if all(x for _, x, _ in g) else "FAILS", "red" if ok else "SURVIVE"))
    return ok


def report(results, mutant):
    lines = []
    for n, ok, d in results:
        lines.append("%-10s %s  %s" % ("PASS" if ok else ("NOT JUDGED" if ok is None else "FAIL"), n, d))
    failed = sum(1 for _, ok, _ in results if ok is False)
    unjudged = sum(1 for _, ok, _ in results if ok is None)
    if mutant:
        ms = [ok for n, ok, _ in results if n in ("incoming-server-is-microsoft-365", "found-by-a-microsoft-signal", "provider-mark-is-microsoft", "app-id-field-shown")]
        red = all(ok is False for ok in ms) and ms
        lines.append("mutant no-microsoft: %s" % ("RED as it must be (no Microsoft 365 without Autodiscover and the mail record)" if red else "SURVIVED: Microsoft 365 found with both Microsoft signals removed"))
        lines.append("%d checks, %d failed, %d not judged (mutant run)" % (len(results), failed, unjudged))
        print("\n".join(lines)); return 0 if red else 1
    lines.append("%d checks, %d failed, %d not judged" % (len(results), failed, unjudged))
    print("\n".join(lines)); return 0 if failed == 0 and unjudged == 0 else 1


def selftest2():
    good = {"stage": "added", "core": "rust", "counts": {"primary": 3, "all": 3, "all inboxes": 3}, "list": [{"subject": "Check message %d" % n, "account": "acc-1"} for n in (3, 2, 1)]}
    bad = dict(good, list=[], counts={"primary": 0})
    nokey = dict(good, counts={"primary": 3, "all": 3})          # the store before the fix: no "all inboxes" figure
    g, b, k = judge2(good), judge2(bad), judge2(nokey)
    ok = all(x for _, x, _ in g) and all(not x for n, x, _ in b if n in ("list-shows-the-synced-inbox", "rows-belong-to-the-added-account", "counts-above-zero")) \
        and [n for n, x, _ in k if not x] == ["all-inboxes-is-the-all-tabs-unread"]
    print("selftest slice 2: model %s, never-stores model %s" % ("all pass" if all(x for _, x, _ in g) else "FAILS", "red" if ok else "SURVIVES"))
    return ok



def selftest_detect():
    """The detector's judges and the outbound case against model answers; every scenario and the case go red on their own mutant."""
    import contextlib
    import io
    import tempfile
    steps = lambda *ids: [{"id": i, "outcome": "miss"} for i in ids[:-1]] + [{"id": ids[-1], "outcome": "found"}]
    good = {"m365-by-mail-record": {"stage": "summary", "found": [{"imap": "outlook.office365.com:993", "smtp": "smtp.office365.com:587"}], "detection.step": steps("ispdb", "autoconfig", "srv", "autodiscover", "mx"), "oauth": [{"kind": "microsoft"}]},
            "google-by-mail-record": {"stage": "summary", "found": [{"imap": "imap.gmail.com:993", "smtp": "smtp.gmail.com:587"}], "detection.step": steps("ispdb", "autoconfig", "srv", "autodiscover", "mx")},
            "provider-database": {"stage": "summary", "found": [{"imap": "imap.gmail.com:993", "smtp": "smtp.gmail.com:465"}], "detection.step": steps("ispdb")},
            "srv-records": {"stage": "summary", "found": [{"imap": "imap.srv-check.test:993", "smtp": "smtp.srv-check.test:587"}], "detection.step": steps("ispdb", "autoconfig", "srv")},
            "own-autodiscover": {"stage": "summary", "found": [{"imap": "mail.onprem.example:993", "smtp": "mail.onprem.example:587"}], "detection.step": steps("ispdb", "autoconfig", "srv", "autodiscover")},
            "plain-host-by-guess": {"stage": "summary", "found": [{"imap": "imap.plain-check.test:143", "smtp": "mail.plain-check.test:465"}], "detection.step": steps("ispdb", "autoconfig", "srv", "autodiscover", "mx", "guess")},
            "no-such-domain": {"stage": "summary", "found": [], "detection.step": [{"id": "ispdb", "outcome": "miss"}, {"id": "autoconfig", "outcome": "miss"}, {"id": "srv", "outcome": "miss"}], "error": [{"text": "That domain does not exist."}]}}
    ok = True
    g = judge_detect(good)
    clean = all(x for _, x, _ in g) and len(g) == len(SCENARIOS)
    print("selftest detect: model %s (%d scenarios)" % ("all pass" if clean else "FAILS " + str([n for n, x, _ in g if not x]), len(g)))
    ok &= clean
    import copy
    mutants = {"m365-by-mail-record": lambda a: a.update(oauth=[]),
               "google-by-mail-record": lambda a: a.update(found=[{"imap": "imap.google-check.test:993", "smtp": "smtp.google-check.test:587"}], **{"detection.step": steps("ispdb", "autoconfig", "srv", "autodiscover", "mx", "guess")}),
               "provider-database": lambda a: a.update(**{"detection.step": steps("ispdb", "autoconfig")}),
               "srv-records": lambda a: a.update(found=[{"imap": "backup-imap.srv-check.test:993", "smtp": "smtp.srv-check.test:587"}]),
               "own-autodiscover": lambda a: a.update(found=[]),
               "plain-host-by-guess": lambda a: a.update(**{"detection.step": [{"id": "guess", "outcome": "miss"}]}),
               "no-such-domain": lambda a: a.update(found=[{"imap": "imap.nx-check.test:993", "smtp": "smtp.nx-check.test:587"}], stage="added")}
    for label, mut in mutants.items():
        bad = copy.deepcopy(good); mut(bad[label])
        r = judge_detect(bad)
        red = [n for n, x, _ in r if not x]
        good_one = red == [label]
        print("selftest detect: mutant on %s: %s" % (label, "red on that case only" if good_one else "WRONG: red %r" % red))
        ok &= good_one
    # the outbound case
    for line, want in [(None, None), ('REFUSED {"count":0,"last_host":null}', True), ('REFUSED {"count":2,"last_host":"autodiscover.nexus-example.com"}', False), ("REFUSED unreadable: no value", None), ("REFUSED 0", True), ("REFUSED 4", False)]:
        got = outside_case(line)[1]
        good_one = got is want
        print("selftest outside: %r -> %r %s" % (line, got, "ok" if good_one else "WRONG (want %r)" % want))
        ok &= good_one
    named = outside_case('REFUSED {"count":2,"last_host":"autodiscover.nexus-example.com"}')[2]
    ok &= "autodiscover.nexus-example.com" in named
    # the wrapper adds one case and a refusal fails the run whatever the slice said
    def slice_says(n, f, u):
        def run():
            print("%d checks, %d failed, %d not judged" % (n, f, u)); return 0 if f == 0 else 1
        return run
    for refused, want_line, want_rc in [('REFUSED {"count":0}', "9 checks, 0 failed, 0 not judged", 0), ('REFUSED {"count":3,"last_host":"h"}', "9 checks, 1 failed, 0 not judged", 1), ("REFUSED unreadable: x", "9 checks, 0 failed, 1 not judged", 0)]:
        LAST["out"] = "ANSWER {}\n" + refused
        buf = io.StringIO()
        with contextlib.redirect_stdout(buf):
            rc = with_outside_case(slice_says(8, 0, 0))
        last = buf.getvalue().strip().splitlines()[-1]
        good_one = last == want_line and rc == want_rc
        print("selftest outside wrapper: %s -> %r exit %d %s" % (refused[:40], last, rc, "ok" if good_one else "WRONG"))
        ok &= good_one
    lnc = [local_network_case([(15, "a")], [("GET", "h", "/p")])[1], local_network_case([], [("GET", "h", "/p")])[1], local_network_case([(15, "a")], [])[1]]
    good_one = lnc == [True, False, False]
    print("selftest detect: the local-network control is true only when DNS and HTTP were both asked: %s" % ("ok" if good_one else "WRONG %r" % lnc))
    ok &= good_one
    # a build without the detector seam is NOT JUDGED and nothing is launched
    d = tempfile.mkdtemp(prefix="detect-bundle-"); os.makedirs(os.path.join(d, "Contents", "MacOS"))
    open(os.path.join(d, "Contents", "MacOS", "App"), "wb").write(b"no seam here")
    global launch
    real = launch
    launched = []
    launch = lambda cmd, env: launched.append(cmd)
    try:
        buf = io.StringIO()
        with contextlib.redirect_stdout(buf):
            rc = main_slice1(d, False)
        said = buf.getvalue()
    finally:
        launch = real
    good_one = rc == 1 and not launched and "NOT JUDGED" in said and "WHOOSH_CORE_CHECK_DETECT" in said
    print("selftest detect: a build without the seam is NOT JUDGED and nothing launches: %s" % ("ok" if good_one else "WRONG %r %r" % (rc, launched)))
    ok &= good_one
    open(os.path.join(d, "Contents", "MacOS", "App"), "wb").write(b"x WHOOSH_CORE_CHECK_DETECT y")
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import realcore_netfake as nf
    good_one = nf.bundle_has(d, "WHOOSH_CORE_CHECK_DETECT")
    print("selftest detect: the seam's name in the bundle is found (control for the line above): %s" % ("ok" if good_one else "WRONG"))
    ok &= good_one
    return ok


def selftest():
    if not selftest_detect() or not selftest2() or not selftest2b() or not selftest3() or not selftest3b() or not selftest4() or not selftest_bye() or not selftest5() or not selftest6() or not selftest_search() or not selftest_labelclick() or not selftest_accountmark() or not selftest_mailview() or not selftest_labels() or not selftest_moved() or not selftest_body() or not selftest_rulecount() or not selftest_crawl() or not selftest_aikey() or not selftest_aisort() or not selftest_attachsend() or not selftest_weights() or not selftest_argv() or not selftest_bodygone() or not selftest_labeltab() or not selftest_sendfail() or not selftest_sources() or not selftest_vip():
        return 1
    good = {"stage": "summary", "added": 0, "found": [{"imap": "outlook.office365.com:993", "smtp": "smtp.office365.com:587"}],
            "detection.step": [{"id": s, "outcome": "miss"} for s in ("ispdb", "autoconfig", "srv", "autodiscover")] + [{"id": "mx", "outcome": "found"}],
            "provider-note": [{"note": "microsoft-no-password"}], "oauth": [{"kind": "microsoft", "client_saved": False}],
            "app_id_field": {"focusable": True}, "marks": [{"placement": "found", "kind": "microsoft"}]}
    bad = {"stage": "summary", "added": 0, "found": [{"imap": "imap.nexus-example.com:993", "smtp": "smtp.nexus-example.com:587"}],
           "detection.step": [{"id": "guess", "outcome": "found"}], "provider-note": [], "oauth": [], "app_id_field": {"focusable": False}, "marks": [{"placement": "found", "kind": "imap"}]}
    g = judge(good); b = judge(bad)
    ok = all(x for _, x, _ in g) and all(not x for n, x, _ in b if n not in ("detection-finished", "no-account-added"))
    print("selftest: model %s, mutant model %s" % ("all pass" if all(x for _, x, _ in g) else "FAILS", "red" if ok else "SURVIVES"))
    return 0 if ok else 1


BYE_WORDS = "Connection closed. 14"
REFUSE_WORDS = "Command Argument Error. 11"


def bye_handler(fake, mail, mode):
    """The check fake's IMAP, plus one fault. mode "bye": the first FETCH answer goes out, then "* BYE <words>" and the
    connection is closed (what the owner's Microsoft 365 did). mode "refuse": every UID SEARCH is answered BAD."""
    class ImapFault(fake.Imap):
        dead = False

        def send(self, b):
            if self.dead:
                return
            text = b if isinstance(b, str) else b.decode("latin-1")
            if mode == "bye" and text.startswith("* ") and " FETCH (" in text.split("\r\n", 1)[0]:
                with mail.lock:
                    play = not mail.bye_played
                    mail.bye_played = mail.bye_played or play
                if play:
                    fake.Imap.send(self, b)
                    fake.Imap.send(self, "* BYE %s\r\n" % BYE_WORDS)
                    with mail.lock:
                        mail.log.append("BYE played")
                    self.dead = True
                    try:
                        self.connection.shutdown(2)
                    except OSError:
                        pass
                    return
            if mode == "refuse" and text.startswith("* SEARCH"):
                tag = text.rstrip("\r\n").rsplit("\r\n", 1)[-1].split(" ")[0]
                fake.Imap.send(self, "%s BAD %s\r\n" % (tag, REFUSE_WORDS))
                return
            fake.Imap.send(self, b)
    return type("ImapFaultFor", (ImapFault,), {"mail": mail})


def judge_bye(answer, played, logins_after, server_errors=()):
    """--slice-bye: the cut FETCH was retried on a new connection and every message reached the list. The local server
    itself must have kept serving: a handler that died (sync-bye-r2: IDLE closed a second in, so the status read
    "Reconnecting in 32 s") is the fake's failure, named as such, never read as the app's."""
    rows = answer.get("list") or []
    subjects = sorted(r.get("subject", "") for r in rows)
    status = (answer.get("status") or [{}])[0]
    return [
        ("the-local-server-kept-serving", not server_errors, "handlers that died: %d%s" % (len(server_errors), (", first: " + server_errors[0]) if server_errors else "")),
        ("account-added", answer.get("stage") == "added", "stage %r, error %r" % (answer.get("stage"), answer.get("error"))),
        ("the-server-cut-a-fetch-with-bye", played, "BYE played %r" % played),
        ("a-new-connection-finished-the-folder", logins_after >= 1, "sign-ins after the BYE: %d" % logins_after),
        ("list-shows-every-message-after-the-bye", subjects == ["Check message 1", "Check message 2", "Check message 3"] and answer.get("driver_refresh") is not True,
         "subjects %r; the list filled on its own (the driver only read it, no click, no refresh) in %ss of a %ds budget" % (subjects, answer.get("seconds"), SYNC_WAIT_S)),
        ("no-account-read-as-failing", status.get("failing_account") is None, "status %r" % status.get("text")),
    ]


def judge_refuse(answer):
    """--slice-bye --refuse: the failure reaches the owner as the step and the server's words, never as an empty 'done'.
    Since sync-words the status line is ONE catalogue sentence; the engine's words (the step, the server's reply) are served only
    under Details: the `details` field of the step row under the added account (`mac-add-account.step`, read while the sheet shows
    the failed account). Nothing serves them on `mac-main.status` (the shell draws them as a tooltip)."""
    rows = answer.get("list") or []
    status = (answer.get("status") or [{}])[0]
    text = status.get("text") or ""
    step = answer.get("step") or {}
    details = step.get("details") or ""
    return [
        ("account-added", answer.get("stage") == "added", "stage %r, error %r" % (answer.get("stage"), answer.get("error"))),
        ("status-names-the-failing-account", status.get("failing_account") is not None, "status %r" % status),
        ("step-reads-failed", step.get("state") == "failed", "step row %r" % step),
        ("details-name-the-step-and-the-servers-words", "could not fetch INBOX" in details and REFUSE_WORDS in details, "details %r" % details),
        ("status-is-the-catalogue-sentence-not-the-engines-words", "fetch the inbox" in text.lower() and REFUSE_WORDS not in text and "could not fetch INBOX" not in text,
         "text %r (control: the sentence must be there, the engine's words must not)" % text),
        ("list-stays-empty", rows == [], "%d rows" % len(rows)),
    ]


def drive_bye():
    """Inside launch-one: drive2's add and list wait, then the status line the shell draws. With --refuse the step row under the
    added account is read first, once it says failed (that row carries the engine's words behind Details)."""
    sys.path.insert(0, os.path.join(ROOT, "journeys"))
    from lib.ctl import Ctl  # noqa: E402

    def read_step(c, answer):
        t0 = time.time()
        step = []
        while time.time() - t0 < SYNC_WAIT_S:
            try:
                step = c.rows(A + ".step")
            except Exception:
                step = []
            if step and step[0].get("state") == "failed":
                break
            time.sleep(0.5)
        answer["step"] = {k: step[0].get(k) for k in ("text", "state", "details")} if step else {}

    answer = drive2(read_step if "--refuse" in sys.argv else None)    # drive2 only READS the list while it waits (c.rows): the list must fill by itself, so no driver refresh happened
    answer["driver_refresh"] = False
    c = Ctl().connect()
    t0 = time.time()
    status = []
    while time.time() - t0 < SYNC_WAIT_S:
        status = c.rows("mac-main.status")
        if "--refuse" not in sys.argv or (status and status[0].get("failing_account")):
            break
        time.sleep(0.5)
    answer["status"] = status
    return answer


def main_slice_bye(bundle, refuse):
    import json
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import realcore_mailfake as fake
    import threading
    mail = fake.Mail(3)
    mail.bye_played = False
    try:
        imap = fake.Server(("127.0.0.1", 0), bye_handler(fake, mail, "refuse" if refuse else "bye"))
        smtp = fake.Server(("127.0.0.1", 0), fake.Smtp)
    except OSError as e:
        print("the local servers did not start: %s" % e)
        print("1 checks, 0 failed, 1 not judged"); return 1
    for srv in (imap, smtp):
        threading.Thread(target=srv.serve_forever, daemon=True).start()
    ports = "%d:%d" % (imap.server_address[1], smtp.server_address[1])
    extra = ["WHOOSH_CORE_PLAIN_LOOPBACK=1", "WHOOSH_CORE_CHECK_SERVERS=" + ports]
    env = dict(os.environ, WHOOSH_RIG_HIDDEN="1", LAUNCH_ENV=" ".join(["WHOOSH_RIG_HIDDEN=1"] + extra))
    env.pop("WHOOSH_FIXTURE_CORE", None)
    inner = 'WHOOSH_SOCKET_DIR="$D" python3 "$0" --inner --slice-bye' + (" --refuse" if refuse else "")
    cmd = ["sh", os.path.join(ROOT, "scripts", "launch-one.sh"), bundle, "--", "--rig-hidden", "sh", "-c", inner, os.path.abspath(__file__)]
    r = launch(cmd, env)
    imap.shutdown(); smtp.shutdown()
    out = r.stdout + r.stderr
    line = next((l for l in out.splitlines() if l.startswith("ANSWER ")), None)
    if line is None:
        print(out.rstrip()[-3000:])
        print("launcher exit %d" % r.returncode)
        print("1 checks, 0 failed, 1 not judged"); return 1
    answer = json.loads(line[len("ANSWER "):])
    log = list(mail.log)
    after = log[log.index("BYE played") + 1:] if "BYE played" in log else []
    logins_after = sum(1 for l in after if l.split(" ")[0] in ("LOGIN", "AUTHENTICATE"))
    results = judge_refuse(answer) if refuse else judge_bye(answer, mail.bye_played, logins_after, list(imap.errors))
    print("launcher exit %d; the local server saw: %s" % (r.returncode, sorted({l.split(" ")[0] for l in log})))
    lines = ["%-10s %s  %s" % ("PASS" if ok else "FAIL", n, d) for n, ok, d in results]
    failed = sum(1 for _, ok, _ in results if not ok)
    lines.append("%d checks, %d failed, 0 not judged" % (len(results), failed))
    print("\n".join(lines)); return 0 if failed == 0 else 1


def selftest_bye():
    """The two judges against a model answer and the behaviour before the fix (the BYE read as an empty, finished folder)."""
    added = {"stage": "added", "error": None}
    good = dict(added, list=[{"subject": "Check message %d" % n} for n in (3, 2, 1)], status=[{"text": "1 of 1 accounts synced", "failing_account": None}])
    before = dict(added, list=[], status=[{"text": "1 of 1 accounts synced", "failing_account": None}])
    g, b = judge_bye(good, True, 1), judge_bye(before, True, 0)
    ok_bye = all(x for _, x, _ in g) and all(not x for n, x, _ in b if n in ("a-new-connection-finished-the-folder", "list-shows-every-message-after-the-bye"))
    # sync-bye-r2: main53's run, the fake's IDLE handler dying on every connection: its own check goes red, by name
    dead = dict(added, list=[], status=[{"text": "New mail is paused: connection lost. Reconnecting in 32 s.", "failing_account": "acc-1"}])
    d = dict((n, x) for n, x, _ in judge_bye(dead, True, 6, ["OSError: cannot read from timed out object"] * 5))
    ok_bye = ok_bye and d["the-local-server-kept-serving"] is False and dict((n, x) for n, x, _ in g)["the-local-server-kept-serving"] is True
    sentence = "owner@whoosh-check.invalid Could not fetch the inbox."
    stat = [{"text": sentence, "failing_account": "acc-1"}]
    step = {"state": "failed", "text": "Could not fetch the inbox.", "details": "could not fetch INBOX: %s" % REFUSE_WORDS}
    refused = dict(added, list=[], status=stat, step=step)
    before_fix = dict(added, list=[], status=[{"text": "Owner could not fetch INBOX: %s" % REFUSE_WORDS, "failing_account": "acc-1"}])   # engine words in the sentence, no step row
    gr = judge_refuse(refused)
    red = lambda ans, names: all(not dict((n, x) for n, x, _ in judge_refuse(ans))[k] for k in names)
    ok_refuse = (all(x for _, x, _ in gr)
                 and red(dict(refused, step=dict(step, details="")), ("details-name-the-step-and-the-servers-words",))          # Details empty (the words lost)
                 and red(dict(refused, step=dict(step, details="could not fetch INBOX")), ("details-name-the-step-and-the-servers-words",))   # the server's reply missing
                 and red(dict(refused, step={}), ("step-reads-failed", "details-name-the-step-and-the-servers-words"))        # nothing served
                 and red(dict(refused, status=[{"text": sentence + " " + REFUSE_WORDS, "failing_account": "acc-1"}]), ("status-is-the-catalogue-sentence-not-the-engines-words",))   # engine words in the sentence
                 and red(dict(refused, status=[{"text": "", "failing_account": "acc-1"}]), ("status-is-the-catalogue-sentence-not-the-engines-words",))   # empty text: the absence check has its control
                 and red(before_fix, ("step-reads-failed", "details-name-the-step-and-the-servers-words", "status-is-the-catalogue-sentence-not-the-engines-words")))
    print("selftest slice bye: model %s, before-the-fix model %s; refuse model %s, before-the-fix model %s" % (
        "all pass" if all(x for _, x, _ in g) else "FAILS", "red" if ok_bye else "SURVIVES",
        "all pass" if all(x for _, x, _ in gr) else "FAILS", "red" if ok_refuse else "SURVIVES"))
    return ok_bye and ok_refuse


# ---------------------------------------------------------------- slice 5 (engine-act): the real core acts on mail

REPLY_KEPT = "Thanks, noted for the plan."
REPLY_UNDONE = "Ignore this, I take it back."
REPLY_AGAIN = "One more thing, the plan moved."
FIELD = "mac-main.quickreply.field"
ACT_WAIT_S = 60
ACT_CHECKS = ("archive-reached-the-server", "star-reached-the-server", "marked-junk-reached-the-servers-junk-folder",
              "mark-not-spam-moves-the-message-back-to-the-servers-inbox")


def undo_window_s():
    """UNDO_SEND_DEFAULT_S from the ledger (docs/CONSTANTS.md), never retyped."""
    for line in open(os.path.join(ROOT, "docs", "CONSTANTS.md"), encoding="utf-8"):
        if line.startswith("| UNDO_SEND_DEFAULT_S"):
            return int(line.split("|")[2].split("·")[0].strip())
    raise SystemExit("docs/CONSTANTS.md has no UNDO_SEND_DEFAULT_S row")


def judge5(answer, server):
    """`answer` is what the app showed (inside launch-one); `server` is what the local IMAP and SMTP sides received (this process)."""
    rows0 = answer.get("rows_before") or []
    acted = answer.get("acted") or {}
    after = [r.get("subject") for r in answer.get("rows_after") or []]
    moves, stores, appends, sent = server.get("moves") or [], server.get("stores") or [], server.get("appends") or [], server.get("smtp") or []
    arch_uid, snooze_uid, star_uid = (acted.get(k, {}).get("uid") for k in ("archive", "snooze", "star"))
    archived = any(m[0] == "INBOX" and arch_uid in m[1] and m[2] == "Archive" for m in moves) or any(arch_uid in st[1] and "\\Archived" in st[3] for st in stores)
    starred = any(st[0] == "INBOX" and star_uid in st[1] and st[2].startswith("+") and "\\Flagged" in st[3] for st in stores)
    snooze_touched = any(snooze_uid in m[1] for m in moves) or any(snooze_uid in st[1] for st in stores)
    kept = [m for m in sent if REPLY_KEPT in m]
    thread = answer.get("thread") or []
    # the sent copy is the thread entry FROM the receiving address (the originals come from "Sender n") or the one carrying the typed words
    joined = [t for t in thread if CHECK_ADDRESS in ((t.get("from") or ""), (t.get("sender") or "")) or REPLY_KEPT in (t.get("text") or "")]
    boxes = server.get("boxes") or {}
    spam_subject, spam_uid = (acted.get("spam") or {}).get("subject"), (acted.get("spam") or {}).get("uid")
    unarch = answer.get("unarchive") or {}
    unarch_subject = acted.get("archive", {}).get("subject")
    notspam = answer.get("not_spam") or {}
    # engine-act-r6: Not spam brings the junk message back from the server's Junk folder (UIDs there are the server's own: the source folder decides)
    back_from_junk = any(m[0] == "Junk" and m[2] == "INBOX" for m in moves)
    not_spam_back = (bool(spam_subject) and spam_subject in (boxes.get("INBOX") or []) and spam_subject not in (boxes.get("Junk") or []) and back_from_junk)
    # the junk move landed: the message sat in Junk at the end, or Not spam moved it out of Junk afterwards
    junked = bool(spam_uid) and any(m[0] == "INBOX" and spam_uid in m[1] and m[2] == "Junk" for m in moves) and (spam_subject in (boxes.get("Junk") or []) or back_from_junk)
    unarch_back = (bool(unarch_subject) and unarch_subject in (boxes.get("INBOX") or []) and unarch_subject not in (boxes.get("Archive") or [])
                   and any(m[0] == "Archive" and m[2] == "INBOX" for m in moves))
    unarch_flagged_only = any(st[0] == "INBOX" and arch_uid in st[1] and st[2].startswith("-") and "\\Archived" in st[3] for st in stores)
    return [
        ("account-added", answer.get("stage") == "added", "stage %r, error %r" % (answer.get("stage"), answer.get("error"))),
        ("core-is-rust", answer.get("core") in (None, "rust"), "uistate core %r" % answer.get("core")),
        ("first-start-flow-ended-before-the-mail-keys", (answer.get("first_start") or {}).get("active") is False, "first start %r" % answer.get("first_start")),
        ("list-shows-the-three-inbox-messages", len(rows0) == 3, "rows %r" % [r.get("subject") for r in rows0]),
        ("archive-reached-the-server", bool(arch_uid) and archived, "archived uid %r; moves %r; stores %r" % (arch_uid, moves, stores)),
        ("archived-row-left-the-list", acted.get("archive", {}).get("subject") not in after, "after %r" % after),
        ("snooze-hides-the-row-and-leaves-the-server-alone", acted.get("snooze", {}).get("subject") not in after and bool(snooze_uid) and not snooze_touched,
         "after %r; snoozed uid %r touched %r" % (after, snooze_uid, snooze_touched)),
        ("star-reached-the-server", bool(star_uid) and starred, "starred uid %r; stores %r" % (star_uid, stores)),
        ("the-reply-reached-smtp-once-threaded", len(kept) == 1 and "In-Reply-To:" in kept[0], "%d SMTP messages, %d with the reply" % (len(sent), len(kept))),
        ("the-sent-copy-reached-the-sent-folder", any(a[0] == "Sent" and REPLY_KEPT in a[2] for a in appends), "appends to %r" % [a[0] for a in appends]),
        ("the-sent-copy-joins-its-thread", len(joined) == 1, "thread %r" % thread),
        ("undo-send-was-pending-when-z-came", answer.get("undo_bar_before_z") is True, "undo bar 'Sending' before Z: %r (the control: without it the zero below proves nothing)" % answer.get("undo_bar_before_z")),
        ("the-undone-send-never-left", not any(REPLY_UNDONE in m for m in sent) and answer.get("undo_bar_after_z") is False, "SMTP messages with the undone words: %d; bar after Z %r" % (sum(1 for m in sent if REPLY_UNDONE in m), answer.get("undo_bar_after_z"))),
        ("enter-opens-the-thread-on-the-real-core", answer.get("enter_opens_thread") is True,
         "Enter on the focused row: %s; the app said %r" % ("opened" if answer.get("enter_opens_thread") else "did nothing", answer.get("enter_failure_words"))),
        ("a-second-reply-in-the-same-thread-is-accepted", answer.get("again_bar") is True,
         "undo bar 'Sending' after the second R and ⌘Enter: %r; the quick reply says %r" % (answer.get("again_bar"), (answer.get("again_retry") or [{}])[0].get("words") if answer.get("again_retry") else None)),
        # engine-act-r4 (delta 19's server moves, read back from the fake's boxes and moves, not from the core's pending rows)
        ("marked-junk-reached-the-servers-junk-folder", junked,
         "junk uid %r (%r); moves %r; server boxes %r" % (spam_uid, spam_subject, moves, {k: v for k, v in boxes.items() if k in ("INBOX", "Junk")})),
        ("unarchive-moved-the-message-back-to-the-servers-inbox", unarch_back,
         "unarchived %r (key pressed %r, rows on the Archive view %r); moves %r; flag-only unarchive seen %r; server boxes %r" % (
             unarch_subject, unarch.get("pressed"), unarch.get("archive_view_rows"), moves, unarch_flagged_only, {k: v for k, v in boxes.items() if k in ("INBOX", "Archive")})),
        # engine-act-r6: the Archive view answers from the local store at once (E keeps the row as INBOX + archived; the sync does not re-read Archive)
        ("archive-view-lists-the-archived-message", bool(unarch_subject) and unarch_subject in (unarch.get("archive_view_rows") or []),
         "archived %r; rows on the Archive view (G E) %r" % (unarch_subject, unarch.get("archive_view_rows"))),
        ("junk-view-lists-the-junked-message", bool(spam_subject) and spam_subject in (notspam.get("junk_view_rows") or []),
         "junked %r; rows on the Junk view (G ⇧!) %r" % (spam_subject, notspam.get("junk_view_rows"))),
        ("mark-not-spam-moves-the-message-back-to-the-servers-inbox", not_spam_back,
         "not spam on %r (palette command clicked %r); moves %r; server boxes %r" % (
             spam_subject, notspam.get("pressed"), moves, {k: v for k, v in boxes.items() if k in ("INBOX", "Junk")})),
        ("the-driver-reached-every-step", not answer.get("driver_errors"), "driver errors %r" % answer.get("driver_errors")),
    ]


def drive5():
    """Inside launch-one: add the local account, act on the three rows by key as J1 does, reply twice through the quick reply."""
    sys.path.insert(0, os.path.join(ROOT, "journeys"))
    from lib.ctl import Ctl  # noqa: E402
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import realcore_mailfake as fake  # noqa: E402
    c = Ctl().connect()
    c.click(A + ".open")
    c.click(A + ".address"); c.type(CHECK_ADDRESS)
    c.click(A + ".password"); c.type(fake.PASSWORD)
    c.click(A + ".continue")
    answer = {}
    t0 = time.time()
    while time.time() - t0 < DETECT_WAIT_S:
        st = c.rows(A + ".state")
        if st and st[0].get("stage") not in ("detecting", "entry"):
            break
        time.sleep(0.3)
    c.click(A + ".add")
    stage = None
    t0 = time.time()
    while time.time() - t0 < DETECT_WAIT_S:
        st = c.rows(A + ".state"); stage = st[0].get("stage") if st else None
        if stage in ("added", "summary"):
            break
        time.sleep(0.3)
    answer["stage"] = stage
    try:
        err = c.rows(A + ".error"); answer["error"] = err[0].get("text") if err else None
    except Exception:
        answer["error"] = None
    try:
        c.click(A + ".done")
    except Exception as e:
        answer["done"] = str(e)
    # main55: a first launch shows the first-start flow over the inbox and it swallows E H S R and the other mail keys until it ends
    # (Sources/Features/FirstStart/FirstStartFeature.swift `swallowed`). A person reads through it with Continue (Enter); so does the driver.
    fs = answer["first_start"] = {"presses": 0, "active": None}
    for _ in range(10):
        try:
            st = c.rows("mac-first-start.state")
        except Exception as e:      # noqa: BLE001
            fs["unreadable"] = str(e); break
        fs["active"] = bool(st and st[0].get("active"))
        if not fs["active"]:
            break
        fs["last_step"] = st[0].get("step")
        try:
            c.key("Enter"); fs["presses"] += 1
        except Exception as e:      # noqa: BLE001
            fs["refused"] = str(e); break
        time.sleep(0.5)
    time.sleep(1.5)      # the veil lifts
    rows = []
    t0 = time.time()
    while time.time() - t0 < ACT_WAIT_S and len(rows) < 3:
        try:
            rows = c.rows("mac-main.list")
        except Exception:
            rows = []
        time.sleep(0.5)
    answer["rows_before"] = [{"id": r.get("id"), "subject": r.get("subject")} for r in rows]
    def uid(r):   # the check messages are "Check message <n>", stored under UID n by the fake
        try:
            return int((r.get("subject") or "").rsplit(" ", 1)[1])
        except (IndexError, ValueError):
            return None
    acted = {}
    if len(rows) >= 3:
        top = lambda: (c.rows("mac-main.list") or [{}])[0]
        trace = answer.setdefault("trace", [])
        errors = answer.setdefault("driver_errors", [])

        def attempt(label, fn):
            """A step that the app refused is recorded by name and the drive goes on, so every judge below still runs."""
            try:
                return fn()
            except Exception as e:          # noqa: BLE001  lib.ctl.Unreadable and socket errors alike
                errors.append("%s: %s" % (label, e)); return None


        def look(label):
            """What the app shows after a step, kept in the log so a red names the step that did nothing."""
            try:
                ui = c.uistate(); lst = c.rows("mac-main.list")
                bar = c.rows("mac-main.undo.bar")
                trace.append({"after": label, "list": [r.get("subject") for r in lst], "focus": ui.get("focus_message"), "overlay": ui.get("overlay"), "first_responder": ui.get("first_responder"), "surface": ui.get("surface"), "last_failure": ui.get("last_failure"),
                              "undo_bar": [b.get("message") for b in bar], "keys": keys_seen()[-3:]})
            except Exception as e:      # noqa: BLE001
                trace.append({"after": label, "unreadable": str(e)})
        def keys_seen():
            try:
                return [(d.get("key"), d.get("command"), d.get("resolved"), d.get("outcome")) for d in c.rows("mac-shortcuts.dispatch")]
            except Exception as e:      # noqa: BLE001
                return [("unreadable", str(e))]
        look("rows ready")
        r = top(); acted["archive"] = {"subject": r.get("subject"), "uid": uid(r)}
        attempt("E", lambda: c.key("E")); time.sleep(0.8); look("E")
        r = top(); acted["snooze"] = {"subject": r.get("subject"), "uid": uid(r)}
        attempt("H", lambda: c.key("H")); time.sleep(0.4); look("H"); attempt("type snooze", lambda: c.type("tomorrow morning")); attempt("snooze Enter", lambda: c.key("Enter")); time.sleep(0.8); look("snooze Enter")
        r = top(); acted["star"] = {"subject": r.get("subject"), "uid": uid(r)}
        attempt("S", lambda: c.key("S")); time.sleep(0.8); look("S")
        def wait_rows(ax, seconds=6.0):
            t = time.time()
            while time.time() - t < seconds:
                r = attempt("rows " + ax, lambda: c.rows(ax)) or []
                if r:
                    return r
                time.sleep(0.25)
            return []

        def reply(words, label):
            """R opens the quick reply on the open thread (par-quickreply, J1.7): wait for its field, make sure it holds the typed words, then ⌘Enter."""
            if not wait_rows("mac-main.reader.subject"):
                errors.append("%s: the thread never opened (no mac-main.reader.subject row)" % label); return False
            attempt(label + " R", lambda: c.key("R"))
            field = wait_rows(FIELD)
            if not field:
                errors.append("%s: the quick reply never opened after R (no %s row)" % (label, FIELD)); return False
            if field[0].get("focused") is False:
                attempt(label + " field click", lambda: c.click(FIELD))
            if (field[0].get("text") or "").strip():
                errors.append("%s: the quick reply was not empty before typing: %r" % (label, field[0].get("text"))); return False
            attempt(label + " type", lambda: c.type(words))
            t = time.time()
            while time.time() - t < 3 and words not in ((attempt(label + " field text", lambda: c.rows(FIELD)) or [{}])[0].get("text") or ""):
                time.sleep(0.2)
            held = (attempt(label + " field text", lambda: c.rows(FIELD)) or [{}])[0]
            if words not in (held.get("text") or ""):
                errors.append("%s: the quick reply holds %r, not the typed words" % (label, held.get("text"))); return False
            answer.setdefault("reply_from", held.get("from"))
            return attempt(label + " ⌘Enter", lambda: (c.key("⌘Enter"), True)[1]) is True

        enter_error = None
        try:
            c.key("Enter")
        except Exception as e:      # noqa: BLE001
            enter_error = str(e)
        answer["enter_opens_thread"] = enter_error is None
        answer["enter_error"] = enter_error
        if enter_error:
            try:
                answer["enter_failure_words"] = (c.uistate().get("last_failure") or {}).get("reason_words")
            except Exception:      # noqa: BLE001
                pass
        time.sleep(0.8); look("Enter")
        # 1. the reply that is taken back inside the undo window (first, so the kept reply that follows has no sent copy of its own in the thread)
        if reply(REPLY_UNDONE, "reply to undo"):
            bar = wait_rows("mac-main.undo.bar", 3.0)
            answer["undo_bar_before_z"] = bool(bar and bar[0].get("message") == "Sending")   # the control: the send IS pending when Z comes
            attempt("Z", lambda: c.key("Z")); time.sleep(0.5)
            bar = attempt("rows undo.bar", lambda: c.rows("mac-main.undo.bar"))
            answer["undo_bar_after_z"] = bool(bar and bar[0].get("message") == "Sending") if bar is not None else None
            answer["field_after_z"] = (attempt("rows field", lambda: c.rows(FIELD)) or [None])[0]
        look("after Z")
        time.sleep(undo_window_s() + 3)   # past the window: the undone reply must still not have left
        if (answer.get("field_after_z") or {}).get("text"):
            # The words Z gave back are thrown away with the reply's own Discard (⇧⌘,). Since 13 Nov Escape keeps them as the conversation's draft and R
            # then continues that draft: the kept reply below found them in the field (main137, slice 5: "the quick reply was not empty before typing").
            attempt("⇧⌘, (discard the words Z gave back)", lambda: c.key("⇧⌘,")); time.sleep(0.5)
        # 2. the kept reply
        reply(REPLY_KEPT, "kept reply"); look("kept reply")
        time.sleep(undo_window_s() + 4)   # the window ends: the core's outbox timer sends it
        # 3. a second reply in the same thread, after the first one was sent: the owner's next R
        if reply(REPLY_AGAIN, "reply again"):
            bar = wait_rows("mac-main.undo.bar", 3.0)
            answer["again_bar"] = bool(bar and bar[0].get("message") == "Sending")
            answer["again_field"] = (attempt("rows field", lambda: c.rows(FIELD)) or [{}])[0]
            answer["again_retry"] = attempt("rows retry", lambda: c.rows("mac-main.reply.retry"))
        look("reply again")
        answer["thread"] = [{k: t.get(k) for k in ("sender", "from", "text")} for t in (attempt("rows thread", lambda: c.rows("mac-main.reader.thread")) or [])]
        c.key("Escape"); c.key("Escape"); time.sleep(0.5)
        # the list as the first steps left it (the unarchive below brings the archived row back, so this is read first)
        try:
            answer["rows_after"] = [{"id": r.get("id"), "subject": r.get("subject")} for r in c.rows("mac-main.list")]
        except Exception:
            answer["rows_after"] = []
        # engine-act-r4: 4. junk the last inbox row (⇧!): the server moves it from INBOX to Junk. First, while it is the only row, so the target is certain.
        r = top(); acted["spam"] = {"subject": r.get("subject"), "uid": uid(r)}
        attempt("⇧!", lambda: c.key("⇧!")); time.sleep(1.5); look("⇧!")
        # 5. bring the archived mail back: G E opens the Archive view, ⇧E marks the focused thread not done (TriageKeys.markNotDone)
        un = answer["unarchive"] = {"pressed": False, "archive_view_rows": []}
        attempt("G", lambda: c.key("G")); attempt("G E", lambda: c.key("E")); time.sleep(1.0)
        un["archive_view_rows"] = [r.get("subject") for r in (attempt("rows archive view", lambda: c.rows("mac-main.list")) or [])]
        look("G E")
        if acted["archive"]["subject"] in un["archive_view_rows"]:
            un["pressed"] = attempt("⇧E", lambda: (c.key("⇧E"), True)[1]) is True
            time.sleep(1.5); look("⇧E")
        else:
            errors.append("the Archive view (G E) does not list %r: %r" % (acted["archive"]["subject"], un["archive_view_rows"]))
        # engine-act-r6: 6. Not spam on the junked message: G ⇧! opens the Junk view, the palette's "Not spam" acts on the focused row
        # (docs/CONTROL-SOCKET.md, fixture-r3 deltas: `click mac-shortcuts.action.not-spam` sends MarkNotSpam). The junk move is applied by now
        # (the worker ran during the unarchive step), so the server's place of the Junk copy is recorded and the way back can name it.
        ns = answer["not_spam"] = {"pressed": False, "junk_view_rows": []}
        attempt("G", lambda: c.key("G")); attempt("G ⇧!", lambda: c.key("⇧!")); time.sleep(1.0)
        ns["junk_view_rows"] = [r.get("subject") for r in (attempt("rows junk view", lambda: c.rows("mac-main.list")) or [])]
        look("G ⇧!")
        if acted["spam"]["subject"] in ns["junk_view_rows"]:
            ns["pressed"] = attempt("not spam", lambda: (c.click("mac-shortcuts.action.not-spam"), True)[1]) is True
            time.sleep(1.5); look("not spam")
        else:
            errors.append("the Junk view (G ⇧!) does not list %r: %r" % (acted["spam"]["subject"], ns["junk_view_rows"]))
        attempt("G", lambda: c.key("G")); attempt("G I", lambda: c.key("I")); time.sleep(0.8)
        time.sleep(4)      # the write-back worker applies the moves
    answer["acted"] = acted
    if "rows_after" not in answer:
        try:
            answer["rows_after"] = [{"id": r.get("id"), "subject": r.get("subject")} for r in c.rows("mac-main.list")]
        except Exception:
            answer["rows_after"] = []
    answer["core"] = c.uistate().get("core")
    return answer


def server_record(mail, smtp):
    return {"moves": [[m[0], list(m[1]), m[2]] for m in mail.moves], "stores": [[st[0], list(st[1]), st[2], list(st[3])] for st in mail.stores],
            "appends": [[a[0], list(a[1]), a[2].decode(errors="replace")] for a in mail.appends], "smtp": [m.decode(errors="replace") for m in smtp.messages],
            "boxes": {name: [subject_of(raw) for _, raw in msgs] for name, msgs in mail.boxes.items()}}


def subject_of(raw):
    """The Subject of a stored raw message (the fake's boxes hold bytes)."""
    for line in raw.decode(errors="replace").split("\r\n\r\n")[0].split("\r\n"):
        if line.lower().startswith("subject:"):
            return line.split(":", 1)[1].strip()
    return ""


def main_slice5(bundle, mutant):
    """The local servers live in THIS process (threads) for the whole launch; the app reaches them on 127.0.0.1 only."""
    import json
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import realcore_mailfake as fake
    try:
        mail, imap, smtp = fake.start(0, 0, inbox=3, act=True)
    except OSError as e:
        print("the local servers did not start: %s" % e)
        print("1 checks, 0 failed, 1 not judged"); return 1
    ports = "%d:%d" % (imap.server_address[1], smtp.server_address[1])
    extra = ["WHOOSH_CORE_PLAIN_LOOPBACK=1", "WHOOSH_CORE_CHECK_SERVERS=" + ports] + (["WHOOSH_CORE_MUTANT=act-never-writes-back"] if mutant else [])
    env = dict(os.environ, WHOOSH_RIG_HIDDEN="1", LAUNCH_ENV=" ".join(["WHOOSH_RIG_HIDDEN=1"] + extra))
    env.pop("WHOOSH_FIXTURE_CORE", None)
    cmd = ["sh", os.path.join(ROOT, "scripts", "launch-one.sh"), bundle, "--", "--rig-hidden",
           "sh", "-c", 'WHOOSH_SOCKET_DIR="$D" python3 "$0" --inner --slice5', os.path.abspath(__file__)]
    r = launch(cmd, env)
    record = server_record(mail, smtp)
    imap.shutdown(); smtp.shutdown()
    out = r.stdout + r.stderr
    line = next((l for l in out.splitlines() if l.startswith("ANSWER ")), None)
    if line is None:
        print(out.rstrip()[-3000:])
        print("launcher exit %d" % r.returncode)
        print("1 checks, 0 failed, 1 not judged"); return 1
    answer = json.loads(line[len("ANSWER "):])
    print("launcher exit %d; the local server saw: %s; %s" % (r.returncode, sorted({l.split(" ")[0] for l in mail.log}), next((l for l in out.splitlines() if l.startswith("instances:")), "")))
    results = judge5(answer, record)
    for k in ("field_after_z", "again_bar", "again_field", "again_retry", "unarchive", "not_spam"):
        print("PROBE      %s %s" % (k, json.dumps(answer.get(k), ensure_ascii=False)))
    for t in answer.get("trace") or []:
        print("TRACE      " + json.dumps(t, ensure_ascii=False))
    lines = ["%-10s %s  %s" % ("PASS" if ok else ("NOT JUDGED" if ok is None else "FAIL"), n, d) for n, ok, d in results]
    failed = sum(1 for _, ok, _ in results if ok is False)
    unjudged = sum(1 for _, ok, _ in results if ok is None)
    if mutant:
        red = all(ok is False for n, ok, _ in results if n in ACT_CHECKS)
        lines.append("mutant act-never-writes-back: %s" % ("RED as it must be (the server heard no archive, no star, no junk move and no Not spam move)" if red else "SURVIVED: the server heard a write the core never sent"))
        lines.append("%d checks, %d failed, %d not judged (mutant run)" % (len(results), failed, unjudged))
        print("\n".join(lines)); return 0 if red else 1
    lines.append("%d checks, %d failed, %d not judged" % (len(results), failed, unjudged))
    print("\n".join(lines)); return 0 if failed == 0 else 1


def selftest5():
    """The judge against a model answer and the never-writes-back model, and the act-mode fake against Python's own IMAP and SMTP clients."""
    import imaplib
    import smtplib
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import realcore_mailfake as fake
    rows = [{"id": "m3", "subject": "Check message 3"}, {"id": "m2", "subject": "Check message 2"}, {"id": "m1", "subject": "Check message 1"}]
    acted = {"archive": {"subject": "Check message 3", "uid": 3}, "snooze": {"subject": "Check message 2", "uid": 2}, "star": {"subject": "Check message 1", "uid": 1}, "spam": {"subject": "Check message 1", "uid": 1}}
    good_answer = {"stage": "added", "core": "rust", "rows_before": rows, "acted": acted, "first_start": {"presses": 7, "active": False}, "rows_after": [rows[2]],
                   "thread": [{"sender": "Sender 1", "from": "sender1@example.test", "text": "hello"}, {"sender": CHECK_ADDRESS, "from": CHECK_ADDRESS, "text": REPLY_KEPT + "\n--\nOwner"}],
                   "undo_bar_before_z": True, "undo_bar_after_z": False, "driver_errors": [], "enter_opens_thread": True, "again_bar": True,
                   "unarchive": {"pressed": True, "archive_view_rows": ["Check message 3"]}, "not_spam": {"pressed": True, "junk_view_rows": ["Check message 1"]}}
    reply = "Subject: Re: Check message 1\r\nIn-Reply-To: <check-inbox-1@whoosh-check.invalid>\r\n\r\n%s\r\n" % REPLY_KEPT
    good_server = {"moves": [["INBOX", [3], "Archive"], ["INBOX", [1], "Junk"], ["Archive", [1], "INBOX"], ["Junk", [1], "INBOX"]], "stores": [["INBOX", [1], "+FLAGS", ["\\Flagged"]]],
                   "appends": [["Sent", ["\\Seen"], reply]], "smtp": [reply],
                   "boxes": {"INBOX": ["Check message 2", "Check message 3", "Check message 1"], "Archive": [], "Junk": [], "Sent": ["Check message 1"]}}
    never = dict(good_server, moves=[], stores=[])
    undone_left = dict(good_server, smtp=[reply, "Subject: Re\r\n\r\n%s\r\n" % REPLY_UNDONE])
    snooze_moved = dict(good_server, moves=good_server["moves"] + [["INBOX", [2], "Archive"]])
    g = judge5(good_answer, good_server)
    unjudged_names = [n for n, x, _ in g if x is None]
    ok = all(x for _, x, _ in g if x is not None) and unjudged_names == []
    ok = ok and all(not x for n, x, _ in judge5(good_answer, never) if n in ACT_CHECKS)
    ok = ok and not dict((n, x) for n, x, _ in judge5(good_answer, undone_left))["the-undone-send-never-left"]
    ok = ok and not dict((n, x) for n, x, _ in judge5(good_answer, snooze_moved))["snooze-hides-the-row-and-leaves-the-server-alone"]
    ok = ok and not dict((n, x) for n, x, _ in judge5(dict(good_answer, thread=good_answer["thread"][:1]), good_server))["the-sent-copy-joins-its-thread"]
    ok = ok and not dict((n, x) for n, x, _ in judge5(dict(good_answer, first_start={"presses": 10, "active": True}), good_server))["first-start-flow-ended-before-the-mail-keys"]
    ok = ok and not dict((n, x) for n, x, _ in judge5(dict(good_answer, again_bar=False, again_retry=[{"words": "the message you reply to has no place on the mail server yet"}]), good_server))["a-second-reply-in-the-same-thread-is-accepted"]
    ok = ok and not dict((n, x) for n, x, _ in judge5(dict(good_answer, undo_bar_before_z=None), good_server))["undo-send-was-pending-when-z-came"]   # no control: no zero
    ok = ok and not dict((n, x) for n, x, _ in judge5(dict(good_answer, undo_bar_after_z=True), good_server))["the-undone-send-never-left"]   # Z did not undo
    ok = ok and not dict((n, x) for n, x, _ in judge5(dict(good_answer, driver_errors=["key ('⌘Enter',): the key ⌘Enter does nothing on mac-main"]), good_server))["the-driver-reached-every-step"]
    ok = ok and not dict((n, x) for n, x, _ in judge5(dict(good_answer, rows_after=rows), good_server))["archived-row-left-the-list"]   # the row stayed
    # engine-act-r4: the junk move and the unarchive are each seen red when the server did not hear them, and the flag-only unarchive (what the core did on main55's sources) is named
    no_junk = dict(good_server, moves=[m for m in good_server["moves"] if m[2] != "Junk"], boxes=dict(good_server["boxes"], INBOX=["Check message 1", "Check message 2", "Check message 3"], Junk=[]))
    ok = ok and not dict((n, x) for n, x, _ in judge5(good_answer, no_junk))["marked-junk-reached-the-servers-junk-folder"]
    no_back = dict(good_server, moves=[m for m in good_server["moves"] if m[2] != "INBOX"], boxes=dict(good_server["boxes"], INBOX=["Check message 2"], Archive=["Check message 3"]))
    ok = ok and not dict((n, x) for n, x, _ in judge5(good_answer, no_back))["unarchive-moved-the-message-back-to-the-servers-inbox"]
    flag_only = dict(no_back, stores=good_server["stores"] + [["INBOX", [3], "-FLAGS.SILENT", ["\\Archived"]]])
    fo = dict((n, (x, d)) for n, x, d in judge5(good_answer, flag_only))["unarchive-moved-the-message-back-to-the-servers-inbox"]
    ok = ok and fo[0] is False and "flag-only unarchive seen True" in fo[1]
    ok = ok and not dict((n, x) for n, x, _ in judge5(dict(good_answer, unarchive={"pressed": False, "archive_view_rows": []}), no_back))["unarchive-moved-the-message-back-to-the-servers-inbox"]
    # engine-act-r6: the Archive view that lists nothing (main57's red) and the Junk view that lists nothing are each red; Not spam that never
    # reached the server is red while the junk move before it still passes; a junk move the server never made is not rescued by a Not spam
    ok = ok and not dict((n, x) for n, x, _ in judge5(dict(good_answer, unarchive={"pressed": False, "archive_view_rows": []}), good_server))["archive-view-lists-the-archived-message"]
    ok = ok and not dict((n, x) for n, x, _ in judge5(dict(good_answer, not_spam={"pressed": False, "junk_view_rows": []}), good_server))["junk-view-lists-the-junked-message"]
    stayed = dict(good_server, moves=[m for m in good_server["moves"] if m[0] != "Junk"], boxes=dict(good_server["boxes"], INBOX=["Check message 2", "Check message 3"], Junk=["Check message 1"]))
    sj = dict((n, x) for n, x, _ in judge5(good_answer, stayed))
    ok = ok and sj["mark-not-spam-moves-the-message-back-to-the-servers-inbox"] is False and sj["marked-junk-reached-the-servers-junk-folder"] is True
    ok = ok and not dict((n, x) for n, x, _ in judge5(good_answer, dict(good_server, moves=[m for m in good_server["moves"] if m[2] != "Junk"])))["marked-junk-reached-the-servers-junk-folder"]
    # a MOVE to Junk the server answered but whose message is in no Junk box and was never moved out of it did not land
    lost = dict(good_server, moves=[m for m in good_server["moves"] if m[0] != "Junk"], boxes=dict(good_server["boxes"], INBOX=["Check message 2", "Check message 3"], Junk=[]))
    ok = ok and not dict((n, x) for n, x, _ in judge5(good_answer, lost))["marked-junk-reached-the-servers-junk-folder"]
    mail, imap, smtp = fake.start(0, 0, inbox=3, act=True)
    try:
        c = imaplib.IMAP4("127.0.0.1", imap.server_address[1])
        c.login(fake.ADDRESS, fake.PASSWORD)
        typ, boxes = c.list()
        listed = all(x in b"".join(boxes) for x in (b"\\Archive", b"\\Junk", b"\\Sent"))
        c.select("INBOX")
        c.uid("STORE", "1", "+FLAGS.SILENT", "(\\Flagged)")
        typ, data = c.uid("FETCH", "1", "(UID FLAGS)")
        flagged = b"\\Flagged" in b"".join(x if isinstance(x, bytes) else x[0] for x in data)
        typ, _ = c.uid("MOVE", "3", "Archive")
        moved = typ == "OK" and [u for u, _ in mail.boxes["INBOX"]] == [1, 2] and len(mail.boxes["Archive"]) == 1
        # engine-act-r4: the way back (Archive -> INBOX) is a recorded MOVE, and the boxes the judge reads show the message in INBOX again
        c.select("Archive"); typ_back, _ = c.uid("MOVE", "1", "INBOX"); c.select("INBOX")
        boxes_now = server_record(mail, smtp)["boxes"]
        moved = moved and typ_back == "OK" and "Check message 3" in boxes_now["INBOX"] and boxes_now["Archive"] == []
        # engine-act-r6: junk and Not spam (INBOX -> Junk -> INBOX) are two recorded MOVEs, and the boxes show the message in INBOX again
        typ_j, _ = c.uid("MOVE", "1", "Junk"); c.select("Junk"); typ_nj, _ = c.uid("MOVE", "1", "INBOX"); c.select("INBOX")
        boxes_now = server_record(mail, smtp)["boxes"]
        moved = moved and typ_j == "OK" and typ_nj == "OK" and "Check message 1" in boxes_now["INBOX"] and boxes_now["Junk"] == []
        typ, _ = c.append("Sent", "(\\Seen)", None, b"Subject: copy\r\n\r\nthe copy\r\n")
        appended = typ == "OK" and mail.appends and mail.appends[-1][0] == "Sent" and b"the copy" in mail.appends[-1][2]
        c.logout()
        s = smtplib.SMTP("127.0.0.1", smtp.server_address[1]); s.ehlo(); s.login(fake.ADDRESS, fake.PASSWORD)
        s.sendmail(fake.ADDRESS, ["friend@example.test"], "Subject: hi\r\n\r\n%s\r\n" % REPLY_KEPT); s.quit()
        accepted = len(smtp.messages) == 1 and REPLY_KEPT.encode() in smtp.messages[0]
        recorded = mail.stores == [("INBOX", [1], "+FLAGS", ["\\Flagged"])] and mail.moves == [("INBOX", [3], "Archive", "MOVE"), ("Archive", [1], "INBOX", "MOVE"),
                                                                                  ("INBOX", [1], "Junk", "MOVE"), ("Junk", [1], "INBOX", "MOVE")]
    finally:
        imap.shutdown(); smtp.shutdown()
    served = listed and flagged and moved and appended and accepted and recorded
    print("selftest slice 5: model %s, mutant models %s, act-mode fake %s" % ("all pass (none left NOT JUDGED)" if all(x for _, x, _ in g if x is not None) and not unjudged_names else "FAILS", "red" if ok else "SURVIVE", "records STORE, MOVE, APPEND and DATA" if served else "WRONG (listed %s flagged %s moved %s appended %s accepted %s recorded %s)" % (listed, flagged, moved, appended, accepted, recorded)))
    return ok and served


# ---------------------------------------------------------------- slice 6 (junk-r2): the local spam filter on live arrivals

JUNK_WAIT_S = 60
JUNK_CHECKS = ("the-phish-reached-the-junk-folder", "the-phish-left-the-list")


def judge6(answer, server):
    """`answer` is what the app showed (inside launch-one); `server` is what the local IMAP side received and delivered (this process)."""
    import realcore_mailfake as fake
    moves = server.get("moves") or []
    phish_uid, plain_uid = server.get("phish_uid"), server.get("plain_uid")
    before = [r.get("subject") for r in answer.get("rows_before") or []]
    after = [r.get("subject") for r in answer.get("rows_after") or []]
    to_junk = [m for m in moves if m[0] == "INBOX" and m[2] == "Junk"]
    return [
        ("account-added", answer.get("stage") == "added", "stage %r, error %r" % (answer.get("stage"), answer.get("error"))),
        ("core-is-rust", answer.get("core") in (None, "rust"), "uistate core %r" % answer.get("core")),
        ("the-history-is-listed-and-never-moved", len(before) >= fake.CORPUS_PER_CLASS and not any(m[1] and phish_uid not in m[1] for m in moves),
         "%d rows before the arrivals; moves %r" % (len(before), moves)),
        ("the-arrivals-were-delivered-while-listening", bool(phish_uid) and bool(plain_uid) and bool(server.get("delivered_after_idle")),
         "phish uid %r, plain uid %r, after IDLE %r" % (phish_uid, plain_uid, server.get("delivered_after_idle"))),
        ("the-phish-reached-the-junk-folder", bool(phish_uid) and any(phish_uid in m[1] for m in to_junk), "moves to Junk %r" % to_junk),
        ("the-plain-message-stayed-on-the-server", bool(plain_uid) and not any(plain_uid in m[1] for m in moves), "moves %r" % moves),
        ("the-phish-left-the-list", bool(after) and fake.PHISH_SUBJECT not in after, "after %r" % after[:6]),
        ("the-plain-message-is-listed", fake.PLAIN_SUBJECT in after, "after %r" % after[:6]),
    ]


def drive6():
    """Inside launch-one: add the local account, wait for its history in the list, then for the plain arrival (the outer process delivers
    the two arrivals once the core holds IDLE) and read the list again."""
    sys.path.insert(0, os.path.join(ROOT, "journeys"))
    from lib.ctl import Ctl  # noqa: E402
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import realcore_mailfake as fake  # noqa: E402
    c = Ctl().connect()
    c.click(A + ".open")
    c.click(A + ".address"); c.type(CHECK_ADDRESS)
    c.click(A + ".password"); c.type(fake.PASSWORD)
    c.click(A + ".continue")
    answer = {}
    t0 = time.time()
    while time.time() - t0 < DETECT_WAIT_S:
        st = c.rows(A + ".state")
        if st and st[0].get("stage") not in ("detecting", "entry"):
            break
        time.sleep(0.3)
    c.click(A + ".add")
    stage = None
    t0 = time.time()
    while time.time() - t0 < DETECT_WAIT_S:
        st = c.rows(A + ".state"); stage = st[0].get("stage") if st else None
        if stage in ("added", "summary"):
            break
        time.sleep(0.3)
    answer["stage"] = stage
    try:
        err = c.rows(A + ".error"); answer["error"] = err[0].get("text") if err else None
    except Exception:
        answer["error"] = None
    try:
        c.click(A + ".done")
    except Exception as e:
        answer["done"] = str(e)
    def rows():
        try:
            return c.rows("mac-main.list") or []
        except Exception:
            return []
    listed = []
    t0 = time.time()
    while time.time() - t0 < JUNK_WAIT_S and len(listed) < fake.CORPUS_PER_CLASS:
        listed = rows(); time.sleep(0.5)
    answer["rows_before"] = [{"subject": r.get("subject")} for r in listed]
    after = []
    t0 = time.time()
    while time.time() - t0 < JUNK_WAIT_S:
        after = rows()
        if any(r.get("subject") == fake.PLAIN_SUBJECT for r in after):
            break
        time.sleep(0.5)
    time.sleep(2)   # the phish arrived in the same IDLE notice; its move is one more round trip
    answer["rows_after"] = [{"subject": r.get("subject")} for r in rows()]
    answer["core"] = c.uistate().get("core")
    return answer


def deliver_when_idle(mail, record, wait_s):
    """The outer side: once the core holds IDLE (its history stored and its first-run training done, sync.rs), the phish then the plain
    message arrive on INBOX. Runs on its own thread for the whole launch."""
    import realcore_mailfake as fake
    t0 = time.time()
    while time.time() - t0 < wait_s:
        with mail.lock:
            idling = any(l.startswith("IDLE") for l in mail.log)
        if idling:
            time.sleep(1.0)   # the core's IDLE is up; the fake announces new mail at its next poll
            record["phish_uid"] = fake.deliver(mail, fake.phish_message())
            record["plain_uid"] = fake.deliver(mail, fake.plain_message())
            record["delivered_after_idle"] = True
            return
        time.sleep(0.3)
    record["delivered_after_idle"] = False


def main_slice6(bundle, mutant):
    """The local server lives in THIS process (threads) for the whole launch; the app reaches it on 127.0.0.1 only."""
    import json
    import threading
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import realcore_mailfake as fake
    try:
        mail, imap, smtp = fake.start(0, 0, inbox=0, act=True, corpus=True)
    except OSError as e:
        print("the local servers did not start: %s" % e)
        print("1 checks, 0 failed, 1 not judged"); return 1
    record = {}
    threading.Thread(target=deliver_when_idle, args=(mail, record, DETECT_WAIT_S + 2 * JUNK_WAIT_S), daemon=True).start()
    ports = "%d:%d" % (imap.server_address[1], smtp.server_address[1])
    extra = ["WHOOSH_CORE_PLAIN_LOOPBACK=1", "WHOOSH_CORE_CHECK_SERVERS=" + ports] + (["WHOOSH_CORE_MUTANT=junk-never-moves"] if mutant else [])
    env = dict(os.environ, WHOOSH_RIG_HIDDEN="1", LAUNCH_ENV=" ".join(["WHOOSH_RIG_HIDDEN=1"] + extra))
    env.pop("WHOOSH_FIXTURE_CORE", None)
    cmd = ["sh", os.path.join(ROOT, "scripts", "launch-one.sh"), bundle, "--", "--rig-hidden",
           "sh", "-c", 'WHOOSH_SOCKET_DIR="$D" python3 "$0" --inner --slice6', os.path.abspath(__file__)]
    r = launch(cmd, env)
    server = dict(record, moves=[[m[0], list(m[1]), m[2]] for m in mail.moves])
    imap.shutdown(); smtp.shutdown()
    out = r.stdout + r.stderr
    line = next((l for l in out.splitlines() if l.startswith("ANSWER ")), None)
    if line is None:
        print(out.rstrip()[-3000:])
        print("launcher exit %d" % r.returncode)
        print("1 checks, 0 failed, 1 not judged"); return 1
    answer = json.loads(line[len("ANSWER "):])
    print("launcher exit %d; the local server saw: %s; %s" % (r.returncode, sorted({l.split(" ")[0] for l in mail.log}), next((l for l in out.splitlines() if l.startswith("instances:")), "")))
    results = judge6(answer, server)
    lines = ["%-10s %s  %s" % ("PASS" if ok else ("NOT JUDGED" if ok is None else "FAIL"), n, d) for n, ok, d in results]
    failed = sum(1 for _, ok, _ in results if ok is False)
    if mutant:
        red = all(ok is False for n, ok, _ in results if n in JUNK_CHECKS)
        lines.append("mutant junk-never-moves: %s" % ("RED as it must be (the phish stayed in INBOX and in the list)" if red else "SURVIVED: the phish moved with a filter that never moves"))
        lines.append("%d checks, %d failed, 0 not judged (mutant run)" % (len(results), failed))
        print("\n".join(lines)); return 0 if red else 1
    lines.append("%d checks, %d failed, 0 not judged" % (len(results), failed))
    print("\n".join(lines)); return 0 if failed == 0 else 1


def selftest6():
    """The judge against a model answer and the never-moves model, and the corpus fake against Python's own IMAP client: the two
    folders hold the corpus, a delivered phish carries its Authentication-Results in the header the core fetches, a MOVE is recorded."""
    import imaplib
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import realcore_mailfake as fake
    history = [{"subject": "Lunch on friday %d" % i} for i in range(fake.CORPUS_PER_CLASS)]
    phish_uid, plain_uid = fake.CORPUS_PER_CLASS + 1, fake.CORPUS_PER_CLASS + 2
    good_answer = {"stage": "added", "core": "rust", "rows_before": history, "rows_after": [{"subject": fake.PLAIN_SUBJECT}] + history}
    good_server = {"moves": [["INBOX", [phish_uid], "Junk"]], "phish_uid": phish_uid, "plain_uid": plain_uid, "delivered_after_idle": True}
    never_answer = dict(good_answer, rows_after=[{"subject": fake.PLAIN_SUBJECT}, {"subject": fake.PHISH_SUBJECT}] + history)
    never_server = dict(good_server, moves=[])
    g = judge6(good_answer, good_server)
    ok = all(x for _, x, _ in g)
    ok = ok and all(not x for n, x, _ in judge6(never_answer, never_server) if n in JUNK_CHECKS)
    both = dict(good_server, moves=[["INBOX", [phish_uid, plain_uid], "Junk"]])
    ok = ok and not dict((n, x) for n, x, _ in judge6(good_answer, both))["the-plain-message-stayed-on-the-server"]
    old = dict(good_server, moves=[["INBOX", [3], "Junk"], ["INBOX", [phish_uid], "Junk"]])
    ok = ok and not dict((n, x) for n, x, _ in judge6(good_answer, old))["the-history-is-listed-and-never-moved"]
    late = dict(good_server, delivered_after_idle=False)
    ok = ok and not dict((n, x) for n, x, _ in judge6(good_answer, late))["the-arrivals-were-delivered-while-listening"]
    mail, imap, smtp = fake.start(0, 0, inbox=0, act=True, corpus=True)
    try:
        c = imaplib.IMAP4("127.0.0.1", imap.server_address[1])
        c.login(fake.ADDRESS, fake.PASSWORD)
        typ, boxes = c.list()
        listed = b"\\Junk" in b"".join(boxes)
        typ, data = c.select("Junk"); junk_n = int(data[0])
        typ, data = c.select("INBOX"); inbox_n = int(data[0])
        uid = fake.deliver(mail, fake.phish_message())
        typ, data = c.uid("FETCH", str(uid), "(UID BODY.PEEK[HEADER])")
        head = b"".join(x[1] for x in data if isinstance(x, tuple))
        headed = head.startswith(b"Authentication-Results: mx.whoosh-check.invalid; spf=fail") and b"dmarc=fail" in head and b'"service@bank.example"' in head
        typ, _ = c.uid("MOVE", str(uid), "Junk")
        moved = typ == "OK" and mail.moves == [("INBOX", [uid], "Junk", "MOVE")] and uid not in [u for u, _ in mail.boxes["INBOX"]]
        c.logout()
    finally:
        imap.shutdown(); smtp.shutdown()
    served = listed and junk_n == fake.CORPUS_PER_CLASS and inbox_n == fake.CORPUS_PER_CLASS and headed and moved
    print("selftest slice 6: model %s, mutant models %s, corpus fake %s" % ("all pass" if all(x for _, x, _ in g) else "FAILS", "red" if ok else "SURVIVE",
          "serves the corpus, the phish's headers and records the MOVE" if served else "WRONG (listed %s junk %s inbox %s headers %s moved %s)" % (listed, junk_n, inbox_n, headed, moved)))
    return ok and served



# ---------------------------------------------------------------- the detector on recorded answers (slice 1 and --slice-detect)
SCENARIOS = [   # (label, domain constant of checks/realcore_netfake.py)
    ("m365-by-mail-record", "M365_MX"), ("google-by-mail-record", "GOOGLE"), ("provider-database", "ISPDB"), ("srv-records", "SRV"),
    ("own-autodiscover", "POX"), ("plain-host-by-guess", "PLAIN"), ("no-such-domain", "MISSING")]


def drive_detect():
    """Inside launch-one: for each scenario, type its address, Continue, wait for the end of detection, read what the sheet says, cancel."""
    sys.path.insert(0, os.path.join(ROOT, "journeys"))
    from lib.ctl import Ctl  # noqa: E402
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import realcore_netfake as nf
    c = Ctl().connect()
    out = {}
    for label, const in SCENARIOS:
        a = {}
        try:
            c.click(A + ".open")
            c.click(A + ".address"); c.type("owner@" + getattr(nf, const))
            c.click(A + ".continue")
            t0 = time.time(); stage = None
            while time.time() - t0 < DETECT_WAIT_S:
                st = c.rows(A + ".state"); stage = st[0].get("stage") if st else None
                if stage not in ("detecting", "entry", None):
                    break
                time.sleep(0.3)
            a["stage"] = stage
            for region in ("found", "detection.step", "oauth", "error"):
                try:
                    a[region] = c.rows("%s.%s" % (A, region))
                except Exception as e:      # noqa: BLE001
                    a[region] = []; a.setdefault("unreadable", []).append("%s: %s" % (region, e))
            c.click(A + ".cancel")
        except Exception as e:      # noqa: BLE001
            a["error_drive"] = str(e)
        out[label] = a
    return out


def judge_detect(answers):
    """One group of cases per scenario: the servers found, the step that answered. [(name, ok, detail)]."""
    want = {"m365-by-mail-record": ("outlook.office365.com:993", "smtp.office365.com:587", "mx"),
            "google-by-mail-record": ("imap.gmail.com:993", "smtp.gmail.com:587", "mx"),
            "provider-database": ("imap.gmail.com:993", "smtp.gmail.com:465", "ispdb"),
            "srv-records": ("imap.srv-check.test:993", "smtp.srv-check.test:587", "srv"),
            "own-autodiscover": ("mail.onprem.example:993", "mail.onprem.example:587", "autodiscover"),
            "plain-host-by-guess": (None, None, "guess")}
    res = []
    for label, _c in SCENARIOS:
        a = answers.get(label) or {}
        found = (a.get("found") or [{}])[0]
        steps = a.get("detection.step") or []
        last = steps[-1] if steps else {}
        if label == "no-such-domain":
            err = (a.get("error") or [{}])[0].get("text")
            res.append((label, not a.get("found") and a.get("stage") != "added" and bool(steps) and last.get("outcome") != "found", "found %r, steps %r, the sheet says %r" % (found, [s.get("id") for s in steps], err)))
            continue
        imap, smtp, step = want[label]
        if label == "plain-host-by-guess":
            ok = str(found.get("imap", "")).startswith("imap.plain-check.test:") and last.get("id") == "guess" and last.get("outcome") == "found"
        else:
            ok = found.get("imap") == imap and found.get("smtp") == smtp and last.get("id") == step and last.get("outcome") == "found"
        if label == "m365-by-mail-record":
            o = (a.get("oauth") or [{}])[0]
            ok = ok and o.get("kind") == "microsoft"
        res.append((label, ok, "found %r, answered at %r%s" % (found, last.get("id"), (", " + "; ".join(a.get("unreadable", []))) if a.get("unreadable") else "")))
    return res


def local_network_case(dns_log, web_log):
    """The positive control of the detector runs: the answers came from the local network (it was asked over DNS AND HTTP), not from nowhere."""
    return ("detector-asked-the-local-network", bool(dns_log) and bool(web_log), "%d DNS questions %r, %d HTTP requests %r" % (len(dns_log), dns_log[:3], len(web_log), [(h, p_) for _m, h, p_ in web_log[:3]]))


def outside_case(count_line):
    """The case every slice ends with. `count_line`: the inner run's `REFUSED ...` line, or None."""
    if count_line is None:
        return ("no-outbound-refused", None, "the inner run printed no refusal count (the build does not serve rig/refused-outbound, or the run ended first)")
    rest = count_line[len("REFUSED "):]
    if rest.startswith("unreadable"):
        return ("no-outbound-refused", None, rest)
    try:
        import json
        v = json.loads(rest)
    except ValueError:
        return ("no-outbound-refused", None, "unreadable count %r" % rest)
    n = v.get("count") if isinstance(v, dict) else v
    return ("no-outbound-refused", n == 0, "the guard refused %r outbound connection(s)%s" % (n, ", the last to %r" % v.get("last_host") if isinstance(v, dict) and v.get("last_host") else ""))


def with_outside_case(run):
    """Runs one slice's main, adds the outbound case to its count line. The slice's own verdict is kept; a refusal makes the whole run exit 1."""
    import contextlib
    import io
    buf = io.StringIO()
    with contextlib.redirect_stdout(buf):
        rc = run()
    text = buf.getvalue()
    line = next((l for l in LAST["out"].splitlines() if l.startswith("REFUSED ")), None)
    name, ok, detail = outside_case(line)
    lines = text.rstrip("\n").split("\n")
    m = re.match(r"(\d+) checks, (\d+) failed, (\d+) not judged(.*)", lines[-1]) if lines else None
    if m is None:
        print(text, end=""); return rc          # a slice that printed no count line (usage, no launch) has nothing to add to
    n, f, u, tail = int(m.group(1)), int(m.group(2)), int(m.group(3)), m.group(4)
    lines[-1:] = ["%-10s %s  %s" % ("PASS" if ok else ("NOT JUDGED" if ok is None else "FAIL"), name, detail),
                  "%d checks, %d failed, %d not judged%s" % (n + 1, f + (ok is False), u + (ok is None), tail)]
    print("\n".join(lines))
    return 1 if ok is False else rc


def detector_launch(bundle, mutant, inner_flag, judge_fn, extra_note):
    """The launch of slice 1 and --slice-detect: the local network fake in THIS process, the seam in LAUNCH_ENV, nothing public."""
    import json
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import realcore_netfake as nf
    if not nf.bundle_has(bundle, "WHOOSH_CORE_CHECK_DETECT"):
        print("NOT JUDGED :: the build carries no detector seam (WHOOSH_CORE_CHECK_DETECT=<dns port>:<http port>, lane engine-act-r6): the detector would reach public DNS and HTTPS, "
              "which the guard refuses and the owner's rule forbids; nothing was launched")
        print("0 checks, 0 failed, 1 not judged"); return 1
    dns, web, stop = nf.start()
    extra = ["WHOOSH_CORE_CHECK_DETECT=%d:%d" % (dns.port, web.server_address[1])] + (["WHOOSH_CORE_MUTANT=no-microsoft"] if mutant else [])
    env = dict(os.environ, WHOOSH_RIG_HIDDEN="1", LAUNCH_ENV=" ".join(["WHOOSH_RIG_HIDDEN=1"] + extra))
    env.pop("WHOOSH_FIXTURE_CORE", None)
    cmd = ["sh", os.path.join(ROOT, "scripts", "launch-one.sh"), bundle, "--", "--rig-hidden",
           "sh", "-c", 'WHOOSH_SOCKET_DIR="$D" python3 "$0" --inner' + inner_flag, os.path.abspath(__file__)]
    r = launch(cmd, env)
    stop()
    out = r.stdout + r.stderr
    line = next((l for l in out.splitlines() if l.startswith("ANSWER ")), None)
    if line is None:
        print(out.rstrip()[-3000:])
        print("launcher exit %d" % r.returncode)
        print("1 checks, 0 failed, 1 not judged"); return 1
    answer = json.loads(line[len("ANSWER "):])
    print("launcher exit %d; %s; the local network was asked %d DNS and %d HTTP questions; %s" % (r.returncode, extra_note(answer), len(dns.log), len(web.log),
          next((l for l in out.splitlines() if l.startswith("instances:")), "")))
    return answer, dns, web


def refused_line():
    """Inside launch-one, after the slice's drive: what the rig core's outbound guard counted over the whole run."""
    sys.path.insert(0, os.path.join(ROOT, "journeys"))
    from lib.ctl import Ctl  # noqa: E402
    import json
    try:
        return "REFUSED " + json.dumps(Ctl().connect().core("rig/refused-outbound"))
    except Exception as e:      # noqa: BLE001
        return "REFUSED unreadable: %s" % e


def main_slice1(bundle, mutant):
    res = detector_launch(bundle, mutant, "", None, lambda a: "detection took %ss" % a.get("seconds"))
    if isinstance(res, int):
        return res
    answer, dns, web = res
    for u in answer.get("unreadable", []):
        print("unreadable: " + u)
    results = judge(answer)
    # the positive control: the answers came from the local network (the detector asked it over DNS and HTTP), not from nowhere
    results.append(local_network_case(dns.log, web.log))
    return report(results, mutant)


def main_slice_detect(bundle):
    res = detector_launch(bundle, False, " --slice-detect", None, lambda a: "%d scenarios read" % len(a))
    if isinstance(res, int):
        return res
    answer, dns, web = res
    results = judge_detect(answer)
    results.append(local_network_case(dns.log, web.log))
    lines = ["%-10s %s  %s" % ("PASS" if ok else "FAIL", n, d) for n, ok, d in results]
    failed = sum(1 for _, ok, _ in results if not ok)
    lines.append("%d checks, %d failed, 0 not judged" % (len(results), failed))
    print("\n".join(lines)); return 0 if failed == 0 else 1


# ---------------------------------------------------------------- slice search (lane search-checks): the real engine answers a search, before and after a restart
#
# The owner's fault (2026-11-11): the real engine had no answer to the search command (rust_core.rs's catch-all said "not built yet") and every check
# searched on the TEST engine, so nothing went red. This slice drives the real screen on the real engine: the keys and the field are the owner's, the
# answers are read from the engine's own state (`search/last/*`, `people/*`). Spec: SPEC A141 to A143, docs/lanes/search-forms.md ("For the checks helper").

SEARCH_BAR, SEARCH_RES, SEARCH_REFUSED = "mac-search.bar", "mac-search.results", "mac-search.refused"
PAIR_CONSULTANTS, PAIR_ADVISOR = "search-check:consultants", "search-check:advisor"      # the engine's seeded pair (WHOOSH_CORE_CHECK_SEARCH=1, search_cmd.rs CHECK_PAIR)
SEARCH_WORD = "consultant"           # the mail PAIR_CONSULTANTS holds only "consultants": the typed word is a prefix of it (A141)
SEARCH_WORD_VARIANTS = ("consultant", "Consultant")   # the same search twice over: the screen runs a text it already ran only once, so a re-ask alternates the capital
# Shares no word with either seeded mail; near both in meaning, far from the decoys. MEASURED on main86 with the real model (2026-11-12, hidden,
# _captures/lead/main86/meaning-query/): this query and two others ("hired experts examining finances", "professional advisory services") rank the
# seeded pair first. The query used before, "freelance expert who gives guidance", did not: the model scored advisor 0.652, the sent dinner mail
# 0.622, consultants 0.592; its pass on main85 was the date order (the scores were not served there: "tail scores [-2, ...]").
MEANING_ONLY_QUERY = os.environ.get("WHOOSH_CHECK_MEANING_QUERY") or "external specialists assessing company strategy"
FINAL_STAGES = ("ranked", "no_model", "model_failed", "failed", "filtered")
NO_VECTOR_SCORE_CEILING = -1.5       # the engine's NO_VECTOR_SCORE is -2.0 (a mail with no vector yet); every cosine is above this
SEARCH_STORE_WAIT_S = 90             # a check bound, not a product number: the account's sync, the pair and the decoys indexed
SEARCH_SETTLE_S = 150                # a check bound: the model's first load and the embedding of the six mails
SEARCH_ASK_WAIT_S = 60               # a check bound: one search, from Return to its final stage
SEARCH_PEOPLE_WAIT_S = 60            # a check bound: the people list is built beside the open
SEARCH_HIT_READ_MAX = 40             # hits read per search (the mailbox holds six mails)

# the local server's mail: three decoys far from both seeded mails in meaning, and one mail the owner sent (minutes = uids, newest last)
SEARCH_INBOX = [("Dana Reyes <dana.reyes@example.test>", "Parking permit renewal", "search-1",
                 "Your parking permit for the north car park runs out at the end of the month. Renew it at the office desk."),
                ("Eli Moss <eli.moss@example.test>", "Salmon recipe for Sunday", "search-2",
                 "Bake the salmon for twelve minutes with lemon and dill. Serve with boiled potatoes."),
                ("Gus Hale <gus.hale@example.test>", "Football match on Saturday", "search-3",
                 "The match starts at three. Bring the blue scarf and the tickets.")]
SEARCH_SENT = ("Fay Lund <fay.lund@example.test>", "Dinner on Friday", "search-4",
               "Fay, a table for four on Friday at eight. Tell me if the time does not suit you.")
SEARCH_MAILS = 6                      # the stored mails in scope: the engine's pair, the three decoys, the sent mail
FROM_WORD, TO_WORD = "dana", "fay"
FROM_SUBJECT, TO_SUBJECT = SEARCH_INBOX[0][1], SEARCH_SENT[1]
DECOY_DATE_ORDER = [SEARCH_SENT[1], SEARCH_INBOX[2][1], SEARCH_INBOX[1][1], SEARCH_INBOX[0][1]]   # newest first (minutes 4, 3, 2, 1)
SEARCH_ALL_CASES = ("consultant-is-an-exact-match-though-the-mail-holds-consultants", "advisor-mail-ranks-first-by-meaning-after-the-exact-group",
                    "a-query-with-no-shared-word-ranks-the-seeded-pair-first-by-meaning", "the-search-ends-ranked-or-no-model-with-every-mail-listed",
                    "from-filter-keeps-only-that-sender", "to-filter-keeps-only-the-mail-the-owner-sent", "people-list-a-recipient-of-sent-mail-and-are-ready",
                    "a-restart-gives-the-same-answers", "an-engine-refusal-is-said-in-words-not-as-a-list",
                    "the-reader-shows-the-selected-result-on-the-real-engine", "a-search-typed-at-once-after-a-start-ends-ranked-never-with-the-missing-model-sentence",
                    "a-copy-with-no-model-keeps-no-model-and-says-so-in-words")
READER_CASE_I, LATE_CASE_I, NOMODEL_CASE_I = 9, 10, 11   # indexes into SEARCH_ALL_CASES
READER_QUERY = "example"             # the letters inside every sender address of the local server's mail (example.test): at least two mails hold them (A149: letters inside words and addresses)
READER_STEPS = ("", "Down", "Down", "Up")   # the first result is selected at once; then the keys the owner presses
READER_WAIT_S = 3.0                  # a check bound: the reader follows the selected result after its own quiet time (SPEC A138)
LATE_FIRST_WAIT_S = 60               # a check bound: the first search of a copy that has just started, to its final stage
LATE_HANDOVER_DELAY_MS = 3000        # item 26a6f568: a rig-only delay of the model's hand-over (Sources/Features/BackgroundWork/BackgroundWork.swift `EmbedderHandOver.rigDelayMs`, read only on a test copy), so the window "the search
                                     # is typed before the model arrives" exists in EVERY run that has a model folder; the copy given no model gets none (nothing is handed over there)
LATE_TYPED_IN_TIME_S = 2.0           # a check bound: a search typed later than this after the restarted copy answered was typed too late for the window to be judged (the delay runs from the app's start)
try:
    LATE_MODEL_MISSING_WORDS = re.search(r'MODEL_MISSING_WORDS: &str = "([^"]+)"', open(os.path.join(ROOT, "core", "crates", "engine", "src", "crawl.rs"), encoding="utf-8").read()).group(1)
except (OSError, AttributeError):
    LATE_MODEL_MISSING_WORDS = None   # read from the engine's own source (core/crates/engine/src/crawl.rs), never typed here
SEARCH_MUTANT_CASES = SEARCH_ALL_CASES[:6] + SEARCH_ALL_CASES[7:8]   # a build with no answer to the search command turns these red (the people list is not the search arm; the refusal case is NOT JUDGED)
SEARCH_MUTANT_NAME = "search-never-answers"    # the engine hook this slice needs (not built: see docs/lanes/search-checks.md); one line in rust_core.rs's Mutant list


def load_search_mail(fake, mail):
    """INBOX uids 1 to 3 (minutes 1 to 3), Sent uid 4 (minute 4): the owner's mail to Fay, who appears nowhere else."""
    owner = "Owner <%s>" % fake.ADDRESS
    mail.boxes["INBOX"] = [(i + 1, flow_raw(frm, fake.ADDRESS, subject, mid, i + 1, body=body)) for i, (frm, subject, mid, body) in enumerate(SEARCH_INBOX)]
    to, subject, mid, body = SEARCH_SENT
    mail.boxes["Sent"] = [(4, flow_raw(owner, to, subject, mid, 4, body=body))]


def jv(v):
    """A state value as the socket gives it: JSON text or already decoded; a bare word stays the word."""
    import json
    if isinstance(v, str):
        try:
            return json.loads(v)
        except ValueError:
            return v
    return v


def cv(c, path):
    """One `core state` read; a path the build does not serve reads as absent (None), never as a crash of the check."""
    try:
        return jv(c.core(path))
    except Exception:       # noqa: BLE001  Unreadable: the path is not served
        return None


def read_search_state(c, asked, expect_in):
    """The engine's last search as the socket reads it, and the screen's rows beside it."""
    st = {"asked": asked, "query": cv(c, "search/last/query"), "stage": cv(c, "search/last/stage")}
    try:
        n = int(cv(c, "search/last/count") or 0)
    except (TypeError, ValueError):
        n = 0
    st["count"] = n
    st["hits"] = [{"id": cv(c, "search/last/hit/%d/message_id" % i), "matched_in": cv(c, "search/last/hit/%d/matched_in" % i),
                   "score": cv(c, "search/last/hit/%d/score" % i)} for i in range(min(n, SEARCH_HIT_READ_MAX))]
    st["screen"] = {r.get("id"): r.get("subject") for r in c.rows(SEARCH_RES)}
    st["refused"] = c.rows(SEARCH_REFUSED)
    if st["stage"] in ("no_model", "model_failed"):
        st["reason"] = cv(c, "search/last/reason")
    return st


def query_matches(q, asked, expect_in):
    return isinstance(q, str) and (q == asked if not expect_in else all(w in q.lower() for w in expect_in))


def run_search(c, text, expect_in=(), wait_s=SEARCH_ASK_WAIT_S):
    """Type the words in the real field, press Return, wait for the engine's FINAL stage for exactly this query (the first list arrives at once, the
    ranked one when the query's vector is ready), read what the engine says. Never a stand-in: the state paths are the engine's."""
    t0 = time.time()
    if c.uistate().get("surface") != "mac-search":
        # the open door, not a key: which key opens the search depends on the preset the engine serves ("/" is the Superhuman preset's;
        # on the real engine of main84 "/" opened nothing and the click on the field then "did nothing": the run ended NOT JUDGED)
        c.click("mac-search.open")
    c.click(SEARCH_BAR); c.key("⌘A"); c.type(text)
    bar = (c.rows(SEARCH_BAR) or [{}])[0]
    if not (bar.get("launched") is True and bar.get("dropdown") is False and bar.get("text") == text):
        c.key("Enter")                                   # the same text already running would open the first result instead (SearchMacActs.enter)
    st = None
    while True:
        st = read_search_state(c, text, expect_in)
        if query_matches(st["query"], text, expect_in) and st["stage"] in FINAL_STAGES:
            break
        if time.time() - t0 > wait_s:
            break
        time.sleep(0.3)
    st["seconds"] = round(time.time() - t0, 1)
    return st


def exact_ids(st):
    out = []
    for h in st.get("hits") or []:
        if h.get("matched_in") == "meaning":
            break
        out.append(h.get("id"))
    return out


def meaning_ids(st):
    return [h.get("id") for h in st.get("hits") or [] if h.get("matched_in") == "meaning"]


def settled(st):
    """The ranking can no longer change by embedding: every mail has a vector (or there is no model to wait for)."""
    if st.get("stage") == "no_model":
        return True
    return st.get("stage") == "ranked" and st.get("count", 0) >= SEARCH_MAILS and all((h.get("score") or -2.0) > NO_VECTOR_SCORE_CEILING for h in st["hits"])


def settle_search(c, wait_s=SEARCH_STORE_WAIT_S + SEARCH_SETTLE_S, end_on=None):
    """Ask for SEARCH_WORD until the pair is stored, the exact group holds the consultants mail and the ranking has settled; the last answer is kept
    with the seconds it took and whether it settled. `end_on`: the round ends only on this exact text (the round after the restart ends on the
    text the round before ended on: 'consultant' and 'Consultant' rank two far mails differently by meaning, and an order compared across two
    spellings read 'same order False' on main143, run r017)."""
    t0, i, st = time.time(), 0, None
    while True:
        text = SEARCH_WORD_VARIANTS[i % 2]
        st = run_search(c, text)
        stored = PAIR_CONSULTANTS in exact_ids(st) and st.get("count", 0) >= SEARCH_MAILS
        if stored and settled(st) and end_on in (None, text):
            st["settled"] = True
            break
        if st.get("stage") in ("model_failed", "failed") or st.get("query") is None or time.time() - t0 > wait_s:   # a query still unset: the engine never answered the command
            st["settled"] = False
            break
        i += 1
        time.sleep(2)
    st["waited"] = round(time.time() - t0, 1)
    return st


def read_people(c):
    out = {}
    t0 = time.time()
    while time.time() - t0 < SEARCH_PEOPLE_WAIT_S:
        out["ready"] = cv(c, "people/ready")
        if out["ready"] is True:
            break
        time.sleep(0.5)
    for key, letters in (("fay", TO_WORD), ("dana", FROM_WORD), ("owner", "owner")):
        out[key] = cv(c, "people/prefix/" + letters)
    return out


def restart(c_old):
    """The copy is stopped and started again on the SAME data folder (launch-one.sh --relaunch: quits by its pid, starts the same copy on $D)."""
    import subprocess
    d = os.environ.get("D") or os.environ.get("WHOOSH_SOCKET_DIR", "")
    if not d:
        return None, "no run folder ($D) in the environment: --relaunch needs it"
    try:
        c_old.close()
    except Exception:
        pass
    p = subprocess.run(["sh", os.path.join(ROOT, "scripts", "launch-one.sh"), "--relaunch"], env=dict(os.environ, D=d, WHOOSH_RIG_HIDDEN="1"), capture_output=True, text=True, cwd=ROOT)
    if p.returncode != 0:
        return None, "launch-one --relaunch exit %d: %s" % (p.returncode, (p.stdout + p.stderr)[-300:])
    from lib.ctl import Ctl, CouldNotDrive
    t0 = time.time()
    while True:
        try:
            nc = Ctl(d).connect()
            nc.uistate()
            return nc, None
        except Exception as e:
            if time.time() - t0 > 60:
                return None, "no socket after the relaunch: %s" % e
            time.sleep(0.5)


def search_round(c, with_meaning_only, end_on=None):
    """One round of the cases that are asked again after the restart: a (and the ranking), the two filters, the people."""
    out = {"main": settle_search(c, end_on=end_on)}
    if with_meaning_only:
        out["meaning_only"] = run_search(c, MEANING_ONLY_QUERY)
    out["from"] = run_search(c, "from:" + FROM_WORD, expect_in=[FROM_WORD])
    out["to"] = run_search(c, "to:" + TO_WORD, expect_in=[TO_WORD])
    out["people"] = read_people(c)
    return out


def reader_round(c):
    """The reader on the real engine (SPEC A148): after a search whose answer holds at least two matches, the first result is selected; then Down, Down, Up as the owner
    presses them. After each, once the reader's quiet time has passed, the reader's mail id and the selected result's id are read from the screen's rows. Read only."""
    st = run_search(c, READER_QUERY)
    out = {"asked": READER_QUERY, "stage": st.get("stage"), "count": st.get("count"), "steps": []}
    for key in READER_STEPS:
        if key:
            c.key(key)
        end = time.time() + READER_WAIT_S
        while True:
            rows = c.rows(SEARCH_RES)
            sel = [r.get("id") for r in rows if r.get("selected")]
            shown = [r.get("id") for r in c.rows("mac-main.reader.subject")]
            if (len(sel) == 1 and shown == sel) or time.time() > end:
                break
            time.sleep(0.05)
        out["steps"].append({"key": key or "none", "listed": [r.get("id") for r in rows], "parts": [r.get("part") for r in rows], "selected": sel, "reader": shown})
    return out


def late_search(c):
    """The first search typed on a copy that has just started: every stage the engine reports for it, polled fast, and whether it said the missing-model sentence. With the
    model handed over after the start (the app says it is coming first) the stages are words, then ranked; the window exists only when the first list came before the hand-over."""
    t0 = time.time()
    try:
        c.click("mac-search.open")
        c.click(SEARCH_BAR); c.key("⌘A"); c.type(SEARCH_WORD); c.key("Enter")
    except Exception as e:       # noqa: BLE001
        return {"error": "%s: %s" % (type(e).__name__, e)}
    typed_s = round(time.time() - t0, 2)
    seen, reasons, stage = [], [], None
    while time.time() - t0 < LATE_FIRST_WAIT_S:
        q, stage = cv(c, "search/last/query"), cv(c, "search/last/stage")
        if q == SEARCH_WORD and stage:
            if not seen or seen[-1] != stage:
                seen.append(stage)
                reasons.append(cv(c, "search/last/reason"))
            if stage in FINAL_STAGES:
                break
        time.sleep(0.05)
    return {"stages": seen, "reasons": reasons, "seconds": round(time.time() - t0, 2), "typed_s": typed_s}


def drive_search():
    """Inside launch-one: add the local account through the sheet, let the pair and the mail be stored, ask, restart the copy, ask again."""
    sys.path.insert(0, os.path.join(ROOT, "journeys"))
    from lib.ctl import Ctl  # noqa: E402
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import realcore_mailfake as fake  # noqa: E402
    c = Ctl().connect()
    ans = {}
    c.click(A + ".open")
    c.click(A + ".address"); c.type(CHECK_ADDRESS)
    c.click(A + ".password"); c.type(fake.PASSWORD)
    c.click(A + ".continue")
    t0 = time.time()
    while time.time() - t0 < DETECT_WAIT_S:
        st = c.rows(A + ".state")
        if st and st[0].get("stage") not in ("detecting", "entry"):
            break
        time.sleep(0.3)
    c.click(A + ".add")
    stage = None
    t0 = time.time()
    while time.time() - t0 < DETECT_WAIT_S:
        st = c.rows(A + ".state"); stage = st[0].get("stage") if st else None
        if stage in ("added", "summary"):
            break
        time.sleep(0.3)
    ans["add"] = {"stage": stage}
    try:
        err = c.rows(A + ".error"); ans["add"]["error"] = err[0].get("text") if err else None
    except Exception:
        ans["add"]["error"] = None
    try:
        c.click(A + ".done")
    except Exception as e:
        ans["add"]["done"] = str(e)
    t0 = time.time()
    rows = []
    while time.time() - t0 < FLOW_WAIT_S:
        try:
            rows = c.rows("mac-main.list")
        except Exception:
            rows = []
        if len(rows) >= len(SEARCH_INBOX):
            break
        time.sleep(0.5)
    ans["add"]["list"] = sorted(r.get("subject", "") for r in rows)
    ans["core"] = c.uistate().get("core")
    ans["before"] = search_round(c, True)
    ans["reader"] = reader_round(c)
    # g: words the app never hands to the engine (only punctuation): the engine's own refusal cannot be reached through the screen
    pre = cv(c, "search/last/query")
    c.click(SEARCH_BAR); c.key("⌘A"); c.type("???")
    try:
        c.key("Enter")
    except Exception:
        pass
    time.sleep(0.5)
    ans["refusal"] = {"asked": "???", "engine_query_before": pre, "engine_query_after": cv(c, "search/last/query"), "refused": c.rows(SEARCH_REFUSED), "list_rows": len(c.rows(SEARCH_RES))}
    if ans["refusal"]["engine_query_after"] == pre and not ans["refusal"]["refused"]:
        # fix-realcore: the screen keeps such words from the engine, so the engine's own refusal is asked through the socket door the app has for it since
        # ca03fa4f2 (Sources/ControlSocket/Verbs.swift `core run search <words...>`, Command::Search as given): empty words, the engine answers in words
        sock = ans["refusal"]["socket"] = {"error": None}
        try:
            c.send("core", "run", "search")
        except Exception as e:      # noqa: BLE001  the refusal comes back as the reply's error, in the engine's words
            sock["error"] = str(e)[-300:]
        ans["refusal"]["engine_query_after_socket"] = cv(c, "search/last/query")
    nc, why = restart(c)
    ans["restart"] = {"ok": nc is not None, "error": why}
    if nc is not None:
        ans["late"] = late_search(nc)          # the first thing typed on the new copy, before anything else is read from it
        t0 = time.time()
        while time.time() - t0 < FLOW_WAIT_S:
            try:
                if len(nc.rows("mac-main.list")) >= len(SEARCH_INBOX):
                    break
            except Exception:
                pass
            time.sleep(0.5)
        asked = (ans["before"].get("main") or {}).get("query")
        ans["after"] = search_round(nc, False, end_on=asked if asked in SEARCH_WORD_VARIANTS else None)
    return ans


def ids_of_subjects(st, subjects):
    """The hit ids whose subject (as the screen lists it) is one of `subjects`, in the engine's order."""
    return [h["id"] for h in st.get("hits") or [] if (st.get("screen") or {}).get(h["id"]) in subjects]


def judge_filter(st, word, subject):
    """A filter alone keeps exactly the mails that pass and nothing else: one mail, listed, the stage `filtered`."""
    shown = [(st.get("screen") or {}).get(h["id"]) for h in st.get("hits") or []]
    ok = (query_matches(st.get("query"), "", [word]) and st.get("count") == 1 and shown == [subject] and st.get("stage") == "filtered")
    return ok, "asked %r, the engine's query %r, stage %r, count %r, listed subjects %r (wanted [%r])" % (st.get("asked"), st.get("query"), st.get("stage"), st.get("count"), shown, subject)


def person(rows, address):
    return next((r for r in rows or [] if isinstance(r, dict) and r.get("address") == address), None)


FAY, DANA = "fay.lund@example.test", "dana.reyes@example.test"


def judge_people(p):
    fay, dana, owner = person(p.get("fay"), FAY), person(p.get("dana"), DANA), p.get("owner")
    ok = (p.get("ready") is True and fay is not None and (fay.get("to_count") or 0) > 0 and fay.get("from_count") == 0
          and dana is not None and dana.get("from_count") == 1 and dana.get("to_count") == 0 and owner == [])
    return ok, "people/ready %r; people/prefix/%s %r; people/prefix/dana %r; people/prefix/owner %r" % (p.get("ready"), TO_WORD, p.get("fay"), p.get("dana"), p.get("owner"))


def judge_reader(rd):
    """(ok, detail). The reader's mail is the selected result's after each key, and always one of the listed results. NOT JUDGED with fewer than two matches (a search with one
    result cannot show that the reader follows a move)."""
    steps = rd.get("steps") or []
    if not steps:
        return None, "the reader was not read (no steps recorded)"
    matches = sum(1 for p in steps[0].get("parts") or [] if p == "match")
    if matches < 2:
        return None, "asked %r: %r matches listed (parts %r): fewer than two, so the reader following a move cannot be judged" % (rd.get("asked"), matches, steps[0].get("parts"))
    got = [(len(x["selected"]) == 1 and x["reader"] == x["selected"], bool(x["reader"]) and all(i in x["listed"] for i in x["reader"])) for x in steps]
    ok = all(a and b for a, b in got)
    return ok, "asked %r, %d matches; per key %s: (the reader shows the selected result, the reader's mail is one of the listed results) %r; selected %r, reader %r" % (
        rd.get("asked"), matches, [x["key"] for x in steps], got, [x["selected"] for x in steps], [x["reader"] for x in steps])


def judge_late(ans):
    """(ok, detail) for the first search on the restarted copy. Judged only with a model folder given and only when the window existed (the first list stage is `words`);
    then the search must end `ranked` and never report `no_model`, `model_failed` or the missing-model sentence on the way."""
    late = ans.get("late")
    if late is None:
        return None, "the copy was not restarted: no first search on a new copy was typed"
    if late.get("error"):
        return None, "the first search on the new copy could not be typed: %s" % late["error"]
    if not ans.get("weights_given"):
        return None, "no model folder was given (WHOOSH_EMBED_WEIGHTS): a copy without a model ends no_model by design, see %s; stages seen %r" % (SEARCH_ALL_CASES[NOMODEL_CASE_I], late.get("stages"))
    stages, reasons = late.get("stages") or [], late.get("reasons") or []
    delay = ans.get("handover_delay_ms") or 0
    if not stages or stages[0] != "words":
        typed = late.get("typed_s")
        if delay and typed is not None and typed <= LATE_TYPED_IN_TIME_S:
            return False, ("the copy was started with WHOOSH_RIG_EMBED_HANDOVER_DELAY_MS=%d and the search was typed %ss after it answered, inside the window, yet the first stage seen was %r, not words: "
                           "the build ignores the delay or the engine ranked before the model was handed over; stages %r" % (delay, typed, stages[0] if stages else None, stages))
        return None, ("the window did not exist in this run: the first stage seen was %r, not words%s; stages %r" % (
            stages[0] if stages else None, (" (the hand-over was delayed %d ms but the search was typed %ss after the copy answered, over the %ss the check allows)" % (delay, typed, LATE_TYPED_IN_TIME_S)) if delay
            else " (the model was handed over before the search was typed, and no hand-over delay was asked of this copy)", stages))
    bad = [x for x in stages if x in ("no_model", "model_failed", "failed")] + [r for r in reasons if r]
    return (stages[-1] == "ranked" and not bad), "stages %r (words first, ranked last), reasons %r (none while it waits), after %ss; the hand-over was delayed %d ms, the search typed %ss after the copy answered" % (stages, reasons, late.get("seconds"), delay, late.get("typed_s"))


def judge_nomodel(ans):
    """(ok, detail) for a copy given no model: the first search ends `no_model` with the engine's sentence (read from its source). NOT JUDGED when the copy has a model."""
    late = ans.get("late")
    if late is None or late.get("error"):
        return None, "no first search on a new copy: %s" % ((late or {}).get("error") or "the copy was not restarted")
    if ans.get("weights_given"):
        return None, "a model folder was given: the copy has a model, so the no-model answer is not asked of it; stages %r" % (late.get("stages"),)
    stages, reasons = late.get("stages") or [], late.get("reasons") or []
    if stages and stages[-1] != "no_model" and stages[-1] in ("ranked",):
        return None, "the copy found a model of its own (stage ranked): nothing to judge about a missing model"
    ok = bool(stages) and stages[-1] == "no_model" and LATE_MODEL_MISSING_WORDS is not None and reasons[-1] == LATE_MODEL_MISSING_WORDS
    return ok, "stages %r, the sentence given %r (the engine's own, crawl.rs MODEL_MISSING_WORDS: %r)" % (stages, reasons[-1] if reasons else None, LATE_MODEL_MISSING_WORDS)


def judge_search(ans):
    """[(name, ok, detail)]: ok True/False, None when the case cannot be judged (no model in the copy; an engine refusal the screen cannot reach)."""
    b = ans.get("before") or {}
    m = b.get("main") or {}
    mo = b.get("meaning_only") or {}
    after = ans.get("after")
    res = []
    hits_text = "stage %r, query %r, count %r, order %r, matched_in %r" % (m.get("stage"), m.get("query"), m.get("count"), [h.get("id") for h in m.get("hits") or []], [h.get("matched_in") for h in m.get("hits") or []])
    exact = exact_ids(m)
    a_ok = (PAIR_CONSULTANTS in exact and (next((h for h in m.get("hits") or [] if h.get("id") == PAIR_CONSULTANTS), {}).get("matched_in") not in (None, "meaning")))
    res.append((SEARCH_ALL_CASES[0], a_ok, "the exact group is %r after %ss (settled %r); %s" % (exact, m.get("waited"), m.get("settled"), hits_text)))
    # b: the advisor mail, no word in common with the query, ranks first in the meaning group when a model answers
    if m.get("stage") == "no_model":
        why = "the copy has no embedding model (stage no_model, reason %r): nothing ranks by meaning, so the meaning ranking cannot be judged; pass WHOOSH_EMBED_WEIGHTS to the check to give the copy one" % (m.get("reason"),)
        res.append((SEARCH_ALL_CASES[1], None, why))
        res.append((SEARCH_ALL_CASES[2], None, why))
    else:
        mean = meaning_ids(m)
        res.append((SEARCH_ALL_CASES[1], m.get("stage") == "ranked" and bool(mean) and mean[0] == PAIR_ADVISOR,
                    "the meaning group is %r (the advisor mail must be first, then the decoys); %s" % (mean, hits_text)))
        mo_ids = [h.get("id") for h in mo.get("hits") or []]
        res.append((SEARCH_ALL_CASES[2], mo.get("stage") == "ranked" and exact_ids(mo) == [] and sorted(mo_ids[:2]) == sorted([PAIR_CONSULTANTS, PAIR_ADVISOR]),
                    "asked %r: stage %r, exact group %r, first two %r (must be the seeded pair in either order); every hit (id, matched in, score): %r" % (MEANING_ONLY_QUERY, mo.get("stage"), exact_ids(mo), mo_ids[:2], [(h.get("id"), h.get("matched_in"), h.get("score")) for h in mo.get("hits") or []])))
    # c: the stage ends ranked, or no_model with the word hits then date order; every mail is listed, none cut
    tail_subjects = [(m.get("screen") or {}).get(i) for i in meaning_ids(m)]
    decoys = [s for s in tail_subjects if s in DECOY_DATE_ORDER]
    scores = [h.get("score") if isinstance(h.get("score"), (int, float)) else -2.0 for h in m.get("hits") or [] if h.get("matched_in") == "meaning"]
    if m.get("stage") == "ranked":
        order_ok = all(x >= y for x, y in zip(scores, scores[1:])) and bool(scores)
        order_text = "tail scores %r (best first)" % (scores,)
    else:
        order_ok = m.get("stage") == "no_model" and decoys == DECOY_DATE_ORDER
        order_text = "tail decoys by date %r (wanted %r)" % (decoys, DECOY_DATE_ORDER)
    after_first_meaning = [h.get("matched_in") for h in (m.get("hits") or [])[len(exact):]]
    res.append((SEARCH_ALL_CASES[3], m.get("stage") in ("ranked", "no_model") and order_ok and m.get("count") == SEARCH_MAILS and len(set(i for i in [h.get("id") for h in m.get("hits") or []])) == SEARCH_MAILS
                and all(x == "meaning" for x in after_first_meaning),
                "stage %r (must end ranked, or no_model), %d of %d mails listed, no exact hit after the first meaning hit %r; %s" % (m.get("stage"), m.get("count") or 0, SEARCH_MAILS, all(x == "meaning" for x in after_first_meaning), order_text)))
    fr, to = b.get("from") or {}, b.get("to") or {}
    res.append((SEARCH_ALL_CASES[4],) + judge_filter(fr, FROM_WORD, FROM_SUBJECT))
    res.append((SEARCH_ALL_CASES[5],) + judge_filter(to, TO_WORD, TO_SUBJECT))
    res.append((SEARCH_ALL_CASES[6],) + judge_people(b.get("people") or {}))
    # f: the restart
    if after is None:
        res.append((SEARCH_ALL_CASES[7], False, "the copy did not come back: %s" % (ans.get("restart") or {}).get("error")))
    else:
        am = after.get("main") or {}
        people_same = (person((after.get("people") or {}).get("fay"), FAY) == person((b.get("people") or {}).get("fay"), FAY))
        same_exact = exact_ids(am) == exact and PAIR_CONSULTANTS in exact_ids(am)
        same_all = sorted(h.get("id") for h in am.get("hits") or []) == sorted(h.get("id") for h in m.get("hits") or [])
        same_order = [h.get("id") for h in am.get("hits") or []] == [h.get("id") for h in m.get("hits") or []] if (m.get("settled") and am.get("settled")) else None
        fa, ta = after.get("from") or {}, after.get("to") or {}
        fok, fdetail = judge_filter(fa, FROM_WORD, FROM_SUBJECT)
        tok, tdetail = judge_filter(ta, TO_WORD, TO_SUBJECT)
        pok, pdetail = judge_people(after.get("people") or {})
        ok = same_exact and same_all and same_order is not False and fok and tok and pok and people_same and am.get("stage") == m.get("stage")
        orders = "" if same_order is not False else "; asked %r then %r, order before %r, after %r" % (m.get("query"), am.get("query"), [h.get("id") for h in m.get("hits") or []], [h.get("id") for h in am.get("hits") or []])
        res.append((SEARCH_ALL_CASES[7], ok, "after the restart: exact group %r (before %r), same mails %r, same order %r (None: a ranking not settled before and after is not compared), stage %r (before %r)%s; from: %s; to: %s; people: %s"
                    % (exact_ids(am), exact, same_all, same_order, am.get("stage"), m.get("stage"), orders, fdetail, tdetail, pdetail)))
    # g: an engine refusal reaches the owner in words
    rf = ans.get("refusal") or {}
    shown = rf.get("refused") or []
    asked_engine = rf.get("engine_query_after") != rf.get("engine_query_before")
    if shown:
        r0 = shown[0]
        res.append((SEARCH_ALL_CASES[8], bool((r0.get("text") or "").strip()) and bool((r0.get("words") or "").strip()) and rf.get("list_rows") == 0,
                    "refused row %r, rows listed %r" % (r0, rf.get("list_rows"))))
    elif asked_engine:
        res.append((SEARCH_ALL_CASES[8], False, "the engine was asked %r and answered a list, not a refusal in words (rows listed %r)" % (rf.get("engine_query_after"), rf.get("list_rows"))))
    elif rf.get("socket") is not None:
        # fix-realcore: asked through `core run search` with empty words: a refusal in words (the reply's error past the verb), and no list made of it
        err = rf["socket"].get("error")
        words = (err or "").split(": ", 1)[-1].strip()
        unreachable = err is not None and ("usage:" in words or "unknown verb" in words.lower())
        kept = rf.get("engine_query_after_socket") == rf.get("engine_query_before")
        ok = None if unreachable else (err is not None and bool(re.search(r"[^\W\d_]", words)) and kept)
        res.append((SEARCH_ALL_CASES[8], ok, "the screen keeps %r from the engine (its query stayed %r); asked through `core run search` with empty words: %s; the engine's last query %s" % (
            rf.get("asked"), rf.get("engine_query_after"), "the build has no such door: %r" % words if unreachable else ("refused in %d characters of words" % len(words) if err is not None else "ACCEPTED, no refusal"),
            "unchanged" if kept else "changed to %r" % rf.get("engine_query_after_socket"))))
    else:
        res.append((SEARCH_ALL_CASES[8], None, "the screen never hands words with no letter or digit to the engine (SearchHits.swift engineQuery returns nil for %r: the engine's query stayed %r), so the engine's refusal of empty words "
                    "cannot be reached through the screen; the hook needed: a socket verb `core run search <words>` that sends Command::Search and returns the reply or the error" % (rf.get("asked"), rf.get("engine_query_after"))))
    res.append((SEARCH_ALL_CASES[READER_CASE_I],) + judge_reader(ans.get("reader") or {}))
    res.append((SEARCH_ALL_CASES[LATE_CASE_I],) + judge_late(ans))
    res.append((SEARCH_ALL_CASES[NOMODEL_CASE_I],) + judge_nomodel(ans))
    return res


WEIGHTS_CASE = "model-folder-is-a-real-folder"


def weights_refusal(folder):
    """fix-realcore round 4 (item 183fe10a): None when the model folder handed to the copy (WHOOSH_EMBED_WEIGHTS) is a real directory; else the reason in words.
    A symbolic link is refused: a copy given one read stage model_failed (_captures/lead/main85/realcore-search-model-debug2.log), so a meaning case run
    through it judges the link, not the engine. No folder given: None (the meaning cases are then NOT JUDGED by their own rule)."""
    if not folder:
        return None
    path = folder.rstrip("/") or folder
    if os.path.islink(path):
        return "the model folder %s is a symbolic link (to %s): give the real folder it points to" % (path, os.path.realpath(path))
    if not os.path.isdir(path):
        return "the model folder %s is not a folder on this Mac" % path
    return None


def selftest_argv():
    """normal_argv: flags handed as one argument are split; a path with a space is kept; an unknown --slice word is named."""
    here = os.path.dirname(os.path.abspath(__file__))
    a, u = normal_argv(["x", here, "--slice-attachsend --mutant"])
    b, v = normal_argv(["x", here, "--slice-attachsned"])
    c, w = normal_argv(["x", here, "--slice-crawl", "--mutant=index-never-writes"])
    ok = a == ["x", here, "--slice-attachsend", "--mutant"] and u is None and v == "--slice-attachsned" and w is None and c[2:] == ["--slice-crawl", "--mutant=index-never-writes"]
    print("selftest arguments: flags in one argument split, an unknown slice named: %s" % ("yes" if ok else "NO"))
    return ok


def selftest_weights():
    """weights_refusal on a real folder (None), a symbolic link to it (refused), a missing path (refused) and no folder (None). The folders are made under a
    fresh temporary folder and removed."""
    import shutil
    import tempfile
    top = tempfile.mkdtemp(prefix="whoosh-weights-selftest-")
    try:
        real = os.path.join(top, "model")
        os.mkdir(real)
        link = os.path.join(top, "model-link")
        os.symlink(real, link)
        ok = (weights_refusal(real) is None and weights_refusal(real + "/") is None and weights_refusal("") is None
              and "symbolic link" in (weights_refusal(link) or "") and "symbolic link" in (weights_refusal(link + "/") or "")
              and "not a folder" in (weights_refusal(os.path.join(top, "absent")) or ""))
    finally:
        shutil.rmtree(top, ignore_errors=True)
    print("selftest model folder: a real folder taken, a symbolic link refused, a missing path refused: %s" % ("yes" if ok else "NO"))
    return ok


def main_slice_search(bundle, mutant):
    """The local servers live in THIS process (threads) for the whole launch (the restart too: the same ports); the app reaches 127.0.0.1 only."""
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import realcore_mailfake as fake
    import json
    weights = os.environ.get("WHOOSH_EMBED_WEIGHTS", "")     # the lead's model folder, only when the lead gives one; the check never looks for the owner's own
    refused = weights_refusal(weights)
    if refused:      # no copy is started on a folder that is not a real one
        print("FAIL       search/%s  %s" % (WEIGHTS_CASE, refused))
        print("1 checks, 1 failed, 0 not judged"); return 1
    first = search_launch(bundle, fake, json, weights, mutant)
    if first is None:
        return 1
    ans, results, lines = first
    if weights:
        results = [(WEIGHTS_CASE, True, "the model folder is a real directory, not a symbolic link")] + list(results)
    if weights and not mutant:
        # fix-realcore round 2 (main95 s2-002: the no-model case NOT JUDGED because the copy had a model): a SECOND hidden copy, started without
        # WHOOSH_EMBED_WEIGHTS (a test copy reads the weights from that folder or its own run folder only, Sources/Features/Embedding/GemmaWeights.swift:74),
        # answers the no-model case; every other case stays the first copy's
        second = search_launch(bundle, fake, json, "", False, quiet=True)
        results = merge_nomodel(results, None if second is None else second[1])
        lines.append("NO-MODEL-COPY: the case %s is read from a second hidden copy started without the model folder" % SEARCH_ALL_CASES[NOMODEL_CASE_I])
    lines += ["%-10s %s/%s  %s" % ("PASS" if ok else ("NOT JUDGED" if ok is None else "FAIL"), "search", n, d) for n, ok, d in results]
    failed = sum(1 for _, ok, _ in results if ok is False)
    unjudged = sum(1 for _, ok, _ in results if ok is None)
    if mutant:
        red = all(ok is False for n, ok, _ in results if n in SEARCH_MUTANT_CASES)      # unchanged by fix-realcore round 2 (its red set on a real run is not measured yet)
        lines.append("mutant %s: %s" % (SEARCH_MUTANT_NAME, "RED as it must be (the engine never answered the search command, the owner's fault)" if red else "SURVIVED: a search was answered by an engine that never answers it (or the build has no such switch yet: docs/lanes/search-checks.md)"))
        lines.append("%d checks, %d failed, %d not judged (mutant run)" % (len(results), failed, unjudged))
        print("\n".join(lines)); return 0 if red else 1
    lines.append("%d checks, %d failed, %d not judged" % (len(results), failed, unjudged))
    print("\n".join(lines)); return 0 if failed == 0 and unjudged == 0 else 1


def merge_nomodel(results, second):
    """The first copy's results with its no-model case replaced by the second copy's (started without a model folder); NOT JUDGED when that copy gave none."""
    name = SEARCH_ALL_CASES[NOMODEL_CASE_I]
    got = next((x for x in second or [] if x[0] == name), None)
    new = (name, None, "the second copy, started without a model folder, gave no answer") if got is None else (name, got[1], "second copy started without WHOOSH_EMBED_WEIGHTS: %s" % got[2])
    return [new if n == name else (n, ok, d) for n, ok, d in results]


def search_launch(bundle, fake, json, weights, mutant, quiet=False):
    """One hidden copy over fresh local servers: (answer, results, header lines), or None (printed) when the copy gave no answer."""
    try:
        mail, imap, smtp = fake.start(0, 0, inbox=0)
    except OSError as e:
        print("the local servers did not start: %s" % e)
        if not quiet:
            print("1 checks, 0 failed, 1 not judged")
        return None
    load_search_mail(fake, mail)
    ports = "%d:%d" % (imap.server_address[1], smtp.server_address[1])
    extra = ["WHOOSH_CORE_PLAIN_LOOPBACK=1", "WHOOSH_CORE_CHECK_SERVERS=" + ports, "WHOOSH_CORE_CHECK_SEARCH=1"]
    if weights:
        extra.append("WHOOSH_EMBED_WEIGHTS=" + weights)
        extra.append("WHOOSH_RIG_EMBED_HANDOVER_DELAY_MS=%d" % LATE_HANDOVER_DELAY_MS)    # kept by the relaunch (launch-one saves LAUNCH_ENV), so the restarted copy hands the model over late
    if mutant:
        extra.append("WHOOSH_CORE_MUTANT=" + SEARCH_MUTANT_NAME)
    env = dict(os.environ, WHOOSH_RIG_HIDDEN="1", LAUNCH_ENV=" ".join(["WHOOSH_RIG_HIDDEN=1"] + extra))
    env.pop("WHOOSH_FIXTURE_CORE", None)
    if not weights:
        env.pop("WHOOSH_EMBED_WEIGHTS", None)
    cmd = ["sh", os.path.join(ROOT, "scripts", "launch-one.sh"), bundle, "--", "--rig-hidden",
           "sh", "-c", 'WHOOSH_SOCKET_DIR="$D" python3 "$0" --inner --slice-search', os.path.abspath(__file__)]
    r = launch(cmd, env)
    imap.shutdown(); smtp.shutdown()
    out = r.stdout + r.stderr
    line = next((l for l in out.splitlines() if l.startswith("ANSWER ")), None)
    if line is None:
        print(out.rstrip()[-3000:])
        print("launcher exit %d" % r.returncode)
        if not quiet:
            print("1 checks, 0 failed, 1 not judged")
        return None
    ans = json.loads(line[len("ANSWER "):])
    ans["weights_given"] = bool(weights)
    ans["handover_delay_ms"] = LATE_HANDOVER_DELAY_MS if weights else 0
    print("launcher exit %d; the local server saw: %s; model folder %s; %s" % (r.returncode, sorted({l.split(" ")[0] for l in mail.log}),
          "given by the lead" if weights else "NOT given (the copy finds its own or none)", next((l for l in out.splitlines() if l.startswith("instances:")), "")))
    results = judge_search(ans)
    lines = [] if quiet else ["SEEDED-SLICE-SEARCH: the mail pair is SEEDED by the engine (WHOOSH_CORE_CHECK_SEARCH=1): this proves the engine path and the restart, NOT a first run of the installed app"]
    return ans, results, lines


def search_answer(mutant=None, model=True):
    """The self-test's stand-in for the app: the answers of a correct engine (or one defect, `mutant`), in the shape drive_search returns. No app is launched."""
    scr = {PAIR_CONSULTANTS: "Visit from the consultants", PAIR_ADVISOR: "An outside advisor", "m-dana": FROM_SUBJECT, "m-eli": SEARCH_INBOX[1][1], "m-gus": SEARCH_INBOX[2][1], "m-fay": TO_SUBJECT}
    def hit(i, mi, s): return {"id": i, "matched_in": mi, "score": s}
    ranked = [hit(PAIR_CONSULTANTS, "subject", 3.1), hit(PAIR_ADVISOR, "meaning", 0.71), hit("m-eli", "meaning", 0.12), hit("m-dana", "meaning", 0.05), hit("m-fay", "meaning", 0.02), hit("m-gus", "meaning", 0.01)]
    by_date = [hit(PAIR_CONSULTANTS, "subject", 3.1), hit("m-fay", "meaning", -2.0), hit("m-gus", "meaning", -2.0), hit(PAIR_ADVISOR, "meaning", -2.0), hit("m-eli", "meaning", -2.0), hit("m-dana", "meaning", -2.0)]
    def state(asked, query, stage, hits, **kw):
        d = {"asked": asked, "query": query, "stage": stage, "count": len(hits), "hits": hits, "screen": dict(scr), "refused": [], "seconds": 1.0}
        d.update(kw); return d
    def round_(first):
        if mutant == "no-search-arm":
            main = state(SEARCH_WORD, None, None, [], settled=False, waited=240.0)
            return {"main": main, "meaning_only": state(MEANING_ONLY_QUERY, None, None, []), "from": state("from:" + FROM_WORD, None, None, []), "to": state("to:" + TO_WORD, None, None, []),
                    "people": {"ready": True, "fay": [{"name": "Fay Lund", "address": FAY, "from_count": 0, "to_count": 1}], "dana": [{"name": "Dana Reyes", "address": DANA, "from_count": 1, "to_count": 0}], "owner": []}}
        stage, tail = ("ranked", ranked) if model else ("no_model", by_date)
        if mutant == "whole-words-only" and model:      # consultant does not find consultants as a word: the mail is found by meaning, first
            tail = [hit(PAIR_CONSULTANTS, "meaning", 0.9)] + ranked[1:]
        if mutant == "advisor-last" and model:
            tail = [h for h in ranked if h["id"] != PAIR_ADVISOR] + [hit(PAIR_ADVISOR, "meaning", 0.001)]
        if mutant == "stuck-on-words":
            stage = "words"
        if mutant == "list-cut" and model:
            tail = ranked[:3]
        if mutant == "exact-after-meaning" and model:
            tail = [ranked[1], dict(ranked[0]), *ranked[2:]]
        main = state(SEARCH_WORD, SEARCH_WORD, stage, tail, settled=model and stage == "ranked" or not model, waited=12.0, **({"reason": "no model"} if not model else {}))
        mo = state(MEANING_ONLY_QUERY, MEANING_ONLY_QUERY, stage, ([hit(PAIR_ADVISOR, "meaning", 0.7), hit(PAIR_CONSULTANTS, "meaning", 0.69)] + [hit(i, "meaning", 0.1) for i in ("m-eli", "m-dana", "m-fay", "m-gus")]) if model else by_date)
        if mutant == "meaning-only-ranks-decoy" and model:
            mo = state(MEANING_ONLY_QUERY, MEANING_ONLY_QUERY, stage, [hit("m-eli", "meaning", 0.7)] + [hit(i, "meaning", 0.1) for i in (PAIR_ADVISOR, PAIR_CONSULTANTS, "m-dana", "m-fay", "m-gus")])
        every = [hit(i, "meaning", 0.0) for i in scr]
        frm = state("from:" + FROM_WORD, 'from:"%s"' % FROM_WORD, "filtered", [hit("m-dana", "subject", 0.0)])
        to = state("to:" + TO_WORD, 'to:"%s"' % TO_WORD, "filtered", [hit("m-fay", "subject", 0.0)])
        if mutant == "from-ignored":
            frm = state("from:" + FROM_WORD, 'from:"%s"' % FROM_WORD, "ranked", every)
        if mutant == "to-ignored":
            to = state("to:" + TO_WORD, 'to:"%s"' % TO_WORD, "filtered", [hit("m-dana", "subject", 0.0), hit("m-fay", "subject", 0.0)])
        people = {"ready": True, "fay": [{"name": "Fay Lund", "address": FAY, "from_count": 0, "to_count": 1}], "dana": [{"name": "Dana Reyes", "address": DANA, "from_count": 1, "to_count": 0}], "owner": []}
        if mutant == "people-no-recipients":
            people = dict(people, fay=[])
        if mutant == "people-not-ready":
            people = dict(people, ready=False)
        if mutant == "owner-listed":
            people = dict(people, owner=[{"name": "Owner", "address": "owner@whoosh-check.invalid", "from_count": 0, "to_count": 0}])
        if mutant == "restart-loses-index" and not first:
            main = state(SEARCH_WORD, SEARCH_WORD, "ranked", [hit(PAIR_ADVISOR, "meaning", 0.5)], settled=False, waited=240.0)
        return {"main": main, "meaning_only": mo, "from": frm, "to": to, "people": people}
    ids = ["r-1", "r-2", "r-3"]
    picks = [0, 1, 2, 1]
    def reader_state(parts):
        steps = []
        for key, i in zip(READER_STEPS, picks):
            shown = [ids[i]]
            if mutant == "reader-keeps-first":
                shown = [ids[0]]
            if mutant == "reader-off-results" and key == "Down":
                shown = ["not-a-result"]
            steps.append({"key": key or "none", "listed": ids, "parts": parts, "selected": [ids[i]], "reader": shown})
        return {"asked": READER_QUERY, "stage": "ranked", "count": 3, "steps": steps}
    if mutant == "no-search-arm":
        reader = {"asked": READER_QUERY, "steps": [{"key": "none", "listed": [], "parts": [], "selected": [], "reader": []}]}
    else:
        reader = reader_state(["match", "match", "match"])
    words = LATE_MODEL_MISSING_WORDS
    if model:
        late = {"stages": ["words", "ranked"], "reasons": [None, None], "seconds": 3.0, "typed_s": 0.7}
        if mutant == "late-ends-no-model":
            late = {"stages": ["words", "no_model"], "reasons": [None, words], "seconds": 3.0}
        if mutant == "late-sentence-between":
            late = {"stages": ["words", "ranked"], "reasons": [words, None], "seconds": 3.0}
        if mutant == "late-stuck-on-words":
            late = {"stages": ["words"], "reasons": [None], "seconds": 60.0, "typed_s": 0.7}
        if mutant == "late-window-missed":      # the delay was asked and the search typed in time, yet the engine ranked at once: the hand-over was not late
            late = {"stages": ["ranked"], "reasons": [None], "seconds": 1.0, "typed_s": 0.7}
        if mutant == "late-typed-too-late":     # the search was typed after the window had closed: NOT JUDGED, never a pass or a fail
            late = {"stages": ["ranked"], "reasons": [None], "seconds": 1.0, "typed_s": 4.5}
    else:
        late = {"stages": ["no_model"], "reasons": [words], "seconds": 0.5}
        if mutant == "nomodel-no-sentence":
            late = {"stages": ["no_model"], "reasons": [None], "seconds": 0.5}
        if mutant == "nomodel-other-sentence":
            late = {"stages": ["no_model"], "reasons": ["No model."], "seconds": 0.5}
        if mutant == "nomodel-stuck-on-words":
            late = {"stages": ["words"], "reasons": [None], "seconds": 60.0}
    ans = {"add": {"stage": "added"}, "core": "rust", "before": round_(True), "reader": reader, "late": late, "weights_given": model, "handover_delay_ms": LATE_HANDOVER_DELAY_MS if model else 0,
           "refusal": {"asked": "???", "engine_query_before": 'to:"fay"', "engine_query_after": 'to:"fay"', "refused": [], "list_rows": 6},
           "restart": {"ok": True, "error": None}, "after": round_(False)}
    ans["after"].pop("meaning_only", None)
    if mutant == "restart-no-copy":
        ans["after"] = None; ans["restart"] = {"ok": False, "error": "no socket after the relaunch"}; ans.pop("late")
    if mutant == "restart-other-order" and model:
        ans["after"]["main"]["hits"] = list(reversed(ans["after"]["main"]["hits"][1:])); ans["after"]["main"]["hits"].insert(0, ranked[0])
    if mutant == "refusal-as-list":
        ans["refusal"] = {"asked": "???", "engine_query_before": 'to:"fay"', "engine_query_after": "???", "refused": [], "list_rows": 6}
    if mutant == "refusal-in-words":
        ans["refusal"] = {"asked": "???", "engine_query_before": 'to:"fay"', "engine_query_after": "", "refused": [{"text": "The search could not run.", "words": "type something to search for"}], "list_rows": 0}
    if mutant == "refusal-empty-words":
        ans["refusal"] = {"asked": "???", "engine_query_before": 'to:"fay"', "engine_query_after": "", "refused": [{"text": "The search could not run.", "words": ""}], "list_rows": 0}
    return ans


SEARCH_SELFTEST_MUTANTS = {   # mutant -> the cases that must go red (each case is red on at least one mutant of its own)
    "no-search-arm": SEARCH_ALL_CASES[:6],
    "whole-words-only": (SEARCH_ALL_CASES[0], SEARCH_ALL_CASES[1]),
    "advisor-last": (SEARCH_ALL_CASES[1],),
    "meaning-only-ranks-decoy": (SEARCH_ALL_CASES[2],),
    "stuck-on-words": SEARCH_ALL_CASES[1:4],
    "list-cut": (SEARCH_ALL_CASES[3],),
    "exact-after-meaning": (SEARCH_ALL_CASES[0], SEARCH_ALL_CASES[3]),
    "from-ignored": (SEARCH_ALL_CASES[4],),
    "to-ignored": (SEARCH_ALL_CASES[5],),
    "people-no-recipients": (SEARCH_ALL_CASES[6],),
    "people-not-ready": (SEARCH_ALL_CASES[6],),
    "owner-listed": (SEARCH_ALL_CASES[6],),
    "restart-loses-index": (SEARCH_ALL_CASES[7],),
    "restart-no-copy": (SEARCH_ALL_CASES[7],),
    "restart-other-order": (SEARCH_ALL_CASES[7],),
    "refusal-as-list": (SEARCH_ALL_CASES[8],),
    "refusal-empty-words": (SEARCH_ALL_CASES[8],),
    "reader-keeps-first": (SEARCH_ALL_CASES[READER_CASE_I],),
    "reader-off-results": (SEARCH_ALL_CASES[READER_CASE_I],),
    "late-ends-no-model": (SEARCH_ALL_CASES[LATE_CASE_I],),
    "late-sentence-between": (SEARCH_ALL_CASES[LATE_CASE_I],),
    "late-stuck-on-words": (SEARCH_ALL_CASES[LATE_CASE_I],),
    "late-window-missed": (SEARCH_ALL_CASES[LATE_CASE_I],),
}
SEARCH_NOMODEL_MUTANTS = {   # judged on the copy with no model: the case of the missing-model sentence
    "nomodel-no-sentence": (SEARCH_ALL_CASES[NOMODEL_CASE_I],),
    "nomodel-other-sentence": (SEARCH_ALL_CASES[NOMODEL_CASE_I],),
    "nomodel-stuck-on-words": (SEARCH_ALL_CASES[NOMODEL_CASE_I],),
}


def selftest_search():
    """The judge against a model of a correct engine (every case passes), the same model with no model in the copy (the two meaning cases NOT JUDGED, never passed,
    the refusal case NOT JUDGED because the screen cannot reach it), the refusal shown in words (passes), and one broken engine per case (each case red, and only
    the cases the defect touches). The fixture mailbox is read back with Python's own IMAP client. No app is launched."""
    import imaplib
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import realcore_mailfake as fake
    names = [n for n, _, _ in judge_search(search_answer())]
    verdict = lambda ans: {n: ok for n, ok, _ in judge_search(ans)}
    good = verdict(search_answer())
    not_asked = (SEARCH_ALL_CASES[8], SEARCH_ALL_CASES[NOMODEL_CASE_I])      # the refusal (the screen cannot reach it) and the no-model sentence (the copy has a model)
    ok_good = names == list(SEARCH_ALL_CASES) and all(v is True for k, v in good.items() if k not in not_asked) and all(good[k] is None for k in not_asked)
    spoken = verdict(search_answer("refusal-in-words"))
    ok_refusal = spoken[SEARCH_ALL_CASES[8]] is True
    # fix-realcore round 2: the no-model case is read from a second copy started without a model folder; every other case stays the first copy's
    first_r, second_r = judge_search(search_answer()), judge_search(search_answer(model=False))
    merged = {n: ok for n, ok, _ in merge_nomodel(first_r, second_r)}
    ok_refusal = ok_refusal and merged[SEARCH_ALL_CASES[NOMODEL_CASE_I]] is True and all(merged[n] == ok for n, ok, _ in first_r if n != SEARCH_ALL_CASES[NOMODEL_CASE_I]) \
        and {n: ok for n, ok, _ in merge_nomodel(first_r, None)}[SEARCH_ALL_CASES[NOMODEL_CASE_I]] is None \
        and {n: ok for n, ok, _ in merge_nomodel(first_r, judge_search(search_answer("nomodel-no-sentence", model=False)))}[SEARCH_ALL_CASES[NOMODEL_CASE_I]] is False
    # fix-realcore: the refusal asked through the socket door: words pass; accepted, blank words or a changed last query are red; a build without the door NOT JUDGED
    def by_socket(error, after='to:"fay"'):
        a = search_answer()
        a["refusal"] = dict(a["refusal"], socket={"error": error}, engine_query_after_socket=after)
        return verdict(a)[SEARCH_ALL_CASES[8]]
    ok_refusal = ok_refusal and by_socket("core ['run', 'search']: type something to search for") is True and by_socket(None) is False \
        and by_socket("core ['run', 'search']: ") is False and by_socket("core ['run', 'search']: type something to search for", after="") is False \
        and by_socket("core ['run', 'search']: usage: core state <path>") is None
    nomodel = verdict(search_answer(model=False))
    none_no_model = SEARCH_ALL_CASES[1:3] + (SEARCH_ALL_CASES[8], SEARCH_ALL_CASES[LATE_CASE_I])
    ok_nomodel = (all(nomodel[k] is None for k in none_no_model) and all(v is True for k, v in nomodel.items() if k not in none_no_model))
    bad, survived = [], []
    for mutant, targets in SEARCH_SELFTEST_MUTANTS.items():
        got = verdict(search_answer(mutant))
        if not all(got[t] is False for t in targets):
            survived.append(mutant)
        touched = sorted(k for k, v in got.items() if v is False and k not in targets and k != SEARCH_ALL_CASES[7])
        if touched:
            bad.append("%s also reddens %s" % (mutant, touched))
    for mutant, targets in SEARCH_NOMODEL_MUTANTS.items():
        got = verdict(search_answer(mutant, model=False))
        if not all(got[t] is False for t in targets):
            survived.append(mutant)
        touched = sorted(k for k, v in got.items() if v is False and k not in targets)
        if touched:
            bad.append("%s also reddens %s" % (mutant, touched))
    ok_too_late = verdict(search_answer("late-typed-too-late"))[SEARCH_ALL_CASES[LATE_CASE_I]] is None      # a search typed after the delayed window closed is NOT JUDGED, never a pass or a fail
    covered = {t for ts in list(SEARCH_SELFTEST_MUTANTS.values()) + list(SEARCH_NOMODEL_MUTANTS.values()) for t in ts}
    ok_cover = covered == set(SEARCH_ALL_CASES)
    mail, imap, smtp = fake.start(0, 0, inbox=0)
    try:
        load_search_mail(fake, mail)
        c = imaplib.IMAP4("127.0.0.1", imap.server_address[1])
        c.login(fake.ADDRESS, fake.PASSWORD)
        c.select("INBOX")
        typ, data = c.uid("FETCH", "1:3", "(UID BODY.PEEK[HEADER])")
        heads = b"".join(x[1] for x in data if isinstance(x, tuple))
        inbox_ok = all(s[1].encode() in heads for s in SEARCH_INBOX) and b"consultant" not in heads.lower()
        c.select("Sent")
        typ, data = c.uid("FETCH", "4", "(UID BODY.PEEK[HEADER])")
        sent = b"".join(x[1] for x in data if isinstance(x, tuple))
        sent_ok = b"To: Fay Lund <fay.lund@example.test>" in sent and b"From: Owner <" + fake.ADDRESS.encode() in sent
        c.logout()
    finally:
        imap.shutdown(); smtp.shutdown()
    print("selftest slice search: model %s, no-model copy %s, refusal in words %s, %d mutant models %s, mailbox %s%s" % (
        "all pass (NOT JUDGED: the refusal case, which the screen cannot reach, and the no-model sentence case, which a copy with a model is not asked)" if ok_good else "FAILS", "four cases NOT JUDGED (the two meaning cases, the late-model case, the refusal), the rest pass" if ok_nomodel else "WRONG",
        "passes" if ok_refusal else "WRONG", len(SEARCH_SELFTEST_MUTANTS) + len(SEARCH_NOMODEL_MUTANTS), "each red on its own case" if not survived and ok_cover else "SURVIVE %s (cases covered %d of %d)" % (survived, len(covered), len(SEARCH_ALL_CASES)),
        "served (decoys and the sent mail to Fay)" if inbox_ok and sent_ok else "WRONG", "" if not bad else "; " + "; ".join(bad)))
    print("selftest slice search: hand-over delay: the late case is judged because the copy is started with WHOOSH_RIG_EMBED_HANDOVER_DELAY_MS=%d (a missed window is red, a search typed over %ss late is NOT JUDGED) %s" % (
        LATE_HANDOVER_DELAY_MS, LATE_TYPED_IN_TIME_S, "yes" if ok_too_late else "WRONG"))
    return ok_good and ok_refusal and ok_nomodel and not survived and ok_cover and inbox_ok and sent_ok and not bad and ok_too_late



# ---------------------------------------------------------------- slice label-click (gap-engine-state, item f31ec0ea)
# "A sidebar label click drives the list and the reader" on the REAL core. The fixture half is par-inboxlist J1.55; this is its real-core cell: the rig copy seeds the
# labels of slice 3 (WHOOSH_CORE_CHECK_FLOW=1: Contracts, Answered, Renewals, all pinned), the local server holds the slice-3 mail, the sync files it, then the check CLICKS
# the sidebar's label rows (`mac-main.sidebar.label.<id>`, a door the app registers) and reads what the list and the reader show. The expected rows come from the seeded
# mail by subject (Answered holds the Budget message, Contracts the contract mail, Renewals nothing), never from the core's own filing record, so a core that files
# nothing is red (the --mutant run).
LABEL_CLICK_CASES = ("label-click-pinned-opens-its-tab", "label-click-unpinned-lists-only-its-messages", "label-click-reader-shows-the-first-row",
                     "label-click-empty-label-says-so-and-clears-the-reader", "label-click-all-inboxes-ends-it")
LABEL_CLICK_MUTANT_RED = ("label-click-unpinned-lists-only-its-messages", "label-click-reader-shows-the-first-row")


def drive_labelclick():
    """Slice 3's drive (the account added, the mail filed), then the clicks: Contracts (pinned: its tab), Answered unpinned through its own pin door (its messages
    only), Renewals unpinned (no message), then All inboxes. Each step reads the list's ids, the reader's shown id and `mac-main.list.state`."""
    def after(c, answer):
        def labels():
            return {r.get("raw_name"): r for r in c.rows("mac-main.sidebar.labels")}

        def look():
            time.sleep(0.6)     # the click's own motion (focus slide, reader follow) ends before the read
            ids = [r.get("id") for r in c.rows("mac-main.list")]
            sh = c.rows("mac-main.reader.shown")
            st = (c.rows("mac-main.list.state") or [{}])[0]
            return {"tab": c.uistate().get("tab"), "ids": ids, "shown": sh[0].get("shown") if sh else None, "label_filter": st.get("label_filter"),
                    "empty_line": ((st.get("empty_words") or {}).get("line"))}
        got = labels()
        answer["label_rows"] = {n: {k: r.get(k) for k in ("id", "pinned", "kind")} for n, r in got.items()}
        ids = {n: (got.get(n) or {}).get("id") for n in ("Contracts", "Answered", "Renewals")}
        answer["label_ids"] = ids
        # evidence that decides the two reds of the first hidden run: the strip the app draws (`mac-main.tabs`, from the core's counts), the tab list the engine serves
        # (`inbox/tabs`, from its pinned labels) and the failure line the app shows after a refused command (`mac-main.list.state` last_failure)
        answer["strip_tab_ids"] = [r.get("id") for r in c.rows("mac-main.tabs")]
        et = cv(c, "inbox/tabs")
        answer["engine_tab_ids"] = [t.get("id") for t in et] if isinstance(et, list) else None
        steps = {}
        try:
            c.click("mac-main.sidebar.label.%s" % ids["Contracts"]); steps["pinned"] = look()
            for n in ("Answered", "Renewals"):          # unpin through the sidebar's own pin door, then read the row back
                c.click("mac-main.sidebar.label.%s.pin" % ids[n]); time.sleep(0.4)
            answer["unpinned_now"] = {n: (labels().get(n) or {}).get("pinned") for n in ("Answered", "Renewals")}
            c.click("mac-main.sidebar.mailbox.all-inboxes"); time.sleep(0.4)
            c.click("mac-main.sidebar.label.%s" % ids["Answered"]); steps["unpinned"] = look()
            c.click("mac-main.sidebar.label.%s" % ids["Renewals"]); steps["empty"] = look()
            c.click("mac-main.sidebar.mailbox.all-inboxes"); steps["all"] = look()
        except Exception as e:
            answer["click_error"] = str(e)
            answer["failure_line_after_error"] = ((c.rows("mac-main.list.state") or [{}])[0]).get("last_failure")
        answer["steps"] = steps
    return flow_drive(FLOW_SUBJECTS, (FLOW_SUBJECTS[0], FLOW_SUBJECTS[2]), after=after)


def judge_labelclick(answer):
    """The cases of J1.55 on the real core: (name, True / False / None, detail). Not read = NOT JUDGED, never a pass."""
    rows, steps, lid = answer.get("rows") or {}, answer.get("steps") or {}, answer.get("label_ids") or {}
    msg = lambda i: (rows.get(FLOW_SUBJECTS[i]) or {}).get("id")
    contract, budget = msg(0), msg(2)

    def case(name, step, want_fn, detail_fn):
        s = steps.get(step)
        if s is None or any(v is None for v in (contract, budget)):
            return (name, None, "not read: %s (click error %r; the failure line the app showed %r)" % (step, answer.get("click_error"), answer.get("failure_line_after_error")))
        return (name, bool(want_fn(s)), detail_fn(s))
    pinned = case("label-click-pinned-opens-its-tab", "pinned", lambda s: s["tab"] == lid.get("Contracts") and s["label_filter"] in (None, "") and s["shown"] == (s["ids"] or [None])[0] and s["shown"] is not None,
                  lambda s: "tab %r (want %r), label_filter %r, reader shows %r, first row %r; the strip holds %r, the engine's inbox/tabs holds %r" % (s["tab"], lid.get("Contracts"), s["label_filter"], s["shown"], (s["ids"] or [None])[0], answer.get("strip_tab_ids"), answer.get("engine_tab_ids")))
    unpinned = case("label-click-unpinned-lists-only-its-messages", "unpinned", lambda s: s["label_filter"] == lid.get("Answered") and s["ids"] == [budget] and answer.get("unpinned_now", {}).get("Answered") is False,
                    lambda s: "label_filter %r (want %r), list %r (want [%r]), Answered pinned after its pin door: %r" % (s["label_filter"], lid.get("Answered"), s["ids"], budget, (answer.get("unpinned_now") or {}).get("Answered")))
    reader = case("label-click-reader-shows-the-first-row", "unpinned", lambda s: s["shown"] == budget and s["shown"] is not None, lambda s: "reader shows %r, the label's first row is %r" % (s["shown"], budget))
    empty = case("label-click-empty-label-says-so-and-clears-the-reader", "empty",
                 lambda s: s["ids"] == [] and s["shown"] is None and "Renewals" in (s["empty_line"] or "") and s["label_filter"] == lid.get("Renewals"),
                 lambda s: "list %r (want []), reader %r (want none), empty words %r, label_filter %r" % (s["ids"], s["shown"], s["empty_line"], s["label_filter"]))
    end = case("label-click-all-inboxes-ends-it", "all", lambda s: s["label_filter"] in (None, "") and len(s["ids"]) == len(FLOW_SUBJECTS),
               lambda s: "label_filter %r, %d row(s) (want %d)" % (s["label_filter"], len(s["ids"]), len(FLOW_SUBJECTS)))
    return [("account-added", answer.get("stage") == "added", "stage %r, error %r" % (answer.get("stage"), answer.get("error"))),
            ("core-is-rust", answer.get("core") in (None, "rust"), "uistate core %r" % answer.get("core")),
            ("labels-are-seeded-and-pinned", all(lid.get(n) for n in ("Contracts", "Answered", "Renewals")) and all((answer.get("label_rows") or {}).get(n, {}).get("pinned") is True for n in ("Contracts", "Answered", "Renewals")),
             "label rows %r" % (answer.get("label_rows"),)),
            pinned, unpinned, reader, empty, end]


def main_slice_labelclick(bundle, mutant):
    """Slice 3's launch (the local servers in THIS process, the flow seed on), the clicks inside. --mutant: WHOOSH_CORE_MUTANT=flow-never-files, so Answered holds nothing:
    the two cases that read a label's own messages must go red (exit 0 when they did)."""
    import json
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import realcore_mailfake as fake
    try:
        mail, imap, smtp = fake.start(0, 0, inbox=0)
    except OSError as e:
        print("the local servers did not start: %s" % e)
        print("1 checks, 0 failed, 1 not judged"); return 1
    load_flow_mail(fake, mail)
    ports = "%d:%d" % (imap.server_address[1], smtp.server_address[1])
    extra = ["WHOOSH_CORE_PLAIN_LOOPBACK=1", "WHOOSH_CORE_CHECK_SERVERS=" + ports, "WHOOSH_CORE_CHECK_FLOW=1"] + (["WHOOSH_CORE_MUTANT=flow-never-files"] if mutant else [])
    env = dict(os.environ, WHOOSH_RIG_HIDDEN="1", LAUNCH_ENV=" ".join(["WHOOSH_RIG_HIDDEN=1"] + extra))
    env.pop("WHOOSH_FIXTURE_CORE", None)
    cmd = ["sh", os.path.join(ROOT, "scripts", "launch-one.sh"), bundle, "--", "--rig-hidden",
           "sh", "-c", 'WHOOSH_SOCKET_DIR="$D" python3 "$0" --inner --slice-labelclick', os.path.abspath(__file__)]
    r = launch(cmd, env)
    imap.shutdown(); smtp.shutdown()
    out = r.stdout + r.stderr
    line = next((l for l in out.splitlines() if l.startswith("ANSWER ")), None)
    if line is None:
        print(out.rstrip()[-3000:])
        print("launcher exit %d" % r.returncode)
        print("1 checks, 0 failed, 1 not judged"); return 1
    answer = json.loads(line[len("ANSWER "):])
    print("launcher exit %d; the local server saw: %s; %s" % (r.returncode, sorted({l.split(" ")[0] for l in mail.log}), next((l for l in out.splitlines() if l.startswith("instances:")), "")))
    results = judge_labelclick(answer)
    lines = ["%-10s labelclick/%s  %s" % ("PASS" if ok else ("NOT JUDGED" if ok is None else "FAIL"), n, d) for n, ok, d in results]
    failed = sum(1 for _, ok, _ in results if ok is False)
    nj = sum(1 for _, ok, _ in results if ok is None)
    if mutant:
        red = all(ok is False for n, ok, _ in results if n in LABEL_CLICK_MUTANT_RED)
        lines.append("mutant flow-never-files: %s" % ("RED as it must be (Answered holds no message: its list is empty and the reader shows none)" if red else "SURVIVED: a label's own messages were listed by a core that files nothing"))
        lines.append("%d checks, %d failed, %d not judged (mutant run)" % (len(results), failed, nj))
        print("\n".join(lines)); return 0 if red else 1
    lines.append("%d checks, %d failed, %d not judged" % (len(results), failed, nj))
    print("\n".join(lines)); return 0 if failed == 0 and nj == 0 else 1


def selftest_labelclick():
    """The judge against a model answer (every case passes) and one broken model per case (each case red, and only the cases the defect touches); no app."""
    ids = {"Contracts": "l-1", "Answered": "l-2", "Renewals": "l-3"}
    def good():
        return {"stage": "added", "core": "rust", "label_ids": dict(ids), "unpinned_now": {"Answered": False, "Renewals": False},
                "label_rows": {n: {"id": i, "pinned": True, "kind": "owner"} for n, i in ids.items()},
                "rows": {FLOW_SUBJECTS[0]: {"id": "m1"}, FLOW_SUBJECTS[1]: {"id": "m2"}, FLOW_SUBJECTS[2]: {"id": "m3"}},
                "steps": {"pinned": {"tab": "l-1", "ids": ["m1"], "shown": "m1", "label_filter": None, "empty_line": None},
                          "unpinned": {"tab": "primary", "ids": ["m3"], "shown": "m3", "label_filter": "l-2", "empty_line": None},
                          "empty": {"tab": "primary", "ids": [], "shown": None, "label_filter": "l-3", "empty_line": "Nothing in Renewals yet."},
                          "all": {"tab": "primary", "ids": ["m1", "m2", "m3"], "shown": "m1", "label_filter": None, "empty_line": None}}}
    def broken(path, value):
        a = good(); d = a
        for k in path[:-1]:
            d = d[k]
        d[path[-1]] = value
        return a
    verdict = lambda a: {n: ok for n, ok, _ in judge_labelclick(a)}
    ok_good = all(v is True for v in verdict(good()).values()) and [n for n, _, _ in judge_labelclick(good())][3:] == list(LABEL_CLICK_CASES)
    models = {   # defect -> the cases that must go red
        "pinned label did not open its tab": (broken(["steps", "pinned", "tab"], "primary"), ("label-click-pinned-opens-its-tab",)),
        "unpinned label listed every message": (broken(["steps", "unpinned", "ids"], ["m1", "m2", "m3"]), ("label-click-unpinned-lists-only-its-messages",)),
        "unpinned label was not focused as a filter": (broken(["steps", "unpinned", "label_filter"], None), ("label-click-unpinned-lists-only-its-messages",)),
        "the reader did not follow the click": (broken(["steps", "unpinned", "shown"], "m1"), ("label-click-reader-shows-the-first-row",)),
        "an empty label kept the old reader": (broken(["steps", "empty", "shown"], "m3"), ("label-click-empty-label-says-so-and-clears-the-reader",)),
        "an empty label said nothing": (broken(["steps", "empty", "empty_line"], None), ("label-click-empty-label-says-so-and-clears-the-reader",)),
        "all inboxes kept the filter": (broken(["steps", "all", "label_filter"], "l-2"), ("label-click-all-inboxes-ends-it",)),
        "a core that files nothing (Answered empty)": (broken(["steps", "unpinned"], {"tab": "primary", "ids": [], "shown": None, "label_filter": "l-2", "empty_line": "x"}),
                                                      LABEL_CLICK_MUTANT_RED),
    }
    survived, noisy = [], []
    for name, (answer, targets) in models.items():
        got = verdict(answer)
        if not all(got[t] is False for t in targets):
            survived.append(name)
        extra = sorted(k for k, v in got.items() if v is False and k not in targets)
        if extra:
            noisy.append("%s also reddens %s" % (name, extra))
    unread = verdict(broken(["steps"], {}))
    ok_unread = all(unread[n] is None for n in LABEL_CLICK_CASES)    # nothing clicked: NOT JUDGED, never a pass
    covered = {t for _, ts in models.values() for t in ts}
    print("selftest slice label-click: model %s, %d broken models %s, nothing clicked %s%s" % (
        "all %d cases pass" % len(LABEL_CLICK_CASES) if ok_good else "FAILS", len(models), "each red on its own cases" if not survived and covered == set(LABEL_CLICK_CASES) else "SURVIVE %s (covered %d of %d)" % (survived, len(covered), len(LABEL_CLICK_CASES)),
        "NOT JUDGED" if ok_unread else "WRONG", "" if not noisy else "; " + "; ".join(noisy)))
    return ok_good and not survived and covered == set(LABEL_CLICK_CASES) and ok_unread and not noisy


# ---------------------------------------------------------------- slice accountmark (gap-engine-state, item 33270f65)
# "The account mark on every surface" on the REAL core: the real-core cell of checks/seams/account-mark.py (which reads the fixture core). One account is added through the sheet
# (slice 2's local server, three inbox messages); its mark is read on the sidebar ring (`mac-main.sidebar.accounts`), the list row (`account_icon`, `account_colour`), the reader
# meta (`mac-main.reader.header`) and the search result (`mac-search.results`). The colour must be the store's first palette colour (core/crates/store/src/realcore.rs COLOURS[0]: the first
# account), and every surface must show the same icon and colour as the sidebar ring. The notch shows an ARRIVAL, which this slice has no way to deliver: NOT JUDGED, said so.
ACCOUNT_MARK_COLOUR = "#3B82F6"     # core/crates/store/src/realcore.rs COLOURS[0]: what the real core gives the first account
ACCOUNT_MARK_SURFACES = ("sidebar-ring", "list-row", "reader-meta", "search-result")
# fix-realcore: a NEW account has no icon on either engine (the test engine's added accounts too, FixtureCore.swift `iconId(a.id, fallback: nil)`; engine_account_mark.rs
# forbids an invented one), so the slice picks one the way the owner does, through Settings > Accounts (the doors journeys/J4.py J4.12 uses), and the real core serves
# SetAccountIcon (core/crates/engine/src/account_cmds.rs). A library glyph: Sources/Design/Emblem/Library/EmblemWork.swift.
ACCOUNT_MARK_ICON = "briefcase"
ACCOUNT_MARK_SETTINGS, ACCOUNT_MARK_ACCOUNTS = "mac-settings-window", "mac-settings-accounts"
# fix-realcore: the notch shows an ARRIVAL, so the slice delivers one: the driver (inside launch-one) writes the file named by WHOOSH_CHECK_ARRIVE, this process's watcher
# puts one new mail in the fake's INBOX (realcore_mailfake.deliver), the IDLE connection announces it, and the driver reads `mac-notch.list` for it.
ACCOUNT_MARK_ARRIVAL_SUBJECT = "Account mark arrival"
ACCOUNT_MARK_NOTCH_WAIT_S = 20     # a check bound: the IDLE poll, the sync's apply and the notch's draw


def accountmark_arrival_raw(address):
    return ("From: Mark Sender <mark.sender@example.test>\r\nTo: %s\r\nSubject: %s\r\nMessage-ID: <accountmark-arrival@whoosh-check.invalid>\r\n"
            "Date: Mon, 09 Nov 2026 12:00:00 +0000\r\n\r\nA new mail for the notch.\r\n" % (address, ACCOUNT_MARK_ARRIVAL_SUBJECT)).encode()


def accountmark_pick_icon(c, account_id):
    """Settings > Accounts > the account's page > its icon > the library glyph > Done, then the main window again; returns what the core then holds."""
    for door in (ACCOUNT_MARK_SETTINGS + ".open", ACCOUNT_MARK_SETTINGS + ".pane-row.accounts", "%s.%s" % (ACCOUNT_MARK_ACCOUNTS, account_id), "%s.%s.icon" % (ACCOUNT_MARK_ACCOUNTS, account_id),
                 ACCOUNT_MARK_ACCOUNTS + ".icon-library." + ACCOUNT_MARK_ICON, ACCOUNT_MARK_ACCOUNTS + ".picker.done", "dock.whoosh"):
        c.click(door)
        time.sleep(0.3)
    held, _ = mv_poll(lambda: cv(c, "account_icon/%s" % account_id), lambda v: v == ACCOUNT_MARK_ICON, 5)
    return held


def drive_accountmark():
    """Slice 3's drive on slice 3's mail without its labels (no flow seed needed to read marks), then read each surface for the first row."""
    def after(c, answer):
        marks = {}
        side = {r.get("id"): r for r in c.rows("mac-main.sidebar.accounts")}
        account_id = next(iter(side), None)
        pick = answer["icon_pick"] = {"account_read": account_id is not None, "core_icon": None, "error": None}
        if account_id is not None:
            try:
                pick["core_icon"] = accountmark_pick_icon(c, account_id)
            except Exception as e:      # noqa: BLE001
                pick["error"] = str(e)[:200]
            time.sleep(0.5)      # the surfaces redraw on the account mark's revision
        side = {r.get("id"): r for r in c.rows("mac-main.sidebar.accounts")}
        rows = c.rows("mac-main.list")
        row = rows[0] if rows else {}
        acc = next(iter(side.values()), {})
        # "served": the row HAS the icon key (a null is the core's word "no icon chosen"; an absent key is a surface that serves nothing)
        marks["sidebar-ring"] = {"icon": acc.get("icon"), "colour": acc.get("colour"), "served": "icon" in acc}
        marks["list-row"] = {"icon": row.get("account_icon"), "colour": row.get("account_colour"), "served": "account_icon" in row}
        answer["first_row"] = {"id": row.get("id"), "sender": row.get("sender"), "sidebar_accounts": len(side)}
        try:
            c.key("Enter"); time.sleep(0.5)
            h = c.rows("mac-main.reader.header"); h = h[0] if h else {}
            marks["reader-meta"] = {"icon": h.get("account_icon"), "colour": h.get("account_colour"), "served": "account_icon" in h}
        except Exception as e:      # noqa: BLE001
            answer["read_error"] = "reader: %s" % e
        try:
            # the open door, not the "/" key: which key opens the search depends on the preset (run_search says why); then the field, the words, Return
            c.key("Escape")
            if c.uistate().get("surface") != "mac-search": c.click("mac-search.open")
            c.click(SEARCH_BAR); c.key("\u2318A"); c.type(row.get("sender") or ""); c.key("Enter")
            # the results land after the index answers: wait for the row up to 15 s, and keep what the search showed, so a miss says why
            rows_now, secs = mv_poll(lambda: c.rows("mac-search.results"), lambda rs: any(r.get("id") == row.get("id") for r in rs), 15)
            found = [r for r in rows_now or [] if r.get("id") == row.get("id")]
            answer["search_note"] = "searched %r: %d result rows after %ss, ids %r, wanted %r" % (row.get("sender"), len(rows_now or []), secs, [r.get("id") for r in (rows_now or [])][:5], row.get("id"))
            marks["search-result"] = {"icon": found[0].get("account_icon"), "colour": found[0].get("colour"), "served": "account_icon" in found[0]} if found else None
        except Exception as e:      # noqa: BLE001
            answer["read_error"] = "search: %s" % e
        trigger = os.environ.get("WHOOSH_CHECK_ARRIVE")
        if trigger:
            try:
                c.key("Escape")
                with open(trigger, "w") as f:
                    f.write("arrive\n")
                hit = lambda rs: next((r for r in rs or [] if r.get("subject") == ACCOUNT_MARK_ARRIVAL_SUBJECT), None)
                # `mac-notch.list` answers nothing while the notch is at rest (NotchRows.swift: `stage != .rest`), and the arrival's drop falls back to rest after its hold: a poll
                # on the list alone can read 0 rows for a notch that holds the mail. So the poll also reads the notch's own row (`items`, `stage`); once it holds the arrival and
                # rests, the list is opened the way a hover does (`fixture pointer-rest mac-notch`, NotchEvents) and read.
                def notch_row():
                    r = c.rows("mac-notch")
                    return r[0] if r else {}
                rows_n, secs = mv_poll(lambda: c.rows("mac-notch.list"), lambda rs: hit(rs) is not None or int(notch_row().get("items") or 0) > 0, ACCOUNT_MARK_NOTCH_WAIT_S)
                facts = notch_row()
                if hit(rows_n) is None and int(facts.get("items") or 0) > 0 and facts.get("stage") == "rest":
                    c.send("fixture", "pointer-rest", "mac-notch")
                    rows_n, _ = mv_poll(lambda: c.rows("mac-notch.list"), lambda rs: hit(rs) is not None, 5)
                n = hit(rows_n)
                try:
                    in_list = any(r.get("subject") == ACCOUNT_MARK_ARRIVAL_SUBJECT for r in c.rows("mac-main.list"))
                except Exception:      # noqa: BLE001
                    in_list = "unread"
                answer["notch_note"] = "delivered %s; the notch listed %d row(s) after %ss, the arrival among them: %s; the notch holds %s item(s) at stage %r (unread %s); the main list holds the arrival: %s" % (
                    os.path.exists(trigger + ".done"), len(rows_n or []), secs, n is not None, facts.get("items"), facts.get("stage"), facts.get("unread"), in_list)
                marks["notch"] = {"icon": n.get("account_icon"), "colour": n.get("account_colour"), "served": "account_icon" in n} if n else None
            except Exception as e:      # noqa: BLE001
                answer["notch_note"] = "notch: %s" % e
        answer["marks"] = marks
    return flow_drive(FLOW_SUBJECTS, (), after=after)


def judge_accountmark(answer):
    """(name, True / False / None, detail): `icon@<surface>` and `colour@<surface>` for each surface, as the seam names them; a surface not read is NOT JUDGED."""
    marks = answer.get("marks") or {}
    ring = marks.get("sidebar-ring") or {}
    out = [("account-added", answer.get("stage") == "added", "stage %r, error %r" % (answer.get("stage"), answer.get("error"))),
           ("core-is-rust", answer.get("core") in (None, "rust"), "uistate core %r" % answer.get("core")),
           ("one-account-in-the-sidebar", (answer.get("first_row") or {}).get("sidebar_accounts") == 1, "sidebar accounts %r" % (answer.get("first_row"),)),
           ("colour-is-the-first-palette-colour-of-the-core", (ring.get("colour") or "").upper() == ACCOUNT_MARK_COLOUR, "sidebar ring colour %r (want %s)" % (ring.get("colour"), ACCOUNT_MARK_COLOUR))]
    pick = answer.get("icon_pick")
    if pick is not None:      # fix-realcore: the icon picked through Settings > Accounts is what the core holds, and the ring shows it
        picked_ok = pick.get("core_icon") == ACCOUNT_MARK_ICON and ring.get("icon") == ACCOUNT_MARK_ICON
        out.append(("icon-picked-through-settings-is-held-and-on-the-ring", picked_ok if pick.get("account_read") else None,
                    "picked %r: the core's account_icon %r, the sidebar ring %r%s" % (ACCOUNT_MARK_ICON, pick.get("core_icon"), ring.get("icon"), "; door error: %s" % pick["error"] if pick.get("error") else "")))
    # A NEW real-core account has NO icon: nothing in the add path writes `account.icon_id` (store migrations 0001_v1.sql:15 is a bare TEXT, realcore.rs:50 reads it), so the sidebar
    # ring serves null. The fixture core's "provider.imap" is the fixture's own pick. A null is not a value to compare (None against None can never pass): with no icon on the ring
    # each `icon@<surface>` is NOT JUDGED with that reason, and ONE case judges the part that can be read: every surface that is read serves the icon key, and serves null.
    no_icon = not ring.get("icon")
    for field, ref in (("icon", ring.get("icon")), ("colour", ring.get("colour"))):
        for surface in ACCOUNT_MARK_SURFACES:
            m = marks.get(surface)
            if m is None:
                out.append(("%s@%s" % (field, surface), None, "not read (%s)" % (answer.get("search_note") if surface == "search-result" and answer.get("search_note") else "read error %r" % answer.get("read_error")))); continue
            got = m.get(field)
            if field == "icon" and no_icon:
                out.append(("icon@%s" % surface, None, "the sidebar ring has no icon to compare with (a new account has none until one is picked%s); the surface served %r" % (
                    "" if pick is None else ", and the pick through Settings left %r" % (pick.get("core_icon"),), got))); continue
            ok = bool(got) and bool(ref) and (str(got).upper() == str(ref).upper() if field == "colour" else got == ref)
            out.append(("%s@%s" % (field, surface), ok, "" if ok else "expected the sidebar ring's %r, got %r" % (ref, got)))
    read = {k: v for k, v in marks.items() if v is not None}
    if marks.get("sidebar-ring") is not None and no_icon:
        gaps = sorted(k for k, v in read.items() if not v.get("served"))
        odd = sorted(k for k, v in read.items() if v.get("served") and no_icon and v.get("icon") is not None)
        out.append(("no-icon-is-served-as-null-on-every-surface-read", not gaps and not odd,
                    "surfaces read %s; icon key missing on %s; an icon shown where the ring has none on %s" % (sorted(read), gaps, odd)))
    n = marks.get("notch")
    if n is None:
        why = answer.get("notch_note") or "the notch shows an arrival: this run delivered none (the fixture seam, checks/seams/account-mark.py, reads it)"
        out.append(("icon@notch", None, why))
        out.append(("colour@notch", None, why))
    else:
        icon_ref, colour_ref = ring.get("icon"), ring.get("colour")
        out.append(("icon@notch", None if no_icon else (n.get("icon") == icon_ref), "the sidebar ring has no icon to compare with" if no_icon else "expected the sidebar ring's %r, got %r" % (icon_ref, n.get("icon"))))
        out.append(("colour@notch", bool(n.get("colour")) and bool(colour_ref) and str(n.get("colour")).upper() == str(colour_ref).upper(), "expected the sidebar ring's %r, got %r" % (colour_ref, n.get("colour"))))
    return out


def main_slice_accountmark(bundle):
    """The local servers in THIS process (slice 3's mail), the real core, hidden; the surfaces are read inside. Exit 1 on any FAIL; the notch's two NOT JUDGED lines are named in the count."""
    import json
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import realcore_mailfake as fake
    try:
        mail, imap, smtp = fake.start(0, 0, inbox=0)
    except OSError as e:
        print("the local servers did not start: %s" % e)
        print("1 checks, 0 failed, 1 not judged"); return 1
    load_flow_mail(fake, mail)
    ports = "%d:%d" % (imap.server_address[1], smtp.server_address[1])
    extra = ["WHOOSH_CORE_PLAIN_LOOPBACK=1", "WHOOSH_CORE_CHECK_SERVERS=" + ports]
    import shutil, tempfile, threading
    trig_dir = tempfile.mkdtemp(prefix="whoosh-accountmark-")
    trigger = os.path.join(trig_dir, "arrive")
    env = dict(os.environ, WHOOSH_RIG_HIDDEN="1", LAUNCH_ENV=" ".join(["WHOOSH_RIG_HIDDEN=1"] + extra), WHOOSH_CHECK_ARRIVE=trigger)
    env.pop("WHOOSH_FIXTURE_CORE", None)
    done = threading.Event()

    def watch():      # one arrival, when the driver asks for it
        while not done.is_set():
            if os.path.exists(trigger) and not os.path.exists(trigger + ".done"):
                fake.deliver(mail, accountmark_arrival_raw(fake.ADDRESS))
                with open(trigger + ".done", "w") as f:
                    f.write("delivered\n")
            time.sleep(0.2)
    watcher = threading.Thread(target=watch, daemon=True)
    watcher.start()
    cmd = ["sh", os.path.join(ROOT, "scripts", "launch-one.sh"), bundle, "--", "--rig-hidden",
           "sh", "-c", 'WHOOSH_SOCKET_DIR="$D" python3 "$0" --inner --slice-accountmark', os.path.abspath(__file__)]
    try:
        r = launch(cmd, env)
    finally:
        done.set(); watcher.join(2)
        shutil.rmtree(trig_dir, ignore_errors=True)
    imap.shutdown(); smtp.shutdown()
    out = r.stdout + r.stderr
    line = next((l for l in out.splitlines() if l.startswith("ANSWER ")), None)
    if line is None:
        print(out.rstrip()[-3000:])
        print("launcher exit %d" % r.returncode)
        print("1 checks, 0 failed, 1 not judged"); return 1
    answer = json.loads(line[len("ANSWER "):])
    print("launcher exit %d; the local server saw: %s; %s" % (r.returncode, sorted({l.split(" ")[0] for l in mail.log}), next((l for l in out.splitlines() if l.startswith("instances:")), "")))
    results = judge_accountmark(answer)
    for n, ok, d in results:
        print("%-10s accountmark/%s  %s" % ("PASS" if ok else ("NOT JUDGED" if ok is None else "FAIL"), n, d))
    failed = sum(1 for _, ok, _ in results if ok is False)
    nj = sum(1 for _, ok, _ in results if ok is None)
    print("%d checks, %d failed, %d not judged" % (len(results), failed, nj)); return 0 if failed == 0 and nj == 0 else 1   # not judged is not a pass: the seam (checks/seams/account-mark.py) exits 1 on the same count


def selftest_accountmark():
    """The judge against a model answer (every judged case passes, the notch NOT JUDGED) and one broken surface per case (each case red, only its own); no app."""
    def good():
        m = {"icon": ACCOUNT_MARK_ICON, "colour": ACCOUNT_MARK_COLOUR, "served": True}
        return {"stage": "added", "core": "rust", "first_row": {"id": "m1", "sender": "Boss", "sidebar_accounts": 1}, "marks": {s: dict(m) for s in ACCOUNT_MARK_SURFACES},
                "icon_pick": {"account_read": True, "core_icon": ACCOUNT_MARK_ICON, "error": None}}
    def broken(surface, field, value):
        a = good(); a["marks"][surface][field] = value; return a
    verdict = lambda a: {n: ok for n, ok, _ in judge_accountmark(a)}
    g = verdict(good())
    ok_good = all(v is True for k, v in g.items() if "notch" not in k) and g["icon@notch"] is None and g["colour@notch"] is None
    survived, noisy = [], []
    for surface in ACCOUNT_MARK_SURFACES:
        for field, bad in (("icon", "envelope"), ("colour", "#007AFF")):
            got = verdict(broken(surface, field, bad))
            want = "%s@%s" % (field, surface)
            if surface == "sidebar-ring":      # the ring IS the reference: a wrong ring is caught by the palette case and by every other surface disagreeing with it
                if field == "colour" and got["colour-is-the-first-palette-colour-of-the-core"] is not False:
                    survived.append(want)
                continue
            if got[want] is not False:
                survived.append(want)
            extra = sorted(k for k, v in got.items() if v is False and k != want)
            if extra:
                noisy.append("%s also reddens %s" % (want, extra))
    unread = good(); unread["marks"].pop("search-result")
    ok_unread = verdict(unread)["icon@search-result"] is None and verdict(unread)["colour@search-result"] is None
    two = good(); two["first_row"]["sidebar_accounts"] = 2
    ok_two = verdict(two)["one-account-in-the-sidebar"] is False
    # the REAL core's new account: no icon anywhere (null on every surface). The icon cases are NOT JUDGED (never a pass on None), the colour cases pass, the null case passes
    def noicon():
        a = good()
        a.pop("icon_pick")      # an answer with no pick (the slice before fix-realcore): the null case alone
        for v in a["marks"].values():
            v["icon"] = None
        return a
    lost = noicon(); lost["icon_pick"] = {"account_read": True, "core_icon": None, "error": "no such door"}
    lv = verdict(lost)
    ok_lost = lv["icon-picked-through-settings-is-held-and-on-the-ring"] is False and all(lv["icon@%s" % x] is None for x in ACCOUNT_MARK_SURFACES)
    ring_only = good(); ring_only["marks"]["sidebar-ring"]["icon"] = None
    ok_lost = ok_lost and verdict(ring_only)["icon-picked-through-settings-is-held-and-on-the-ring"] is False
    # the notch, once the run delivered an arrival: read like the other surfaces; a wrong mark is red; no row read stays NOT JUDGED with the driver's note
    lit = good(); lit["marks"]["notch"] = {"icon": ACCOUNT_MARK_ICON, "colour": ACCOUNT_MARK_COLOUR, "served": True}
    accent = good(); accent["marks"]["notch"] = {"icon": ACCOUNT_MARK_ICON, "colour": "#007AFF", "served": True}
    generic = good(); generic["marks"]["notch"] = {"icon": "envelope", "colour": ACCOUNT_MARK_COLOUR, "served": True}
    unlit = good(); unlit["notch_note"] = "delivered True; the notch listed 0 row(s) after 20s, the arrival among them: False"
    lv, av, gv, uv = verdict(lit), verdict(accent), verdict(generic), {n: (ok, d) for n, ok, d in judge_accountmark(unlit)}
    ok_lost = ok_lost and lv["icon@notch"] is True and lv["colour@notch"] is True and av["colour@notch"] is False and av["icon@notch"] is True \
        and gv["icon@notch"] is False and gv["colour@notch"] is True and uv["icon@notch"][0] is None and "listed 0 row(s)" in uv["icon@notch"][1]
    ni = verdict(noicon())
    ok_noicon = all(ni["icon@%s" % x] is None for x in ACCOUNT_MARK_SURFACES) and ni["no-icon-is-served-as-null-on-every-surface-read"] is True and all(ni["colour@%s" % x] is True for x in ACCOUNT_MARK_SURFACES)
    miss = noicon(); miss["marks"]["list-row"]["served"] = False
    shown = noicon(); shown["marks"]["reader-meta"]["icon"] = "envelope"
    ok_noicon = ok_noicon and verdict(miss)["no-icon-is-served-as-null-on-every-surface-read"] is False and verdict(shown)["no-icon-is-served-as-null-on-every-surface-read"] is False
    nosearch = noicon(); nosearch["marks"].pop("search-result"); nosearch["search_note"] = "searched 'Kim': 0 result rows"
    ok_noicon = ok_noicon and "0 result rows" in {n: d for n, _, d in judge_accountmark(nosearch)}["icon@search-result"]
    print("selftest slice accountmark: a lost icon pick %s; model %s, %d broken surfaces %s, a surface not read %s, two accounts %s%s; a core with no icon %s" % (
        "red on the pick case, the icon cases NOT JUDGED" if ok_lost else "WRONG",
        "all judged cases pass (the notch NOT JUDGED)" if ok_good else "FAILS", 2 * (len(ACCOUNT_MARK_SURFACES) - 1), "each red on its own case" if not survived else "SURVIVE %s" % survived,
        "NOT JUDGED" if ok_unread else "WRONG", "red" if ok_two else "SURVIVES", "" if not noisy else "; " + "; ".join(noisy), "keeps the icon cases NOT JUDGED and judges the null" if ok_noicon else "WRONG"))
    return ok_good and not survived and ok_unread and ok_two and not noisy and ok_noicon and ok_lost


# ---------------------------------------------------------------- lane gap-mailview-checks: the mail view on the REAL engine
# --slice-mailview: the body on selection (no Enter), the file download through the app's own DownloadAttachment, Space and the Quick Look button on a file
# that is not on this Mac, the inline logo left out of the gallery. --slice-labels: the four built-in labels on a new installation. Both run the real core
# against the local fake (checks/realcore_mailfake.py, 127.0.0.1); nothing real is contacted. The fake gets the one thing it lacked, the answer to a numbered
# part fetch (`UID FETCH n (UID BODY.PEEK[2])`, what the core's DownloadAttachment sends): patched in THIS process only (`fake.fetch_line`), never edited.
MV_WAIT_S = 45           # the sync, the body prefetch and the parse each land in the background; every wait is a poll up to this
MV_PART_DELAY_S = 2.0    # the fake holds each part answer back this long, so "downloading" can be read before the file lands (a fake's own pace)
MV_HTML_SUBJECT, MV_PLAIN_SUBJECT, MV_FILES_SUBJECT = "Weekly digest from Orbit Supply", "Notes from the planning call", "Quote files for the first quarter"
MV_HTML_TEXT = ["Orbit Supply weekly digest for the owner of the shop.", "The spring catalogue is out with forty new items in every range.",
                "Order before Friday noon and the delivery is free of charge.", "Our returns desk is open from nine to five on every working day.",
                "Ask for the trade price list if you buy more than twenty units.", "The warehouse moves to the new site on the first of December.",
                "Thank you for buying from Orbit Supply this year."]
MV_PLAIN_TEXT = ["Hello, here are the notes from the planning call this morning.", "We agreed to ship the first version of the plan on Thursday.",
                 "Pat will write the budget and Kim will check the dates with the team.", "The next call is on Monday at ten, in the same room as before.",
                 "Please send me anything you want added before the end of the day.", "I will collect the answers and send one summary to everyone.",
                 "Thanks again for your time and for the clear questions."]
MV_FILES = ["Quote-Q1.docx", "Prices-Q1.xlsx"]       # not PDF or plain text: those two kinds are fetched in the background by themselves (attachment_auto.rs, SPEC A152)
MV_LOGO = "logo.png"
# The two orders differ, and both are the app's own: the tiles list the names SORTED (store realcore.rs:156 `ORDER BY file_name`: Prices before Quote), the engine's attachment id
# carries the MIME ordinal (realcore.rs:1012 `attachment_row_id`, uid 3's part order: Quote 000, Prices 001, the logo 002). A tile's own id is therefore found by its NAME.
MV_TILE_ORDER = sorted(MV_FILES + [MV_LOGO])[:2]      # what the chips list, in order (the logo is not a chip)


def mv_att_id(message, name):
    """The engine's id of the file called `name` in the file mail: `<message>:att:<MIME ordinal>`; None for a name the mail does not carry."""
    return "%s:att:%03d" % (message, (MV_FILES + [MV_LOGO]).index(name)) if name in MV_FILES + [MV_LOGO] else None
MV_DOCX = "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
MV_XLSX = "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
MV_PARTS = {1: "text", 2: MV_FILES[0], 3: MV_FILES[1], 4: MV_LOGO}   # the root's children, as IMAP numbers them (parse::mime section_numbers)


def mv_messages(fake):
    """The three raw INBOX messages: uid 1 plain, uid 2 an HTML newsletter, uid 3 (newest, first on the list) a mail with two files and an inline logo."""
    import email.message
    import email.policy

    def base(uid, subject, name, addr):
        m = email.message.EmailMessage(policy=email.policy.SMTP)
        m["From"], m["To"], m["Subject"] = "%s <%s>" % (name, addr), fake.ADDRESS, subject
        m["Message-ID"] = "<mv-%d@whoosh-check.invalid>" % uid
        m["Date"] = "Mon, 09 Nov 2026 10:%02d:00 +0000" % uid
        return m
    plain = base(1, MV_PLAIN_SUBJECT, "Pat Plan", "pat@example.test")
    plain.set_content("\n\n".join(MV_PLAIN_TEXT) + "\n")
    html = base(2, MV_HTML_SUBJECT, "Orbit Supply", "digest@orbit-supply.example")
    html.set_content("\n\n".join(MV_HTML_TEXT) + "\n")
    html.add_alternative("<html><body style=\"font-family:Georgia\">" + "".join("<p>%s</p>" % t for t in MV_HTML_TEXT) + "</body></html>", subtype="html")
    files = base(3, MV_FILES_SUBJECT, "Compta", "compta@files.example")
    files.set_content("Hello, the two files for the first quarter are attached.\n")
    payload = lambda n: (b"fake file bytes for the check, " + n.encode() + b"\n") * 60
    files.add_attachment(payload(MV_FILES[0]), maintype="application", subtype=MV_DOCX.split("/")[1], filename=MV_FILES[0])
    files.add_attachment(payload(MV_FILES[1]), maintype="application", subtype=MV_XLSX.split("/")[1], filename=MV_FILES[1])
    files.add_attachment(payload(MV_LOGO), maintype="image", subtype="png", filename=MV_LOGO, disposition="inline", cid="<logo@files.example>")
    return [m.as_bytes(policy=email.policy.SMTP) for m in (plain, html, files)]


def mv_part_bytes(raw, section):
    """The bytes of one numbered part in its transfer encoding (what `BODY.PEEK[<n>]` answers): the root's children are 1, 2, 3, a multipart's are n.1, n.2."""
    import email
    msg = email.message_from_bytes(raw)
    for n in (int(x) for x in section.split(".")):
        if msg.is_multipart():
            msg = msg.get_payload()[n - 1]
        elif n != 1:
            raise IndexError("no part %s" % section)
    return str(msg.get_payload()).encode("ascii", "replace")


def mv_patch_fetch(fake, part_log):
    """Teach the fake the numbered part fetch and record each one (uid, section). Returns the function that puts the fake back."""
    original = fake.fetch_line

    def fetch_line(seq, uid, raw, items, flags=()):
        line = original(seq, uid, raw, items, flags)
        m = re.search(r"BODY\.PEEK\[(\d+(?:\.\d+)*)\]", items, re.I)
        if not m:
            return line
        part_log.append((uid, m.group(1)))
        time.sleep(MV_PART_DELAY_S)
        data = mv_part_bytes(raw, m.group(1))
        return line[:-3] + b" BODY[%s] {%d}\r\n" % (m.group(1).encode(), len(data)) + data + b")\r\n"
    fake.fetch_line = fetch_line
    return lambda: setattr(fake, "fetch_line", original)


def mv_add_account(c, fake, answer, want_rows):
    """Inside launch-one: the sheet adds the local account, the first-start flow is read through, the list is waited for (the shared start of the two slices)."""
    c.click(A + ".open")
    c.click(A + ".address"); c.type(CHECK_ADDRESS)
    c.click(A + ".password"); c.type(fake.PASSWORD)
    c.click(A + ".continue")
    t0 = time.time()
    while time.time() - t0 < DETECT_WAIT_S:
        st = c.rows(A + ".state")
        if st and st[0].get("stage") not in ("detecting", "entry"):
            break
        time.sleep(0.3)
    c.click(A + ".add")
    stage = None
    t0 = time.time()
    while time.time() - t0 < DETECT_WAIT_S:
        st = c.rows(A + ".state"); stage = st[0].get("stage") if st else None
        if stage in ("added", "summary"):
            break
        time.sleep(0.3)
    answer["stage"] = stage
    try:
        c.click(A + ".done")
    except Exception as e:      # noqa: BLE001
        answer["done"] = str(e)
    for _ in range(10):         # the first-start flow swallows the mail keys until it ends (main55)
        try:
            st = c.rows("mac-first-start.state")
        except Exception:       # noqa: BLE001
            break
        if not (st and st[0].get("active")):
            break
        try:
            c.key("Enter")
        except Exception:       # noqa: BLE001
            break
        time.sleep(0.5)
    time.sleep(1.5)
    rows = []
    t0 = time.time()
    while time.time() - t0 < MV_WAIT_S and len(rows) < want_rows:
        try:
            rows = c.rows("mac-main.list")
        except Exception:       # noqa: BLE001
            rows = []
        time.sleep(0.5)
    answer["rows"] = [{"id": r.get("id"), "subject": r.get("subject"), "preview": r.get("preview")} for r in rows]


def mv_poll(fn, ok, wait_s=MV_WAIT_S):
    """Reads `fn` until `ok(value)` or the budget ends: (last value, seconds). An unreadable value reads as None."""
    t0 = time.time()
    got = None
    while True:
        try:
            got = fn()
        except Exception:       # noqa: BLE001  Unreadable: not served yet
            got = None
        try:
            done = bool(ok(got))
        except Exception:       # noqa: BLE001  a value that is not there yet (None) does not satisfy the wait
            done = False
        if done or time.time() - t0 >= wait_s:
            return got, round(time.time() - t0, 2)
        time.sleep(0.15)


def mv_select(c, key, subject, sentences):
    """One move with a key (no Enter), its commit time, then the body the reader holds for the focused mail, waited for until it carries every sentence."""
    out = {"subject": subject}
    c.key(key)
    out["commit_ms"] = float(c.measure("mac-main.interaction.commit")["ms"])
    out["focus"] = c.uistate().get("focus_message")
    join = lambda r: " ".join(r[0].get("paragraphs") or [])
    body, secs = mv_poll(lambda: c.rows("mac-main.reader.body"), lambda r: bool(r) and all(s in join(r) for s in sentences))
    out["paragraphs"] = (body or [{}])[0].get("paragraphs") or []
    out["body_seconds"] = secs
    for r in c.rows("mac-main.list"):
        if r.get("id") == out["focus"]:
            out["row_subject"], out["preview"] = r.get("subject"), r.get("preview")
    return out


def mv_tstate(c, att_id):
    """`core state attachment/<id>/transfer` as its word (not_on_this_mac, downloading, on_this_mac, failed), None when the path is not served."""
    v = cv(c, "attachment/%s/transfer" % att_id)
    return v.get("state") if isinstance(v, dict) else v


def mv_snap(c, att_id, i, region="mac-main.attachments"):
    """What the tile, the panel and the core say about file i at one moment."""
    chip = c.rows(region)[i]
    pan = (c.rows(region + ".panel") or [{}])[0]
    return {"state": chip.get("state"), "waiting": chip.get("waiting"), "percent": chip.get("percent"), "name": chip.get("name"), "panel_open": pan.get("open"),
            "panel_item": pan.get("item"), "transfer": mv_tstate(c, att_id), "id": att_id}


def drive_mailview():
    """Inside launch-one: select the mails (body and commit), open the file mail, then Space and the Quick Look button on files that are not on this Mac."""
    sys.path.insert(0, os.path.join(ROOT, "journeys"))
    from lib.ctl import Ctl  # noqa: E402
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import realcore_mailfake as fake  # noqa: E402
    c = Ctl().connect()
    R = "mac-main.attachments"
    answer = {"driver_errors": [], "core": None}

    def step(label, fn):
        try:
            return fn()
        except Exception as e:      # noqa: BLE001  a step that cannot run is told, never a crash of the whole answer
            answer["driver_errors"].append("%s: %s: %s" % (label, type(e).__name__, e))
            return None
    mv_add_account(c, fake, answer, 3)
    answer["core"] = c.uistate().get("core")
    top = (answer.get("rows") or [{}])[0].get("subject")
    answer["top_subject"] = top
    answer["html_sel"] = step("select html mail", lambda: mv_select(c, "J", MV_HTML_SUBJECT, MV_HTML_TEXT))
    answer["plain_sel"] = step("select plain mail", lambda: mv_select(c, "J", MV_PLAIN_SUBJECT, MV_PLAIN_TEXT))
    for _ in range(2):
        step("back to the file mail", lambda: c.key("K"))
    answer["files_focus"] = c.uistate().get("focus_message")
    step("open the file mail", lambda: c.key("Enter"))
    time.sleep(0.5)
    step("close the reply field", lambda: c.key("Escape"))
    rows, secs = mv_poll(lambda: c.rows(R), lambda r: len(r) >= 2)
    answer["chips"] = {"names": [r.get("name") for r in rows or []], "states": [r.get("state") for r in rows or []], "seconds": secs}
    answer["gallery"] = step("gallery row", lambda: (c.rows(R + ".gallery") or [{}])[0])
    if not rows or len(rows) < 2:
        return answer
    ids = [mv_att_id(r.get("message"), r.get("name")) for r in rows]      # by NAME: the tile's index is the sorted place, not the MIME ordinal
    answer["ids"] = ids
    if None in ids:
        answer["driver_errors"].append("a tile named a file the mail does not carry: %r" % [r.get("name") for r in rows])
        return answer
    answer["before"] = step("before any act", lambda: mv_snap(c, ids[0], 0))
    # Space on the first file: the tile reads fetching and the preview waits for the bytes
    step("select file 0", lambda: c.click(R + ".0"))
    step("Space", lambda: c.key("Space"))
    fetching, secs = mv_poll(lambda: mv_snap(c, ids[0], 0), lambda s: s["state"] == "fetching", 10)
    answer["space_fetching"] = dict(fetching or {}, seconds=secs)
    landed, secs = mv_poll(lambda: mv_snap(c, ids[0], 0), lambda s: s["state"] == "present" and s["panel_open"] is True)
    answer["space_landed"] = dict(landed or {}, seconds=secs)
    step("Space closes", lambda: c.key("Space"))
    closed, _ = mv_poll(lambda: mv_snap(c, ids[0], 0), lambda s: s["panel_open"] is False, 5)
    answer["space_closed"] = closed
    # the Quick Look button with the second file selected
    answer["button_before"] = step("before the button", lambda: mv_snap(c, ids[1], 1))
    step("select file 1", lambda: c.click(R + ".1"))
    step("Quick Look button", lambda: c.click(R + ".gallery.quick-look"))
    fetching, secs = mv_poll(lambda: mv_snap(c, ids[1], 1), lambda s: s["state"] == "fetching", 10)
    answer["button_fetching"] = dict(fetching or {}, seconds=secs)
    landed, secs = mv_poll(lambda: mv_snap(c, ids[1], 1), lambda s: s["state"] == "present" and s["panel_open"] is True)
    answer["button_landed"] = dict(landed or {}, seconds=secs)
    step("Escape closes", lambda: c.key("Escape"))
    closed, _ = mv_poll(lambda: mv_snap(c, ids[1], 1), lambda s: s["panel_open"] is False, 5)
    answer["button_closed"] = closed
    return answer


def mv_commit_limit():
    sys.path.insert(0, os.path.join(ROOT, "journeys"))
    from lib.fixture import const  # noqa: E402
    return float(const("INTERACTION_COMMIT_MS"))


def judge_mailview(answer, server, limit_ms):
    """`answer`: what the app showed (inside launch-one). `server`: {"parts": [[uid, section], ...]} the part fetches the fake received (this process)."""
    out = []

    def add(name, ok, detail):
        out.append((name, ok, detail))
    add("account-added", answer.get("stage") in ("added", "summary"), "stage %r, core %r" % (answer.get("stage"), answer.get("core")))
    add("core-is-rust", answer.get("core") in (None, "rust"), "uistate core %r" % answer.get("core"))
    add("list-shows-the-three-mails-newest-first", [r.get("subject") for r in answer.get("rows") or []] == [MV_FILES_SUBJECT, MV_HTML_SUBJECT, MV_PLAIN_SUBJECT],
        "rows %r" % [r.get("subject") for r in answer.get("rows") or []])
    for tag, key, sentences in (("html", "html_sel", MV_HTML_TEXT), ("plain", "plain_sel", MV_PLAIN_TEXT)):
        sel = answer.get(key)
        if not sel:
            for n in ("selection-shows-the-whole-%s-body", "selected-%s-body-is-longer-than-the-list-preview", "selection-commit-of-the-%s-mail-is-under-the-budget"):
                add(n % tag, None, "the selection step did not run: %s" % answer.get("driver_errors"))
            continue
        joined = " ".join(sel.get("paragraphs") or [])
        missing = [s for s in sentences if s not in joined]
        add("selection-shows-the-whole-%s-body" % tag, not missing and "<" not in joined and sel.get("row_subject") == sel["subject"],
            "focused row %r; %d paragraphs, %d characters, after %ss; sentences missing %r; raw markup %s" % (sel.get("row_subject"), len(sel.get("paragraphs") or []), len(joined), sel.get("body_seconds"), missing, "<" in joined))
        prev = sel.get("preview") or ""
        add("selected-%s-body-is-longer-than-the-list-preview" % tag, len(joined) > len(prev) > 0, "body %d characters, the list row's preview %d characters (%r)" % (len(joined), len(prev), prev[:60]))
        add("selection-commit-of-the-%s-mail-is-under-the-budget" % tag, sel["commit_ms"] < limit_ms, "`measure mac-main.interaction.commit` after the key: %.1f ms, budget %g ms (INTERACTION_COMMIT_MS)" % (sel["commit_ms"], limit_ms))
    chips = answer.get("chips") or {}
    gal = answer.get("gallery") or {}
    listed = list(chips.get("names") or []) + list(gal.get("media") or []) + list(gal.get("documents") or []) + list(gal.get("chips") or []) + list(gal.get("tiles") or [])
    add("the-file-mail-is-the-focused-mail", answer.get("files_focus") is not None and answer.get("files_focus") == (answer.get("rows") or [{}])[0].get("id"), "focus %r, top row %r" % (answer.get("files_focus"), (answer.get("rows") or [{}])[0].get("id")))
    add("the-gallery-lists-the-two-files-and-not-the-inline-logo", sorted(chips.get("names") or []) == sorted(MV_FILES) and MV_LOGO not in listed,
        "chips %r after %ss; gallery media %r documents %r chips %r tiles %r inline %r" % (chips.get("names"), chips.get("seconds"), gal.get("media"), gal.get("documents"), gal.get("chips"), gal.get("tiles"), gal.get("inline")))
    b, f, l, cl = (answer.get(k) for k in ("before", "space_fetching", "space_landed", "space_closed"))
    tile_name = lambda i: ((chips.get("names") or [None, None]) + [None, None])[i]      # the name the tile itself read: the panel must open on THAT file
    if not b:
        for n in ("an-absent-file-is-named-absent-before-any-act", "space-on-an-absent-file-starts-the-download-and-waits", "the-core-says-downloading-then-on-this-mac",
                  "the-tile-shows-fetching-with-no-figure", "the-preview-opens-when-the-file-lands", "space-closes-the-preview", "the-quick-look-button-starts-the-download-and-waits",
                  "the-button-opens-the-preview-when-the-file-lands", "escape-closes-the-button-preview"):
            add(n, None, "no file tile was served, so no act could be driven: %s" % answer.get("driver_errors"))
    else:
        add("an-absent-file-is-named-absent-before-any-act", b.get("state") == "absent" and b.get("transfer") == "not_on_this_mac" and b.get("panel_open") is not True, "tile %r, core %r, panel open %r" % (b.get("state"), b.get("transfer"), b.get("panel_open")))
        add("space-on-an-absent-file-starts-the-download-and-waits", f.get("state") == "fetching" and f.get("waiting") == "quick-look" and f.get("panel_open") is not True,
            "tile %r waiting %r, panel open %r, after %ss" % (f.get("state"), f.get("waiting"), f.get("panel_open"), f.get("seconds")))
        add("the-core-says-downloading-then-on-this-mac", f.get("transfer") == "downloading" and l.get("transfer") == "on_this_mac",
            "`core state attachment/%s/transfer`: %r while the tile fetched, %r when it landed" % (b.get("id"), f.get("transfer"), l.get("transfer")))
        add("the-tile-shows-fetching-with-no-figure", f.get("state") == "fetching" and f.get("percent") is None, "tile %r, percent %r (the engine gives no figure: null)" % (f.get("state"), f.get("percent")))
        add("the-preview-opens-when-the-file-lands", l.get("state") == "present" and l.get("panel_open") is True and tile_name(0) is not None and l.get("panel_item") == tile_name(0),
            "tile %r, panel open %r on %r (want %r, the name of tile 0), after %ss" % (l.get("state"), l.get("panel_open"), l.get("panel_item"), tile_name(0), l.get("seconds")))
        add("space-closes-the-preview", bool(cl) and cl.get("panel_open") is False, "panel open %r" % (cl or {}).get("panel_open"))
        bb, bf, bl, bc = (answer.get(k) for k in ("button_before", "button_fetching", "button_landed", "button_closed"))
        add("the-quick-look-button-starts-the-download-and-waits", bool(bb) and bb.get("state") == "absent" and bf.get("state") == "fetching" and bf.get("waiting") == "quick-look" and bf.get("transfer") == "downloading",
            "before: tile %r; after the click: tile %r waiting %r, core %r, panel open %r" % ((bb or {}).get("state"), bf.get("state"), bf.get("waiting"), bf.get("transfer"), bf.get("panel_open")))
        add("the-button-opens-the-preview-when-the-file-lands", bl.get("state") == "present" and bl.get("panel_open") is True and tile_name(1) is not None and bl.get("panel_item") == tile_name(1) and bl.get("transfer") == "on_this_mac",
            "tile %r, panel open %r on %r (want %r, the name of tile 1), core %r, after %ss" % (bl.get("state"), bl.get("panel_open"), bl.get("panel_item"), tile_name(1), bl.get("transfer"), bl.get("seconds")))
        add("escape-closes-the-button-preview", bool(bc) and bc.get("panel_open") is False, "panel open %r" % (bc or {}).get("panel_open"))
    parts = sorted(tuple(p) for p in server.get("parts") or [])
    want = [(3, "2"), (3, "3")]
    add("the-server-saw-one-part-fetch-per-file-and-none-for-the-logo", parts == want, "part fetches %r (want %r: parts %s are the files, part 4 is the inline logo)" % (parts, want, "2 and 3"))
    add("the-driver-reached-every-step", not answer.get("driver_errors"), "driver errors %r" % answer.get("driver_errors"))
    return out


MV_ALL_CASES = ("selection-shows-the-whole-html-body", "selection-shows-the-whole-plain-body", "selected-html-body-is-longer-than-the-list-preview", "selected-plain-body-is-longer-than-the-list-preview",
                "selection-commit-of-the-html-mail-is-under-the-budget", "selection-commit-of-the-plain-mail-is-under-the-budget",
                "the-gallery-lists-the-two-files-and-not-the-inline-logo", "an-absent-file-is-named-absent-before-any-act", "space-on-an-absent-file-starts-the-download-and-waits",
                "the-core-says-downloading-then-on-this-mac", "the-tile-shows-fetching-with-no-figure", "the-preview-opens-when-the-file-lands", "space-closes-the-preview",
                "the-quick-look-button-starts-the-download-and-waits", "the-button-opens-the-preview-when-the-file-lands", "escape-closes-the-button-preview",
                "the-server-saw-one-part-fetch-per-file-and-none-for-the-logo")


def mv_good_answer():
    """A model answer of the real core: what the app would show when every case holds (the self-test's clean run; never read from a build)."""
    rows = [{"id": "acc:INBOX:1:3", "subject": MV_FILES_SUBJECT, "preview": "Hello, the two files"}, {"id": "acc:INBOX:1:2", "subject": MV_HTML_SUBJECT, "preview": "Orbit Supply weekly digest"},
            {"id": "acc:INBOX:1:1", "subject": MV_PLAIN_SUBJECT, "preview": "Hello, here are the notes"}]
    sel = lambda subject, row, text: {"subject": subject, "commit_ms": 9.0, "focus": row["id"], "row_subject": subject, "preview": row["preview"], "paragraphs": list(text), "body_seconds": 0.4}
    snap = lambda i, state, panel, transfer, **k: dict({"state": state, "waiting": "", "percent": 100 if state == "present" else 0, "name": MV_TILE_ORDER[i], "panel_open": panel, "panel_item": MV_TILE_ORDER[i] if panel else None,
                                                        "transfer": transfer, "id": mv_att_id("acc:INBOX:1:3", MV_TILE_ORDER[i])}, **k)
    return {"stage": "added", "core": "rust", "rows": rows, "driver_errors": [], "html_sel": sel(MV_HTML_SUBJECT, rows[1], MV_HTML_TEXT), "plain_sel": sel(MV_PLAIN_SUBJECT, rows[2], MV_PLAIN_TEXT),
            "files_focus": rows[0]["id"], "chips": {"names": list(MV_TILE_ORDER), "states": ["absent", "absent"], "seconds": 0.6},
            "gallery": {"media": [], "documents": list(MV_FILES), "chips": list(MV_FILES), "tiles": [], "inline": []},
            "before": snap(0, "absent", False, "not_on_this_mac"), "space_fetching": snap(0, "fetching", False, "downloading", waiting="quick-look", percent=None, seconds=0.2),
            "space_landed": snap(0, "present", True, "on_this_mac", seconds=2.3), "space_closed": snap(0, "present", False, "on_this_mac"),
            "button_before": snap(1, "absent", False, "not_on_this_mac"), "button_fetching": snap(1, "fetching", False, "downloading", waiting="quick-look", percent=None, seconds=0.2),
            "button_landed": snap(1, "present", True, "on_this_mac", seconds=2.2), "button_closed": snap(1, "present", False, "on_this_mac")}, {"parts": [[3, "2"], [3, "3"]]}


def mv_mutants(good):
    """One broken answer per case: exactly that case must turn red (the broken copy of the case's behaviour). Each is a function of a deep copy of the answer and the server record."""
    import copy

    def edit(fn):
        a, s = copy.deepcopy(good[0]), copy.deepcopy(good[1])
        fn(a, s)
        return a, s
    return {
        "selection-shows-the-whole-html-body": edit(lambda a, s: a["html_sel"].update(paragraphs=a["html_sel"]["paragraphs"][:1])),
        "selection-shows-the-whole-plain-body": edit(lambda a, s: a["plain_sel"].update(paragraphs=a["plain_sel"]["paragraphs"][:1])),
        "selected-html-body-is-longer-than-the-list-preview": edit(lambda a, s: a["html_sel"].update(preview=" ".join(a["html_sel"]["paragraphs"]) + " and more")),
        "selected-plain-body-is-longer-than-the-list-preview": edit(lambda a, s: a["plain_sel"].update(preview=" ".join(a["plain_sel"]["paragraphs"]) + " and more")),
        "selection-commit-of-the-html-mail-is-under-the-budget": edit(lambda a, s: a["html_sel"].update(commit_ms=81.0)),
        "selection-commit-of-the-plain-mail-is-under-the-budget": edit(lambda a, s: a["plain_sel"].update(commit_ms=50.0)),
        "the-gallery-lists-the-two-files-and-not-the-inline-logo": edit(lambda a, s: a["chips"].update(names=a["chips"]["names"] + [MV_LOGO])),
        "an-absent-file-is-named-absent-before-any-act": edit(lambda a, s: a["before"].update(state="present")),
        "space-on-an-absent-file-starts-the-download-and-waits": edit(lambda a, s: a["space_fetching"].update(waiting="")),
        "the-core-says-downloading-then-on-this-mac": edit(lambda a, s: a["space_landed"].update(transfer="downloading")),
        "the-tile-shows-fetching-with-no-figure": edit(lambda a, s: a["space_fetching"].update(percent=0)),
        "the-preview-opens-when-the-file-lands": edit(lambda a, s: a["space_landed"].update(panel_open=False)),
        "space-closes-the-preview": edit(lambda a, s: a["space_closed"].update(panel_open=True)),
        "the-quick-look-button-starts-the-download-and-waits": edit(lambda a, s: a["button_fetching"].update(state="absent")),
        "the-button-opens-the-preview-when-the-file-lands": edit(lambda a, s: a["button_landed"].update(panel_item=a["chips"]["names"][0])),
        "escape-closes-the-button-preview": edit(lambda a, s: a["button_closed"].update(panel_open=True)),
        "the-server-saw-one-part-fetch-per-file-and-none-for-the-logo": edit(lambda a, s: s.update(parts=s["parts"] + [[3, "4"]])),
    }


def selftest_mailview():
    """The judge against a model answer (all pass, none NOT JUDGED), one broken answer per case (that case red), the part fetch patch against Python's own IMAP client."""
    import imaplib
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import realcore_mailfake as fake
    good = mv_good_answer()
    base = judge_mailview(good[0], good[1], 50.0)
    clean = all(ok is True for _, ok, _ in base)
    cases = {n for n, _, _ in base}
    muts = mv_mutants(good)
    bad = []
    for name, (a, s) in muts.items():
        res = judge_mailview(a, s, 50.0)
        red = {n for n, ok, _ in res if ok is not True}
        if red != {name}:
            bad.append("%s -> red %s" % (name, sorted(red)))
    covered = set(muts) == set(MV_ALL_CASES) and set(MV_ALL_CASES) <= cases
    no_run = judge_mailview(dict(good[0], before=None), good[1], 50.0)
    unjudged_ok = all(ok is None for n, ok, _ in no_run if n in ("space-closes-the-preview", "the-quick-look-button-starts-the-download-and-waits"))
    part_log = []
    mail, imap, smtp = fake.start(0, 0, inbox=0)
    put_back = mv_patch_fetch(fake, part_log)
    served = False
    try:
        raws = mv_messages(fake)
        mail.boxes["INBOX"] = [(i + 1, r) for i, r in enumerate(raws)]
        c = imaplib.IMAP4("127.0.0.1", imap.server_address[1])
        c.login(fake.ADDRESS, fake.PASSWORD)
        c.select("INBOX")
        t0 = time.time()
        typ, data = c.uid("FETCH", "3", "(UID BODY.PEEK[2])")
        took = time.time() - t0
        got = b"".join(x[1] for x in data if isinstance(x, tuple))
        import base64
        decoded = base64.b64decode(got)
        typ4, data4 = c.uid("FETCH", "3", "(UID BODY.PEEK[4])")
        logo = base64.b64decode(b"".join(x[1] for x in data4 if isinstance(x, tuple)))
        c.logout()
        served = typ == "OK" and decoded.startswith(b"fake file bytes for the check, " + MV_FILES[0].encode()) and logo.startswith(b"fake file bytes for the check, " + MV_LOGO.encode()) \
            and part_log == [(3, "2"), (3, "4")] and took >= MV_PART_DELAY_S
        import email as _e
        names = [p.get_filename() for p in _e.message_from_bytes(raws[2]).walk() if p.get_filename()]
        # the tile order (sorted names) and the engine's ordinal (MIME order) differ: tile 0 is Prices and its id ends :att:001, tile 1 is Quote and ends :att:000
        served = served and MV_TILE_ORDER == ["Prices-Q1.xlsx", "Quote-Q1.docx"] and [mv_att_id("m", n) for n in MV_TILE_ORDER] == ["m:att:001", "m:att:000"] and mv_att_id("m", "x") is None
        served = served and names == MV_FILES + [MV_LOGO] and [p.get_content_disposition() for p in _e.message_from_bytes(raws[2]).walk()][-1] == "inline"
    finally:
        put_back()
        imap.shutdown(); smtp.shutdown()
    print("selftest slice mailview: model %s, %d broken answers %s, a missing file tile leaves the act cases NOT JUDGED %s, part fetch %s" % (
        "all %d cases pass" % len(base) if clean else "FAILS", len(muts), "each red on its own case" if not bad and covered else "WRONG: %s (covered %s)" % (bad, covered),
        "yes" if unjudged_ok else "NO", "served and recorded (two files, the inline logo by its number)" if served else "WRONG"))
    return clean and not bad and covered and unjudged_ok and served


def mv_launch(bundle, inner_flag, inbox, extra_env=(), extra_boxes=None, start_inbox=0):
    """The launch both slices share: the fake in THIS process, the rig copy with the real core against it, hidden. Returns (answer or None, text, mail, part_log)."""
    import json
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import realcore_mailfake as fake
    try:
        mail, imap, smtp = fake.start(0, 0, inbox=start_inbox)     # start_inbox: the fake's own numbered mails, as --slice2 holds them (default none)
    except OSError as e:
        print("the local servers did not start: %s" % e)
        print("1 checks, 0 failed, 1 not judged"); return None, "", None, []
    part_log = []
    put_back = mv_patch_fetch(fake, part_log)
    for folder, special in (extra_boxes or {}).items():     # a folder the server lists besides INBOX and Sent (set on THIS process's fake only; the fake file is not edited)
        mail.boxes[folder] = []
        mail.special[folder] = special
    if inbox:
        mail.boxes["INBOX"] = [(i + 1, r) for i, r in enumerate(mv_messages(fake))]
    ports = "%d:%d" % (imap.server_address[1], smtp.server_address[1])
    extra = ["WHOOSH_CORE_PLAIN_LOOPBACK=1", "WHOOSH_CORE_CHECK_SERVERS=" + ports, "WHOOSH_ATTACH_AUTO_FETCH=off"] + list(extra_env)
    env = dict(os.environ, WHOOSH_RIG_HIDDEN="1", LAUNCH_ENV=" ".join(["WHOOSH_RIG_HIDDEN=1"] + extra))
    env.pop("WHOOSH_FIXTURE_CORE", None)
    cmd = ["sh", os.path.join(ROOT, "scripts", "launch-one.sh"), os.path.abspath(bundle), "--", "--rig-hidden",
           "sh", "-c", 'WHOOSH_SOCKET_DIR="$D" python3 "$0" --inner ' + inner_flag, os.path.abspath(__file__)]
    try:
        r = launch(cmd, env)
    finally:
        put_back()
        imap.shutdown(); smtp.shutdown()
    out = r.stdout + r.stderr
    line = next((l for l in out.splitlines() if l.startswith("ANSWER ")), None)
    if line is None:
        print(out.rstrip()[-3000:])
        print("launcher exit %d" % r.returncode)
        print("1 checks, 0 failed, 1 not judged"); return None, out, mail, part_log
    print("launcher exit %d; the local server saw: %s; %s" % (r.returncode, sorted({l.split(" ")[0] for l in mail.log}), next((l for l in out.splitlines() if l.startswith("instances:")), "")))
    return json.loads(line[len("ANSWER "):]), out, mail, part_log


def mv_report(results):
    lines = ["%-10s %s  %s" % ("PASS" if ok else ("NOT JUDGED" if ok is None else "FAIL"), n, d) for n, ok, d in results]
    failed = sum(1 for _, ok, _ in results if ok is False)
    unjudged = sum(1 for _, ok, _ in results if ok is None)
    lines.append("%d checks, %d failed, %d not judged" % (len(results), failed, unjudged))
    print("\n".join(lines))
    return 0 if failed == 0 and unjudged == 0 else 1


def main_slice_mailview(bundle):
    answer, _out, _mail, part_log = mv_launch(bundle, "--slice-mailview", 3)
    if answer is None:
        return 1
    return mv_report(judge_mailview(answer, {"parts": [list(p) for p in part_log]}, mv_commit_limit()))


# ---- --slice-labels: the four built-in labels on a new real installation (core/crates/engine/tests/engine_builtin_labels_r6.rs reads the store; this reads the app)
LB_IDS = ("needs-reply", "updates", "promotions", "social")
LB_NEEDS_REPLY = "Emails that wait for a reply from me: a question, a request, a decision that someone is waiting for."
LB_ENTRY_RULES = {"needs-reply": 0, "updates": 0, "promotions": 1, "social": 0}     # the entry rules the seed writes (promotions: a mailing list); Needs Reply, Updates and Social are sorted by the AI card
LB_ENTRY_FACTS = {"needs-reply": [], "updates": [], "promotions": ["mailing_list"], "social": []}    # the Entry card's fact cells that are set (store seed: label_meta.rs builtin_seeds); a fact is a cell of the grid, never a text row


def lb_entry_rules(g):
    """The entry rules a label really holds: the set fact cells plus the text rows with a value. The inspector pads an empty Entry card with two blank rows
    (From, Subject: LabelInspectorModel.readDraft), so `summary.rules` and the `.rules` rows count placeholders and never a stored rule. None = not read."""
    e = g.get("entry")
    if not e or e.get("rows") is None:
        return None
    return {"facts": list(e.get("set_facts") or []), "text_rows": [r for r in e["rows"] if r.get("values")]}


def drive_labels():
    """Inside launch-one: a fresh data folder with one account added, then each built-in label opened on the inspector (modify) and read."""
    sys.path.insert(0, os.path.join(ROOT, "journeys"))
    from lib.ctl import Ctl  # noqa: E402
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import realcore_mailfake as fake  # noqa: E402
    c = Ctl().connect()
    INS = "mac-labels.inspector"
    answer = {"driver_errors": [], "labels": {}}
    mv_add_account(c, fake, answer, 0)
    answer["core"] = c.uistate().get("core")
    # the real core serves no `labels/tree` path (only the fixture core does; core/crates/engine/src/rust_core.rs `state` has none): the tree is what the sidebar lists
    answer["tree"] = [{"id": r.get("id"), "name": r.get("raw_name")} for r in c.rows("mac-main.sidebar.labels")]
    for lid in LB_IDS:
        got = {}
        try:
            c.click(INS + ".open.modify." + lid)
            s, secs = mv_poll(lambda: (c.rows(INS) or [{}])[0], lambda r: r.get("open") is True and r.get("label_id") == lid, 10)
            got["summary"] = {k: (s or {}).get(k) for k in ("open", "mode", "label_id", "name", "meaning", "rules", "match")}
            got["rules"] = [{"kind": r.get("kind"), "values": r.get("values")} for r in c.rows(INS + ".rules")]
            # the Entry card's own chain row (it has a "chain" key; the card row of the same region has none): the text rows and the fact cells that are set
            ch = [r for r in c.rows(INS + ".flow") if r.get("chain") == "entry"]
            got["entry"] = {"set_facts": sorted(k for k, v in ((ch[0].get("facts") or {}).items() if ch else []) if v != "any"), "rows": ch[0].get("rows") if ch else None} if ch else None
            empty, _ = mv_poll(lambda: c.rows(INS + ".empty"), lambda r: bool(r), 8)
            got["ai_words"] = (empty or [{}])[0].get("words") if empty else None
            c.click(INS + ".close")
        except Exception as e:      # noqa: BLE001
            answer["driver_errors"].append("%s: %s: %s" % (lid, type(e).__name__, e))
        answer["labels"][lid] = got
    return answer


def judge_labels(answer):
    import json
    out = []
    lab = answer.get("labels") or {}
    out.append(("core-is-rust", answer.get("core") in (None, "rust"), "uistate core %r" % answer.get("core")))
    for lid in LB_IDS:
        g = lab.get(lid) or {}
        s = g.get("summary") or {}
        if not s:
            for n in ("has-a-description", "has-its-entry-rules"):
                out.append(("label-%s-%s" % (lid, n), None, "the inspector did not open on it: %s" % answer.get("driver_errors")))
            continue
        ids = [t.get("id") for t in (answer.get("tree") or [])]
        out.append(("label-%s-is-on-the-tree" % lid, lid in ids, "the sidebar's label rows (ids %r) hold %r: %s" % (ids, lid, lid in ids)))
        out.append(("label-%s-has-a-description" % lid, bool((s.get("meaning") or "").strip()) and (lid != "needs-reply" or s.get("meaning") == LB_NEEDS_REPLY), "inspector meaning %r" % s.get("meaning")))
        held = lb_entry_rules(g)
        out.append(("label-%s-has-its-entry-rules" % lid, None if held is None else (held["facts"] == LB_ENTRY_FACTS[lid] and not held["text_rows"] and len(held["facts"]) == LB_ENTRY_RULES[lid]),
                    "the Entry card holds %r (want facts %r, no text row with a value); the inspector's own row count %r and rows %r count blank placeholders" % (held, LB_ENTRY_FACTS[lid], s.get("rules"), g.get("rules"))
                    if held is not None else "the inspector served no `.flow` entry row: not judged"))
        if LB_ENTRY_RULES[lid] == 0:
            words = g.get("ai_words")
            out.append(("label-%s-ai-card-says-why-it-cannot-sort" % lid, bool(words and words.strip()) if words is not None else None,
                        "the empty-list words %r (no model is set on a new installation)" % words if words is not None else "the inspector served no `.empty` row within the wait: not judged"))
    out.append(("the-driver-reached-every-step", not answer.get("driver_errors"), "driver errors %r" % answer.get("driver_errors")))
    return out


def selftest_labels():
    """The label judge against a model answer and one broken answer per case."""
    import copy
    good = {"core": "rust", "driver_errors": [], "tree": [{"id": i} for i in LB_IDS],
            "labels": {lid: {"summary": {"open": True, "mode": "modify", "label_id": lid, "name": lid, "meaning": LB_NEEDS_REPLY if lid == "needs-reply" else "About %s." % lid, "rules": 2},     # the real inspector counts its two blank placeholder rows (From, Subject)
                             "rules": [{"kind": "from", "values": []}, {"kind": "subject", "values": []}],
                             "entry": {"set_facts": list(LB_ENTRY_FACTS[lid]), "rows": [{"kind": "from", "values": []}, {"kind": "subject", "values": []}]}, "ai_words": "The AI cannot sort yet: no model is set." if LB_ENTRY_RULES[lid] == 0 else None}
                       for lid in LB_IDS}}
    base = judge_labels(good)
    clean = all(ok is True for _, ok, _ in base)
    bad = []
    for lid in LB_IDS:
        for case, edit in (("has-a-description", lambda a, l=lid: a["labels"][l]["summary"].update(meaning="")),
                           ("has-its-entry-rules", lambda a, l=lid: a["labels"][l]["entry"].update(set_facts=[] if LB_ENTRY_FACTS[l] else ["unread"])),
                           ("ai-card-says-why-it-cannot-sort", lambda a, l=lid: a["labels"][l].update(ai_words="")), ("is-on-the-tree", lambda a, l=lid: a.update(tree=[{"id": i} for i in LB_IDS if i != l]))):
            name = "label-%s-%s" % (lid, case)
            if name not in {n for n, _, _ in base}:
                continue
            a = copy.deepcopy(good)
            edit(a)
            red = {n for n, ok, _ in judge_labels(a) if ok is not True}
            if red != {name}:
                bad.append("%s -> %s" % (name, sorted(red)))
    more = copy.deepcopy(good)     # a stored text rule on Updates (a row with a value) is a rule the seed does not write
    more["labels"]["updates"]["entry"]["rows"][0]["values"] = ["news"]
    more_red = {n for n, ok, _ in judge_labels(more) if ok is not True} == {"label-updates-has-its-entry-rules"}
    unread = copy.deepcopy(good)   # the Entry card not served at all is NOT JUDGED (None), never a pass
    unread["labels"]["social"]["entry"] = None
    unread_none = [ok for n, ok, _ in judge_labels(unread) if n == "label-social-has-its-entry-rules"] == [None]
    wrong = copy.deepcopy(good)
    wrong["labels"]["needs-reply"]["summary"]["meaning"] = "something else"
    wrong_red = {n for n, ok, _ in judge_labels(wrong) if ok is not True} == {"label-needs-reply-has-a-description"}
    print("selftest slice labels: model %s, broken answers %s, a changed Needs Reply sentence %s, a stored text rule %s, an unread Entry card %s" % (
        "all %d cases pass" % len(base) if clean else "FAILS", "each red on its own case" if not bad else "WRONG: %s" % bad, "red" if wrong_red else "WRONG",
        "red" if more_red else "WRONG", "not judged" if unread_none else "WRONG"))
    return clean and not bad and wrong_red and more_red and unread_none


def main_slice_labels(bundle):
    answer, _out, _mail, _log = mv_launch(bundle, "--slice-labels", 0)
    if answer is None:
        return 1
    return mv_report(judge_labels(answer))


# ---------------------------------------------------------------- --slice-sources (lane sources-slice, item 021cf2d6): the account card on Settings, Replies, Sources, REAL engine
# The local server lists INBOX, Sent and Archive (Archive set on this process's fake only). The app registers each folder of the account (store replies.rs reply_register_folders:
# Inbox and Sent on, the rest off); the pane's rows are RepliesRows.sources (RepliesRows.swift:45-52); the footnote `no-account` only exists when the account list is empty (SourcesPane.swift:21).
SRC_P = "mac-settings-replies"
SRC_EXTRA_BOXES = {"Archive": " \\Archive"}
SRC_SERVER_FOLDERS = ("inbox", "sent", "archive")           # the names the pane shows for the server's INBOX, Sent and Archive
SRC_ROW_KEYS = {"ax", "kind", "line", "open", "name", "enabled", "count", "progress", "gone", "wish", "path"}
SRC_WAIT_S = 60


def drive_sources():
    """Inside launch-one: the account added as slice 2 does, Settings, Replies, Sources opened, the rows read when the folders are listed (or the wait ends)."""
    sys.path.insert(0, os.path.join(ROOT, "journeys"))
    from lib.ctl import Ctl  # noqa: E402
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import realcore_mailfake as fake  # noqa: E402
    c = Ctl().connect()
    answer = {"driver_errors": []}
    mv_add_account(c, fake, answer, 0)
    answer["core"] = c.uistate().get("core")
    answer.pop("rows", None)
    try:
        try:
            c.click("mac-settings-window.open")
        except Exception as e:      # noqa: BLE001  the pane may be reachable already
            answer["open_note"] = "%s: %s" % (type(e).__name__, e)
        c.click(SRC_P + ".page.sources")
        rows, secs = mv_poll(lambda: c.rows(SRC_P + ".sources"), lambda r: sum(1 for x in r if x.get("kind") == "mail_folder") >= len(SRC_SERVER_FOLDERS), SRC_WAIT_S)
        answer["seconds"] = secs
        # only the app's own row fields are kept; a key the pane does not define goes to `extra_keys` (by name), its value is never copied
        answer["sources"] = [{k: v for k, v in r.items() if k in SRC_ROW_KEYS} for r in (rows or [])]
        answer["extra_keys"] = sorted({k for r in (rows or []) for k in r if k not in SRC_ROW_KEYS})
        # the footnote is a view, not a region of rows (SourcesPane.swift:21): `rows` answers "no rows at" for it whether it is drawn or not
        # (main193, 15 Nov). Its frame is asked instead; the same question on the account card, which is drawn, shows the question is answered
        # on this pane, so "nothing on screen carries that id" for the footnote means it is absent.
        def drawn(ax):
            try:
                return bool((c.measure(ax) or {}).get("frame"))
            except Exception as e:      # noqa: BLE001
                if "no measurement at" in str(e):
                    return False
                raise
        card = next((r.get("ax") for r in (rows or []) if r.get("kind") == "account"), None)
        answer["card_drawn"] = drawn(card) if card else None
        answer["no_account_rows"] = 1 if drawn(SRC_P + ".no-account") else 0
        if answer["card_drawn"] is not True:
            raise RuntimeError("the account card %r answers no frame: the footnote's absence cannot be read" % card)
        answer["no_account_read"] = True
    except Exception as e:      # noqa: BLE001
        answer["driver_errors"].append("%s: %s" % (type(e).__name__, e))
    return answer


def judge_sources(answer):
    out = []
    errors = answer.get("driver_errors") or []
    out.append(("account-added", answer.get("stage") in ("added", "summary"), "stage %r, core %r" % (answer.get("stage"), answer.get("core"))))
    rows = answer.get("sources")
    if rows is None:
        why = "the Sources rows were not read: %s" % errors
        return out + [(n, None, why) for n in ("one-row-of-kind-account", "mail-folders-enabled-are-inbox-and-sent-only", "archive-is-listed-and-off", "no-account-footnote-absent", "rows-hold-no-mail-text")] \
            + [("the-driver-reached-every-step", False, "driver errors %r" % errors)]
    accounts = [r for r in rows if r.get("kind") == "account"]
    folders = [r for r in rows if r.get("kind") == "mail_folder"]
    out.append(("one-row-of-kind-account", len(accounts) == 1, "%d row(s) of kind account (kinds read: %s)" % (len(accounts), sorted({str(r.get("kind")) for r in rows}))))
    on = sorted(str(r.get("name")) for r in folders if r.get("enabled") is True)
    out.append(("mail-folders-enabled-are-inbox-and-sent-only", None if not folders else sorted(n.lower() for n in on) == ["inbox", "sent"],
                "%d mail_folder rows; enabled: %s; off: %s" % (len(folders), on, sorted(str(r.get("name")) for r in folders if r.get("enabled") is not True)) if folders else "no mail_folder row read within %ss" % answer.get("seconds")))
    arch = [r for r in folders if str(r.get("name")).lower() == "archive"]
    out.append(("archive-is-listed-and-off", None if not folders else (len(arch) == 1 and arch[0].get("enabled") is False), "%d Archive row(s), enabled %r" % (len(arch), [r.get("enabled") for r in arch])))
    if not answer.get("no_account_read"):
        out.append(("no-account-footnote-absent", None, "the row mac-settings-replies.no-account was not read: %s" % errors))
    else:
        out.append(("no-account-footnote-absent", answer.get("no_account_rows") == 0, "%r row(s) of mac-settings-replies.no-account" % answer.get("no_account_rows")))
    names = [str(r.get("name")).lower() for r in folders]
    clean = not answer.get("extra_keys") and all(n in SRC_SERVER_FOLDERS for n in names) and all(isinstance(r.get("enabled"), bool) for r in folders) \
        and all(r.get("kind") in ("account", "mail_folder", "local_folder", "calendar") for r in rows)
    out.append(("rows-hold-no-mail-text", clean, "kinds %s, folder names %s (the server's: %s), keys outside the pane's own list: %s" % (
        sorted({str(r.get("kind")) for r in rows}), names, list(SRC_SERVER_FOLDERS), answer.get("extra_keys") or "none")))
    out.append(("the-driver-reached-every-step", not errors, "driver errors %r" % errors))
    return out


def src_good():
    f = lambda i, name, on: {"ax": "mac-settings-replies.sources.account.acc-1.folder.%d" % i, "kind": "mail_folder", "name": name, "enabled": on, "count": 0, "progress": "", "gone": False, "line": "", "wish": None}
    return {"stage": "added", "core": "rust", "driver_errors": [], "seconds": 3.2, "extra_keys": [], "no_account_rows": 0, "no_account_read": True,
            "sources": [{"ax": "mac-settings-replies.sources.account.acc-1", "kind": "account", "line": "2 of 3 folders", "open": True}, f(0, "Inbox", True), f(1, "Sent", True), f(2, "Archive", False)]}


def selftest_sources():
    """The judge against a clean stand-in answer (green) and broken copies (each red on its own case, named), and a pane that was not read (NOT JUDGED)."""
    import copy
    good = src_good()
    base = judge_sources(good)
    clean = all(ok is True for _, ok, _ in base)

    def no_account(a): a["sources"] = [r for r in a["sources"] if r["kind"] != "account"]
    def two_accounts(a): a["sources"].append(dict(a["sources"][0], ax="mac-settings-replies.sources.account.acc-2"))
    def archive_on(a): a["sources"][3]["enabled"] = True
    def sent_off(a): a["sources"][2]["enabled"] = False
    def footnote(a): a.update(no_account_rows=1)
    def mail_text(a): a["sources"][1]["subject"] = "Quote for the first quarter"; a.update(extra_keys=["subject"])
    def stranger(a): a["sources"][3]["name"] = "Quote for the first quarter"
    broken = {"no account row": (no_account, {"one-row-of-kind-account"}), "two account rows": (two_accounts, {"one-row-of-kind-account"}),
              "a third folder switched on": (archive_on, {"mail-folders-enabled-are-inbox-and-sent-only", "archive-is-listed-and-off"}),
              "Sent switched off": (sent_off, {"mail-folders-enabled-are-inbox-and-sent-only"}),
              "the no-account footnote still there": (footnote, {"no-account-footnote-absent"}), "a mail text field in a row": (mail_text, {"rows-hold-no-mail-text"}),
              "a folder named like a mail subject": (stranger, {"rows-hold-no-mail-text", "archive-is-listed-and-off"})}
    verdicts, bad = [], []
    for name, (edit, want) in broken.items():
        a = copy.deepcopy(good)
        edit(a)
        red = {n for n, ok, _ in judge_sources(a) if ok is not True}
        verdicts.append("%s: %s" % (name, "red on " + ", ".join(sorted(red)) if red == want else "WRONG (red on %s, wanted %s)" % (sorted(red), sorted(want))))
        if red != want:
            bad.append(name)
    unread = {"stage": "added", "core": "rust", "driver_errors": ["Unreadable: no such door"]}
    ur = {n: ok for n, ok, _ in judge_sources(unread)}
    ok_unread = ur["one-row-of-kind-account"] is None and ur["no-account-footnote-absent"] is None and ur["the-driver-reached-every-step"] is False
    nofolders = copy.deepcopy(good); nofolders["sources"] = nofolders["sources"][:1]
    nf = {n: ok for n, ok, _ in judge_sources(nofolders)}
    ok_nf = nf["mail-folders-enabled-are-inbox-and-sent-only"] is None and nf["one-row-of-kind-account"] is True
    for v in verdicts:
        print("selftest slice sources: broken copy %s" % v)
    print("selftest slice sources: clean stand-in %s (%d cases green), %d broken copies %s, a pane not read %s, no folder row %s" % (
        "green" if clean else "FAILS", len(base), len(broken), "each red on its own cases" if not bad else "WRONG: %s" % bad,
        "NOT JUDGED" if ok_unread else "WRONG", "NOT JUDGED" if ok_nf else "WRONG"))
    return clean and not bad and ok_unread and ok_nf


def main_slice_sources(bundle):
    answer, _out, _mail, _log = mv_launch(bundle, "--slice-sources", 0, extra_boxes=SRC_EXTRA_BOXES)
    if answer is None:
        return 1
    return mv_report(judge_sources(answer))


# ---------------------------------------------------------------- --slice-vip (lane sources-slice, item cd640430): mark a sender as VIP through the palette, REAL engine
# The app side (docs/lanes/vip-mark.md): the palette's one line `command:Mark as VIP` (after a mark, on a mail of that sender, `command:Remove from VIPs`) in rows `mac-palette.results`;
# the engine moves the sender's inbox mail to the VIPs tab (EditVips), so it leaves Primary; the sidebar row `mac-main.sidebar.mailbox.vips` carries `counts().mailboxes["vips"]`
# (unread inbox mail of VIP senders; the row draws no count for zero). The palette key and its results rows are the ones checks/parity/par-palettemac.py `c_vip_mark` drives.
VIP_MARK_ID, VIP_REMOVE_ID = "command:Mark as VIP", "command:Remove from VIPs"
VIP_WAIT_S = 20
VIP_CASES = ("vip-count-follows-the-mark", "vip-mail-leaves-primary", "palette-offers-remove-after-the-mark", "vip-count-is-back-after-remove")


def vip_count(c):
    """The VIPs row's count on the sidebar (mailboxes or views region): its figure, 0 when the row draws none, None when no row is served."""
    for region in ("mac-main.sidebar.mailboxes", "mac-main.sidebar.views"):
        for r in c.rows(region):
            if str(r.get("ax", "")).endswith(".mailbox.vips"):
                v = r.get("count")
                return int(v) if isinstance(v, (int, float)) and not isinstance(v, bool) else (int(v) if isinstance(v, str) and v.isdigit() else 0)
    return None


def vip_palette(c, query):
    """Opens the palette on `query` and returns its result rows as (ax, id) pairs. Nothing but door and line ids."""
    c.key("⌘P")
    c.type(query)
    time.sleep(0.4)
    return [(r.get("ax"), r.get("id")) for r in c.rows("mac-palette.results")]


def vip_close(c):
    try:
        if c.uistate().get("surface") == "mac-palette":
            c.key("Escape")
    except Exception:       # noqa: BLE001
        pass


def drive_vip():
    """Inside launch-one: the account added as slice 2 does, then the mark and the remove through the palette; every read is a count, an id or a door."""
    sys.path.insert(0, os.path.join(ROOT, "journeys"))
    from lib.ctl import Ctl  # noqa: E402
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import realcore_mailfake as fake  # noqa: E402
    c = Ctl().connect()
    answer = {"driver_errors": [], "mark_served": None}
    mv_add_account(c, fake, answer, 3)
    answer["core"] = c.uistate().get("core")
    answer["list_rows"] = len(answer.pop("rows", []))
    try:
        answer["before"] = vip_count(c) or 0
        answer["vips_row_before"] = vip_count(c) is not None
        # one unread inbox mail: the focused one, else the next unread row down (J), at most three moves
        pick = None
        for _ in range(4):
            rows = c.rows("mac-main.list")
            focus = c.uistate().get("focus_message")
            row = next((r for r in rows if r.get("id") == focus), None)
            if row and row.get("unread") and row.get("sender_address"):
                pick = row
                break
            c.key("J"); time.sleep(0.4)
        if pick is None:
            raise RuntimeError("no unread focused mail within four moves")
        sender = pick["sender_address"]
        answer["focus"] = pick["id"]
        answer["unread_of_sender"] = sum(1 for r in c.rows("mac-main.list") if r.get("sender_address") == sender and r.get("unread"))
        answer["rows_of_sender_before"] = sum(1 for r in c.rows("mac-main.list") if r.get("sender_address") == sender)
        answer["tab"] = c.uistate().get("tab")
        lines = vip_palette(c, "mark as vip")
        answer["mark_served"] = any(i == VIP_MARK_ID for _, i in lines)
        if not answer["mark_served"]:
            vip_close(c)
            return answer
        c.click(next(a for a, i in lines if i == VIP_MARK_ID))
        want = answer["before"] + answer["unread_of_sender"]
        got, secs = mv_poll(lambda: vip_count(c) or 0, lambda v: v == want, VIP_WAIT_S)
        answer["after_mark"], answer["after_mark_seconds"] = got, secs
        left, _ = mv_poll(lambda: sum(1 for r in c.rows("mac-main.list") if r.get("sender_address") == sender), lambda n: n == 0, VIP_WAIT_S)
        answer["primary_left"] = left
        # the VIPs view: a mail of that sender, then the palette on "vip": which of the two lines it offers
        c.click("mac-main.sidebar.mailbox.vips")
        vrows, _ = mv_poll(lambda: [r for r in c.rows("mac-main.list") if r.get("sender_address") == sender], lambda r: bool(r), VIP_WAIT_S)
        answer["vips_view_rows"] = len(vrows or [])
        if vrows:
            for _ in range(5):                  # focus a row of that sender (the first row is focused on opening; K goes up when it is not)
                if c.uistate().get("focus_message") in {r.get("id") for r in vrows}:
                    break
                c.key("K"); time.sleep(0.3)
            answer["focus_in_vips_view"] = c.uistate().get("focus_message") in {r.get("id") for r in vrows}
        lines = vip_palette(c, "vip")
        ids = [i for _, i in lines]
        answer["remove_listed"], answer["mark_listed_after"] = VIP_REMOVE_ID in ids, VIP_MARK_ID in ids
        if answer["remove_listed"]:
            c.click(next(a for a, i in lines if i == VIP_REMOVE_ID))
            back, secs = mv_poll(lambda: vip_count(c) or 0, lambda v: v == answer["before"], VIP_WAIT_S)
            answer["after_remove"], answer["after_remove_seconds"] = back, secs
        else:
            vip_close(c)
    except Exception as e:      # noqa: BLE001
        answer["driver_errors"].append("%s: %s" % (type(e).__name__, e))
        vip_close(c)
    return answer


def judge_vip(answer):
    out = [("account-added", answer.get("stage") in ("added", "summary"), "stage %r, core %r, %r list rows" % (answer.get("stage"), answer.get("core"), answer.get("list_rows")))]
    errors = answer.get("driver_errors") or []
    nj = lambda why: [(n, None, why) for n in VIP_CASES]
    if answer.get("mark_served") is False:
        return out + nj("the palette lists no `%s` line for the focused mail: a build without the command" % VIP_MARK_ID) + [("the-driver-reached-every-step", not errors, "driver errors %r" % errors)]
    if answer.get("mark_served") is None:
        return out + nj("the mark line was never read: %s" % errors) + [("the-driver-reached-every-step", False, "driver errors %r" % errors)]
    before, n = answer.get("before"), answer.get("unread_of_sender")
    after = answer.get("after_mark")
    if n and after is not None:
        out.append(("vip-count-follows-the-mark", after == before + n, "VIPs count %r before, %r after the mark (want %r: %r unread inbox mail(s) of that sender), read in %ss" % (before, after, before + n, n, answer.get("after_mark_seconds"))))
    else:
        out.append(("vip-count-follows-the-mark", None, "no unread mail of the sender (%r) or the count after the mark was not read: %s" % (n, errors)))
    left = answer.get("primary_left")
    if left is None or not answer.get("rows_of_sender_before"):
        out.append(("vip-mail-leaves-primary", None, "the list rows of that sender were not read (before %r, after %r): %s" % (answer.get("rows_of_sender_before"), left, errors)))
    else:
        out.append(("vip-mail-leaves-primary", left == 0, "%r row(s) of that sender in the list before the mark, %r after (tab %r)" % (answer.get("rows_of_sender_before"), left, answer.get("tab"))))
    if answer.get("remove_listed") is None or not answer.get("vips_view_rows"):
        out.append(("palette-offers-remove-after-the-mark", None, "no mail of that sender in the VIPs view (%r row(s)), so the palette was not read there: %s" % (answer.get("vips_view_rows"), errors)))
    else:
        out.append(("palette-offers-remove-after-the-mark", bool(answer.get("remove_listed")) and not answer.get("mark_listed_after") and answer.get("focus_in_vips_view") is not False,
                    "on the VIPs view (%r row(s) of that sender, focus on one: %r) the palette lists `%s`: %r, `%s`: %r" % (
                        answer.get("vips_view_rows"), answer.get("focus_in_vips_view"), VIP_REMOVE_ID, answer.get("remove_listed"), VIP_MARK_ID, answer.get("mark_listed_after"))))
    back = answer.get("after_remove")
    if back is None:
        out.append(("vip-count-is-back-after-remove", None, "Remove was not run (the palette listed no `%s` line): %s" % (VIP_REMOVE_ID, errors)))
    else:
        out.append(("vip-count-is-back-after-remove", back == before, "VIPs count %r before the mark, %r after Remove (read in %ss)" % (before, back, answer.get("after_remove_seconds"))))
    out.append(("the-driver-reached-every-step", not errors, "driver errors %r" % errors))
    return out


def vip_good():
    return {"stage": "added", "core": "rust", "driver_errors": [], "list_rows": 3, "mark_served": True, "before": 0, "vips_row_before": False, "unread_of_sender": 1, "rows_of_sender_before": 1,
            "tab": "primary", "after_mark": 1, "after_mark_seconds": 0.6, "primary_left": 0, "vips_view_rows": 1, "focus_in_vips_view": True, "remove_listed": True, "mark_listed_after": False,
            "after_remove": 0, "after_remove_seconds": 0.4}


def selftest_vip():
    """The judge against a clean stand-in answer (green) and four broken copies, each red on its own case only; a build with no mark line, a sender with no unread mail and a list not read are NOT JUDGED."""
    import copy
    good = vip_good()
    base = judge_vip(good)
    clean = all(ok is True for _, ok, _ in base) and [n for n, _, _ in base if n in VIP_CASES] == list(VIP_CASES)
    def never_moves(a): a.update(after_mark=a["before"])
    def stays(a): a.update(primary_left=1)
    def keeps_mark(a): a.update(mark_listed_after=True, remove_listed=False, after_remove=None)
    def not_back(a): a.update(after_remove=a["before"] + 1)
    broken = {"the count never moves": (never_moves, {"vip-count-follows-the-mark"}), "the mail stays in Primary": (stays, {"vip-mail-leaves-primary"}),
              "the palette keeps offering Mark": (keeps_mark, {"palette-offers-remove-after-the-mark"}), "the count is not back after Remove": (not_back, {"vip-count-is-back-after-remove"})}
    verdicts, bad = [], []
    for name, (edit, want) in broken.items():
        a = copy.deepcopy(good)
        edit(a)
        res = judge_vip(a)
        red = {n for n, ok, _ in res if ok is False}
        unj = {n for n, ok, _ in res if ok is None}
        nj_want = {"vip-count-is-back-after-remove"} if name == "the palette keeps offering Mark" else set()     # no Remove line: Remove is not run, its count case is NOT JUDGED, never a pass
        ok_shape = red == want and unj == nj_want
        verdicts.append("%s: %s" % (name, ("red on " + ", ".join(sorted(red)) + ("; NOT JUDGED: " + ", ".join(sorted(unj)) if unj else "")) if ok_shape else
                                    "WRONG (red %s, not judged %s)" % (sorted(red), sorted(unj))))
        if not ok_shape:
            bad.append(name)
    old = {"stage": "added", "core": "rust", "driver_errors": [], "mark_served": False}
    ov = {n: ok for n, ok, _ in judge_vip(old)}
    ok_old = all(ov[n] is None for n in VIP_CASES)
    noread = {"stage": "added", "core": "rust", "driver_errors": ["RuntimeError: no unread focused mail within four moves"], "mark_served": None}
    nv = {n: ok for n, ok, _ in judge_vip(noread)}
    ok_noread = all(nv[n] is None for n in VIP_CASES) and nv["the-driver-reached-every-step"] is False
    none_unread = copy.deepcopy(good); none_unread.update(unread_of_sender=0)
    ok_none = {n: ok for n, ok, _ in judge_vip(none_unread)}["vip-count-follows-the-mark"] is None
    for v in verdicts:
        print("selftest slice vip: broken copy %s" % v)
    print("selftest slice vip: clean stand-in %s (%d cases), %d broken copies %s, a build with no mark line %s, a list not read %s, no unread mail of the sender %s" % (
        "green" if clean else "FAILS", len(base), len(broken), "each red on its own case" if not bad else "WRONG: %s" % bad,
        "NOT JUDGED" if ok_old else "WRONG", "NOT JUDGED" if ok_noread else "WRONG", "NOT JUDGED" if ok_none else "WRONG"))
    return clean and not bad and ok_old and ok_noread and ok_none


def main_slice_vip(bundle):
    answer, _out, _mail, _log = mv_launch(bundle, "--slice-vip", 0, start_inbox=3)
    if answer is None:
        return 1
    return mv_report(judge_vip(answer))


# ---------------------------------------------------------------- slice moved (gap-moved-slice, SPEC A144): a reply to a mail the server moved away
# The server moves a mail out of INBOX by its own hand. Case 1: the sync (a server with no change list) takes the mail out of the app's inbox.
# Case 2: a reply to a mail the app still shows but the server no longer holds in INBOX is queued from the headers the store kept (In-Reply-To
# and References), with no header fetch for that UID after the move. The reply targets mail 1 (moved quietly, the app not told yet) and not mail 2:
# once the sync has told the app, the store holds nothing of mail 2 to reply from (sync.rs deletes the vanished row), so only a mail the app still
# shows can be replied to; the two cases therefore move two different mails. The check parses headers and never prints a mail's text.

MOVED_MUTANT_NAME = "reply-ignores-store"
MOVED_CASE_LEAVES = "a-mail-moved-away-on-the-server-leaves-the-apps-inbox"
MOVED_CASE_REPLY = "a-reply-to-a-stored-mail-whose-server-copy-moved-is-queued-with-in-reply-to-and-references"
MOVED_CASES = (MOVED_CASE_LEAVES, MOVED_CASE_REPLY)
REPLY_MOVED = "Noted, thank you for the plan."
MOVED_SETTLE_S = 60


def uid_in_set(spec, uid):
    """Whether the IMAP set `spec` ("1", "1:3,7", "2:*") holds `uid`."""
    for part in spec.split(","):
        lo, _, hi = part.partition(":")
        try:
            a = int(lo)
            b = int(hi) if hi and hi != "*" else (10 ** 12 if hi == "*" else a)
        except ValueError:
            continue
        if min(a, b) <= uid <= max(a, b):
            return True
    return False


def header_fetches(log, uid, start, end):
    """The FETCH commands in log[start:end] that ask for a message's HEADER and name `uid` (the fake logs "<CMD> <args>", FETCH after UID or alone)."""
    found = []
    for line in log[start:end]:
        m = re.match(r"(?:UID )?FETCH (\S+) (.*)$", line)
        if m and "HEADER" in m.group(2).upper() and uid_in_set(m.group(1), uid):
            found.append(line)
    return found


def smtp_threading(raw):
    """(In-Reply-To values, References tokens) of one received message; only the two headers are read, nothing else is kept."""
    import email
    msg = email.message_from_string(raw)
    irt = [" ".join(str(v).split()) for v in msg.get_all("In-Reply-To") or []]
    refs = [t for v in msg.get_all("References") or [] for t in str(v).split()]
    return irt, refs


def judge_moved(answer, server):
    """`answer` is what the app showed (inside launch-one); `server` is what the local IMAP and SMTP sides saw (this process)."""
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import realcore_mailfake as fake
    errors = answer.get("driver_errors") or []
    away = {a[0]: a for a in server.get("away") or []}
    before, after = answer.get("rows_before") or [], answer.get("rows_after_move2")
    subj2 = "Check message 2"
    # case 1
    if len(before) != 3 or after is None or 2 not in away:
        left = (MOVED_CASE_LEAVES, None, "not reached: rows before %r, rows after the move %r, server moves %r, driver %r" % (before, after, sorted(away), errors))
    else:
        left = (MOVED_CASE_LEAVES, subj2 in before and subj2 not in after and len(after) == len(before) - 1,
                "the server moved UID 2 to %s; rows before %r, after %r" % (away[2][1], before, after))
    # case 2
    mid, chain = fake.message_id(1), fake.ref_chain(1)
    stored = answer.get("stored_before_move") or {}
    smtp, log = server.get("smtp") or [], server.get("log") or []
    if 1 not in away or not answer.get("reply_sent") or not stored:
        why = []
        if not stored:
            why.append("the stored headers of mail 1 were never read (answered %r after %ss)" % (answer.get("stored_last"), MOVED_SETTLE_S))
        if 1 not in away:
            why.append("the server never moved UID 1")
        if not answer.get("reply_sent"):
            why.append("the reply was never sent from the quick reply")
        reply = (MOVED_CASE_REPLY, None, "not reached: %s; driver %r" % ("; ".join(why), errors))
    else:
        kept = [m for m in smtp if REPLY_MOVED in m]
        irt, refs = smtp_threading(kept[0]) if len(kept) == 1 else ([], [])
        move_mark, end_mark = away[1][3], server.get("wake_mark")
        fetched = header_fetches(log, 1, move_mark, end_mark if end_mark is not None else len(log))
        stored_ok = stored.get("message_id") == mid and stored.get("references") == chain
        ok = (len(smtp) == 1 and len(kept) == 1 and irt == [mid] and refs[-1:] == [mid] and refs[:-1] == chain and not fetched and stored_ok)
        reply = (MOVED_CASE_REPLY, ok, "%d SMTP messages, %d with the reply; In-Reply-To %r (want %r); References %r (want %r then %r); header fetches for UID 1 after the move: %d; "
                 "stored headers as the server holds them: %s; the app said %r" % (len(smtp), len(kept), irt, mid, refs, chain, mid, len(fetched), stored_ok, answer.get("retry_words")))
    return [left, reply]


def drive_moved():
    """Inside launch-one: add the local account, read the stored headers, ask the outer side to move mail 2 (loud) and mail 1 (quiet: the app is not told),
    reply to mail 1 through the quick reply, then ask it to tell the app."""
    import json
    sys.path.insert(0, os.path.join(ROOT, "journeys"))
    from lib.ctl import Ctl  # noqa: E402
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import realcore_mailfake as fake  # noqa: E402
    ctl_dir = sys.argv[sys.argv.index("--ctl") + 1]
    c = Ctl().connect()
    c.click(A + ".open")
    c.click(A + ".address"); c.type(CHECK_ADDRESS)
    c.click(A + ".password"); c.type(fake.PASSWORD)
    c.click(A + ".continue")
    answer = {"driver_errors": []}
    errors = answer["driver_errors"]
    t0 = time.time()
    while time.time() - t0 < DETECT_WAIT_S:
        st = c.rows(A + ".state")
        if st and st[0].get("stage") not in ("detecting", "entry"):
            break
        time.sleep(0.3)
    c.click(A + ".add")
    stage = None
    t0 = time.time()
    while time.time() - t0 < DETECT_WAIT_S:
        st = c.rows(A + ".state"); stage = st[0].get("stage") if st else None
        if stage in ("added", "summary"):
            break
        time.sleep(0.3)
    answer["stage"] = stage
    try:
        c.click(A + ".done")
    except Exception as e:      # noqa: BLE001
        errors.append("done: %s" % e)
    for _ in range(10):         # the first-start flow swallows the mail keys until it ends (drive5)
        try:
            st = c.rows("mac-first-start.state")
            if not (st and st[0].get("active")):
                break
            c.key("Enter")
        except Exception as e:      # noqa: BLE001
            errors.append("first start: %s" % e); break
        time.sleep(0.5)
    time.sleep(1.5)

    def attempt(label, fn):
        try:
            return fn()
        except Exception as e:      # noqa: BLE001
            errors.append("%s: %s" % (label, e)); return None

    def listed():
        return attempt("rows list", lambda: c.rows("mac-main.list")) or []

    def wait_rows(ax, seconds=6.0):
        t = time.time()
        while time.time() - t < seconds:
            try:
                r = c.rows(ax)
            except Exception:      # noqa: BLE001
                r = []
            if r:
                return r
            time.sleep(0.25)
        return []

    def ask(name, wait_s=30):
        """A request to the outer side (it owns the fake): a file in the control folder, answered by `<name>.done`."""
        open(os.path.join(ctl_dir, name), "w").close()
        t = time.time()
        while time.time() - t < wait_s:
            try:
                return json.load(open(os.path.join(ctl_dir, name + ".done")))
            except (OSError, ValueError):
                time.sleep(0.2)
        errors.append("the outer side did not answer %s in %ds" % (name, wait_s))
        return None

    rows = []
    t0 = time.time()
    while time.time() - t0 < ACT_WAIT_S and len(rows) < 3:
        rows = listed(); time.sleep(0.5)
    answer["rows_before"] = [r.get("subject") for r in rows]
    if len(rows) < 3:
        errors.append("the list never held three rows: %r" % answer["rows_before"])
        return answer
    by_subject = {r.get("subject"): r.get("id") for r in rows}
    # 1. the headers the store holds for mail 1 (the fill-in pass or the arrival wrote them): read through the engine's own state
    def stored_headers(n):
        v = cv(c, "message/%s/thread_headers" % by_subject.get("Check message %d" % n))
        return v if isinstance(v, dict) and v.get("message_id") else None
    t0 = time.time()
    while time.time() - t0 < MOVED_SETTLE_S:
        got = [stored_headers(n) for n in (1, 2, 3)]
        answer["stored_last"] = got
        if all(got):
            break
        time.sleep(0.5)
    answer["stored_before_move"] = stored_headers(1)
    # 2. the server moves mail 2 away; the app's sync is told at once (IDLE)
    answer["move2"] = ask("move-2")
    t0 = time.time()
    while time.time() - t0 < MOVED_SETTLE_S:
        now = [r.get("subject") for r in listed()]
        if "Check message 2" not in now:
            break
        time.sleep(0.5)
    answer["rows_after_move2"] = [r.get("subject") for r in listed()]
    # 3. mail 1 is opened (the reader holds it), then the server moves it WITHOUT telling the app
    opened = False
    for _ in range(4):
        attempt("Enter", lambda: c.key("Enter"))
        if "Check message 1" in json.dumps(wait_rows("mac-main.reader.subject", 3.0), ensure_ascii=False):
            opened = True
            break
        attempt("Escape", lambda: c.key("Escape")); attempt("J", lambda: c.key("J")); time.sleep(0.4)
    if not opened:
        errors.append("the thread of mail 1 never opened")
        return answer
    time.sleep(2)               # its body is fetched now, before the move
    answer["move1"] = ask("move-1-quiet")
    attempt("R", lambda: c.key("R"))
    field = wait_rows(FIELD)
    if not field:
        errors.append("the quick reply never opened after R (no %s row)" % FIELD)
        return answer
    if field[0].get("focused") is False:
        attempt("field click", lambda: c.click(FIELD))
    attempt("type", lambda: c.type(REPLY_MOVED))
    t = time.time()
    while time.time() - t < 3 and REPLY_MOVED not in ((attempt("field text", lambda: c.rows(FIELD)) or [{}])[0].get("text") or ""):
        time.sleep(0.2)
    if REPLY_MOVED not in ((attempt("field text", lambda: c.rows(FIELD)) or [{}])[0].get("text") or ""):
        errors.append("the quick reply does not hold the typed words")
        return answer
    answer["reply_sent"] = attempt("⌘Enter", lambda: (c.key("⌘Enter"), True)[1]) is True
    time.sleep(2)
    bar = wait_rows("mac-main.undo.bar", 2.0)
    answer["undo_bar"] = bool(bar and bar[0].get("message") == "Sending")
    retry = attempt("rows retry", lambda: c.rows("mac-main.reply.retry")) or []
    answer["retry_words"] = retry[0].get("words") if retry else None
    time.sleep(undo_window_s() + 6)      # the window ends: the outbox timer sends it
    # 4. the app is told of the second move; informational only
    answer["wake"] = ask("wake")
    time.sleep(4)
    answer["rows_end"] = [r.get("subject") for r in listed()]
    answer["core"] = attempt("uistate", lambda: c.uistate().get("core"))
    return answer


def serve_moved_requests(mail, smtp, ctl_dir, stop):
    """The outer side: the fake lives in this process, the drive asks for what the server does by its own hand (files in `ctl_dir`)."""
    import json
    import realcore_mailfake as fake
    done = set()
    while not stop.is_set():
        for name in sorted(os.listdir(ctl_dir)):
            if name.endswith(".done") or name.endswith(".tmp") or name in done:
                continue
            done.add(name)
            try:
                if name.startswith("move-"):
                    uid = int(name.split("-")[1])
                    new = fake.move_away(mail, uid, "Archive", quiet=name.endswith("-quiet"))
                    reply = {"uid": uid, "new_uid": new, "mark": len(mail.log)}
                elif name == "wake":
                    with mail.lock:
                        reply = {"mark": len(mail.log), "smtp": len(smtp.messages)}
                    fake.announce(mail)
                else:
                    reply = {"error": "unknown request"}
            except Exception as e:      # noqa: BLE001  the drive reads the words
                reply = {"error": str(e)}
            tmp = os.path.join(ctl_dir, name + ".tmp")
            json.dump(reply, open(tmp, "w"))
            os.replace(tmp, os.path.join(ctl_dir, name + ".done"))
        time.sleep(0.2)


def main_slice_moved(bundle, mutant):
    """The local servers live in THIS process (threads) for the whole launch; the app reaches them on 127.0.0.1 only."""
    import json
    import tempfile
    import threading
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import realcore_mailfake as fake
    try:
        mail, imap, smtp = fake.start(0, 0, inbox=3, act=True, refs=True, plain=True)
    except OSError as e:
        print("the local servers did not start: %s" % e)
        print("1 checks, 0 failed, 1 not judged"); return 1
    ctl_dir = tempfile.mkdtemp(prefix="whoosh-moved-ctl-")
    stop = threading.Event()
    threading.Thread(target=serve_moved_requests, args=(mail, smtp, ctl_dir, stop), daemon=True).start()
    ports = "%d:%d" % (imap.server_address[1], smtp.server_address[1])
    extra = ["WHOOSH_CORE_PLAIN_LOOPBACK=1", "WHOOSH_CORE_CHECK_SERVERS=" + ports] + (["WHOOSH_CORE_MUTANT=" + MOVED_MUTANT_NAME] if mutant else [])
    env = dict(os.environ, WHOOSH_RIG_HIDDEN="1", LAUNCH_ENV=" ".join(["WHOOSH_RIG_HIDDEN=1"] + extra))
    env.pop("WHOOSH_FIXTURE_CORE", None)
    cmd = ["sh", os.path.join(ROOT, "scripts", "launch-one.sh"), bundle, "--", "--rig-hidden",
           "sh", "-c", 'WHOOSH_SOCKET_DIR="$D" python3 "$0" --inner --slice-moved --ctl "$1"', os.path.abspath(__file__), ctl_dir]
    r = launch(cmd, env)
    stop.set()
    with mail.lock:
        wake = None
        try:
            wake = json.load(open(os.path.join(ctl_dir, "wake.done")))
        except (OSError, ValueError):
            pass
        server = {"smtp": [m.decode(errors="replace") for m in smtp.messages], "log": list(mail.log), "away": [list(a) for a in mail.away],
                  "wake_mark": (wake or {}).get("mark"), "change_lists": list(mail.change_lists), "inbox_uids": [u for u, _ in mail.boxes["INBOX"]]}
    imap.shutdown(); smtp.shutdown()
    out = r.stdout + r.stderr
    line = next((l for l in out.splitlines() if l.startswith("ANSWER ")), None)
    if line is None:
        print(out.rstrip()[-3000:])
        print("launcher exit %d" % r.returncode)
        print("1 checks, 0 failed, 1 not judged"); return 1
    answer = json.loads(line[len("ANSWER "):])
    print("launcher exit %d; the local server saw: %s; change-list commands asked of the plain server: %d; %s" % (
        r.returncode, sorted({l.split(" ")[0] for l in mail.log}), len(server["change_lists"]), next((l for l in out.splitlines() if l.startswith("instances:")), "")))
    results = judge_moved(answer, server)
    for k in ("stored_before_move", "move2", "move1", "undo_bar", "retry_words", "wake", "rows_end", "driver_errors"):
        print("PROBE      %s %s" % (k, json.dumps(answer.get(k), ensure_ascii=False)))
    lines = ["%-10s %s  %s" % ("PASS" if ok else ("NOT JUDGED" if ok is None else "FAIL"), n, d) for n, ok, d in results]
    failed = sum(1 for _, ok, _ in results if ok is False)
    unjudged = sum(1 for _, ok, _ in results if ok is None)
    if mutant:
        red = dict((n, ok) for n, ok, _ in results)[MOVED_CASE_REPLY] is False
        lines.append("mutant %s: %s" % (MOVED_MUTANT_NAME, "RED as it must be (the reply no longer came from the stored headers)" if red else "SURVIVED: the reply was still threaded with the store's headers ignored"))
        lines.append("%d checks, %d failed, %d not judged (mutant run)" % (len(results), failed, unjudged))
        print("\n".join(lines)); return 0 if red else 1
    lines.append("%d checks, %d failed, %d not judged" % (len(results), failed, unjudged))
    print("\n".join(lines)); return 0 if failed == 0 and unjudged == 0 else 1


def selftest_moved():
    """The judge against a model run and its broken models, and its log and header parsing against the real fake (Python's own imaplib and smtplib)."""
    import imaplib
    import smtplib
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import realcore_mailfake as fake
    mid, chain = fake.message_id(1), fake.ref_chain(1)
    reply = "From: %s\r\nTo: sender1@example.test\r\nSubject: Re: Check message 1\r\nIn-Reply-To: %s\r\nReferences: %s\r\n %s\r\n %s\r\n\r\n%s\r\n" % (
        fake.ADDRESS, mid, chain[0], chain[1], mid, REPLY_MOVED)
    subjects = ["Check message 3", "Check message 2", "Check message 1"]
    good_answer = {"stage": "added", "driver_errors": [], "rows_before": subjects, "rows_after_move2": ["Check message 3", "Check message 1"],
                   "stored_before_move": {"message_id": mid, "references": chain}, "reply_sent": True, "retry_words": None}
    quiet_log = ["LOGIN x (hidden)", "UID FETCH 1:3 (UID FLAGS BODY.PEEK[HEADER])", "SELECT INBOX", "UID FETCH 3 (UID BODY.PEEK[HEADER])", "UID SEARCH UID 1:*", "IDLE"]
    good_server = {"smtp": [reply], "log": quiet_log, "away": [[2, "Archive", 2, 2], [1, "Archive", 1, 4]], "wake_mark": 6}
    g = judge_moved(good_answer, good_server)
    ok = [x for _, x, _ in g] == [True, True] and [n for n, _, _ in g] == list(MOVED_CASES)
    def verdicts(answer, server):
        return dict((n, x) for n, x, _ in judge_moved(answer, server))
    red = lambda v, name: v[name] is False and all(v[o] is True for o in MOVED_CASES if o != name)
    cut = lambda text, a, b="": text.replace(a, b)
    broken = [   # (what is broken, the case it must turn red, the verdicts of the run)
        ("mail 2 stays in the inbox", MOVED_CASE_LEAVES, verdicts(dict(good_answer, rows_after_move2=subjects), good_server)),
        ("the count falls by two", MOVED_CASE_LEAVES, verdicts(dict(good_answer, rows_after_move2=["Check message 1"]), good_server)),
        ("the reply is refused, no SMTP (the broken copy of the hook)", MOVED_CASE_REPLY, verdicts(dict(good_answer, retry_words="the message you reply to has no place on the mail server yet"), dict(good_server, smtp=[]))),
        ("no In-Reply-To", MOVED_CASE_REPLY, verdicts(good_answer, dict(good_server, smtp=[cut(reply, "In-Reply-To: %s\r\n" % mid)]))),
        ("the chain is lost", MOVED_CASE_REPLY, verdicts(good_answer, dict(good_server, smtp=[cut(reply, "References: %s\r\n %s\r\n %s" % (chain[0], chain[1], mid), "References: %s" % chain[0])]))),
        ("References does not end with the id", MOVED_CASE_REPLY, verdicts(good_answer, dict(good_server, smtp=[cut(reply, " %s\r\n\r\n" % mid, " <other@x.invalid>\r\n\r\n")]))),
        ("two messages reach SMTP", MOVED_CASE_REPLY, verdicts(good_answer, dict(good_server, smtp=[reply, reply]))),
        ("a header fetch for UID 1 after the move", MOVED_CASE_REPLY, verdicts(good_answer, dict(good_server, log=quiet_log[:4] + ["UID FETCH 1 (UID BODY.PEEK[HEADER])"] + quiet_log[4:]))),
        ("a header fetch of a range holding UID 1 after the move", MOVED_CASE_REPLY, verdicts(good_answer, dict(good_server, log=quiet_log[:4] + ["FETCH 1:* (UID BODY.PEEK[HEADER])"] + quiet_log[4:]))),
        ("the store kept the wrong chain", MOVED_CASE_REPLY, verdicts(dict(good_answer, stored_before_move={"message_id": mid, "references": []}), good_server)),
    ]
    each_red = [(what, red(v, case)) for what, case, v in broken]
    ok = ok and all(x for _, x in each_red)
    # a fetch BEFORE the move, or after the app was told (after wake_mark), is not the reply's
    ok = ok and verdicts(good_answer, dict(good_server, log=["UID FETCH 1 (UID BODY.PEEK[HEADER])"] + quiet_log, away=[[2, "Archive", 2, 3], [1, "Archive", 1, 5]], wake_mark=7))[MOVED_CASE_REPLY] is True
    ok = ok and verdicts(good_answer, dict(good_server, log=quiet_log + ["UID FETCH 1 (UID BODY.PEEK[HEADER])"], wake_mark=6))[MOVED_CASE_REPLY] is True
    # a run that did not reach a step is NOT JUDGED, never a pass: no stored headers, no reply sent, no move, no rows
    unreached = [verdicts(dict(good_answer, stored_before_move=None), good_server), verdicts(dict(good_answer, reply_sent=None), good_server),
                 verdicts(good_answer, dict(good_server, away=[[2, "Archive", 2, 2]])), verdicts(dict(good_answer, rows_after_move2=None), good_server),
                 verdicts({"stage": "added", "driver_errors": ["the list never held three rows: []"], "rows_before": []}, {})]
    ok = ok and unreached[0][MOVED_CASE_REPLY] is None and unreached[1][MOVED_CASE_REPLY] is None and unreached[2][MOVED_CASE_REPLY] is None
    ok = ok and unreached[3][MOVED_CASE_LEAVES] is None and all(v[n] is None for v in unreached[4:] for n in MOVED_CASES)
    # the log and header parsing against the real fake
    mail, imap, smtp = fake.start(0, 0, inbox=3, act=True, refs=True, plain=True)
    try:
        c = imaplib.IMAP4("127.0.0.1", imap.server_address[1])
        c.login(fake.ADDRESS, fake.PASSWORD)
        c.select("INBOX")
        start = len(mail.log)
        c.uid("FETCH", "1", "(UID BODY.PEEK[HEADER])")
        c.uid("FETCH", "3", "(UID BODY.PEEK[HEADER])")
        c.uid("FETCH", "2:*", "(UID FLAGS)")
        c.uid("FETCH", "1", "(UID BODY.PEEK[TEXT]<0.2048>)")
        c.logout()
        found = header_fetches(mail.log, 1, start, len(mail.log))
        parsed = len(found) == 1 and not header_fetches(mail.log, 2, start, len(mail.log)) and len(header_fetches(mail.log, 3, start, len(mail.log))) == 1
        s = smtplib.SMTP("127.0.0.1", smtp.server_address[1]); s.ehlo(); s.login(fake.ADDRESS, fake.PASSWORD)
        s.sendmail(fake.ADDRESS, ["sender1@example.test"], reply); s.quit()
        irt, refs = smtp_threading(smtp.messages[0].decode())
        parsed = parsed and irt == [mid] and refs == chain + [mid]
    finally:
        imap.shutdown(); smtp.shutdown()
    print("selftest slice moved: model %s, %d broken models %s, the fetch-window and not-reached runs %s, log and header parsing against the fake %s" % (
        "all pass" if [x for _, x, _ in g] == [True, True] else "FAILS", len(broken), "red on their own case only" if all(x for _, x in each_red) else "SURVIVE: " + str([w for w, x in each_red if not x]),
        "yes" if ok else "WRONG", "right" if parsed else "WRONG"))
    return ok and parsed


# ---------------------------------------------------------------- lane tail-realcore: four more slices on the real engine (body, rulecount, crawl, aikey)
# Each is driven like the slices above: the local servers live in THIS process, the hidden copy reaches 127.0.0.1 only ("nothing leaves this Mac except the relay"),
# every case prints `PASS|FAIL|NOT JUDGED <name>  <detail>`, a case whose app part is not served is NOT JUDGED with the reason (never a pass), and no mail text is ever
# printed: the details are counts, booleans and header words.

def run_inner(bundle, extra, inner_flags):
    """The hidden launch of one slice: `extra` are the app's LAUNCH_ENV words, `inner_flags` what follows `--inner`. Returns (answer or None, the launcher's text, the result)."""
    import json
    env = dict(os.environ, WHOOSH_RIG_HIDDEN="1", LAUNCH_ENV=" ".join(["WHOOSH_RIG_HIDDEN=1"] + list(extra)))
    env.pop("WHOOSH_FIXTURE_CORE", None)
    cmd = ["sh", os.path.join(ROOT, "scripts", "launch-one.sh"), bundle, "--", "--rig-hidden",
           "sh", "-c", 'WHOOSH_SOCKET_DIR="$D" python3 "$0" --inner ' + inner_flags, os.path.abspath(__file__)]
    r = launch(cmd, env)
    out = r.stdout + r.stderr
    line = next((l for l in out.splitlines() if l.startswith("ANSWER ")), None)
    return (json.loads(line[len("ANSWER "):]) if line else None), out, r


def mutant_verdict(mutant_name, results, must_red, red_words="", survived_words=""):
    """(line, exit code) of a mutant run: `mutant <name>: KILLED by <cases>` and 0 when the red cases are exactly `must_red`; else SURVIVED (an expected case not
    red) or `ALSO RED` (a red case not expected), and 1."""
    red = [n for n, ok, _ in results if ok is False]
    want = list(must_red)
    missing = [n for n in want if n not in red]
    extra = [n for n in red if n not in want]
    if not want:
        return "mutant %s: NO EXPECTED CASES named for it in this run" % mutant_name, 1
    if missing:
        return "mutant %s: SURVIVED: %s (or the build has no such switch); expected red, not red: %s%s" % (
            mutant_name, survived_words or "the expected cases were not all red", ", ".join(missing), "; also red: %s" % ", ".join(extra) if extra else ""), 1
    if extra:
        return "mutant %s: ALSO RED beyond the expected cases: %s (expected red: %s)" % (mutant_name, ", ".join(extra), ", ".join(want)), 1
    return "mutant %s: KILLED by %s%s" % (mutant_name, ", ".join(want), " (%s)" % red_words if red_words else ""), 0


def slice_lines(results, mutant_name=None, must_red=(), red_words="", survived_words="", note=None):
    """(lines, exit code) of a slice's report: one line per case, then, in a mutant run, whether the cases that must be red were, and the count line."""
    lines = []
    if note:
        lines.append(note)
    failed = sum(1 for _, ok, _ in results if ok is False)
    unjudged = sum(1 for _, ok, _ in results if ok is None)
    if mutant_name and mutant_verdict(mutant_name, results, must_red)[1] == 0:
        # fix-realcore round 7 (lead): a correct KILL is a clean run: its expected reds print KILLED (never FAIL) and the count line says 0 failed
        lines += ["%-10s %s  %s" % ("PASS" if ok else ("NOT JUDGED" if ok is None else ("KILLED" if n in must_red else "FAIL")), n, d) for n, ok, d in results]
        line, _ = mutant_verdict(mutant_name, results, must_red, red_words, survived_words)
        lines.append(line)
        lines.append("%d checks, 0 failed, %d not judged (mutant run: %d expected red, all KILLED)" % (len(results), unjudged, failed))
        return lines, 0
    lines += ["%-10s %s  %s" % ("PASS" if ok else ("NOT JUDGED" if ok is None else "FAIL"), n, d) for n, ok, d in results]
    if mutant_name:
        # fix-realcore round 2 (lead): a mutant run is clean when EXACTLY the expected cases are red: `KILLED by <cases>`, exit 0. An expected case
        # that is not red (passed or NOT JUDGED) is SURVIVED; a red case the mutant does not explain is named, and either exits 1.
        line, rc = mutant_verdict(mutant_name, results, must_red, red_words, survived_words)
        lines.append(line)
        lines.append("%d checks, %d failed, %d not judged (mutant run)" % (len(results), failed, unjudged))
        return lines, rc
    lines.append("%d checks, %d failed, %d not judged" % (len(results), failed, unjudged))
    return lines, 0 if failed == 0 and unjudged == 0 else 1


def no_answer(out, r):
    print(out.rstrip()[-3000:])
    print("launcher exit %d" % r.returncode)
    print("1 checks, 0 failed, 1 not judged")
    return 1


def drive_step(answer, label, fn):
    """One driver step: a step that cannot run is told in `driver_errors`, never a crash of the whole answer."""
    try:
        return fn()
    except Exception as e:      # noqa: BLE001
        answer.setdefault("driver_errors", []).append("%s: %s: %s" % (label, type(e).__name__, e))
        return None


# ---- slice body (item d99929d0): a multipart quoted-printable mail is shown as decoded text, on the real engine.
BODY_MAILS = ("sweetgreen", "soyo")
BODY_KINDS = ("body-has-no-boundary", "body-has-no-content-type", "body-has-no-soft-break", "body-holds-the-decoded-words", "preview-is-decoded")
BODY_ALL_CASES = ("account-added", "list-shows-the-two-mails-newest-first") + tuple("%s@%s" % (k, m) for m in BODY_MAILS for k in BODY_KINDS)
BODY_MUTANT_NAME = "body-stored-raw"        # core/crates/engine/src/rust_core.rs (fix-core): the fetched message is kept whole as the text
# Item f5ce98e7 (15 Nov, main191): the broken copy stores the fetched message whole as the text. Since 14 Nov the READER reads a raw-stored mail as
# its words (its rescue of mail an older version stored raw), so the four body kinds stay green on this broken copy, and that is right: a red there
# would mean the rescue is gone. What the broken copy breaks is the list row's preview, which the core makes from the stored text. A clean broken-copy
# run is therefore: the two preview cases red, every body case green (`mutant_verdict`: exactly the expected cases red; a body case red is ALSO RED).
# Before this the expected set was every kind, and the run ended SURVIVED on main180 and main191 with the previews red.
BODY_MUTANT_RED = tuple("preview-is-decoded@%s" % m for m in BODY_MAILS)
BODY_BODY_WAIT_S = 45      # a check bound: the sync, the background download and the parse each land on their own
BODY_PREVIEW_WAIT_S = 30   # a check bound: the list row's preview is made from the stored body
BODY_LEADS = {"sweetgreen": "The Omega Glow Bowl", "soyo": "Soyo World weekly picks"}   # the first words of each decoded text (what a preview must start with)
SOFT_BREAK_OR_ESCAPE = re.compile(r"=(?:\r?\n|$)|=[0-9A-F]{2}", re.M)    # a quoted-printable soft line break, or an escape left undecoded


def body_shapes(fake):
    return [{"tag": "sweetgreen", "subject": fake.SWEET_SUBJECT, "words": list(fake.SWEET_WORDS), "bound": fake.BODY_BOUNDARY, "lead": BODY_LEADS["sweetgreen"]},
            {"tag": "soyo", "subject": fake.SOYO_SUBJECT, "words": list(fake.SOYO_WORDS), "bound": fake.SOYO_BOUNDARY, "lead": BODY_LEADS["soyo"]}]


def body_select(c, key, shape):
    """One move with a key (no Enter), then the reader's body for the focused mail, waited for until it holds the decoded words or shows raw MIME, then the list row's preview."""
    out = {"subject": shape["subject"]}
    c.key(key)
    out["focus"] = c.uistate().get("focus_message")
    text = lambda r: "\n".join((r[0].get("paragraphs") or [])) if r else ""
    raw_seen = lambda t: any(b in t for b in (shape["bound"], "Content-", "MIME-Version"))
    body, secs = mv_poll(lambda: c.rows("mac-main.reader.body"), lambda r: bool(r) and (all(w in " ".join(text(r).split()) for w in shape["words"]) or raw_seen(text(r))), BODY_BODY_WAIT_S)
    out["paragraphs"] = (body or [{}])[0].get("paragraphs") or []
    out["body_seconds"] = secs

    def row():
        return next((r for r in c.rows("mac-main.list") if r.get("id") == out["focus"]), None)
    prev, psecs = mv_poll(row, lambda r: bool(r) and bool((r.get("preview") or "").strip()) and (shape["lead"] in r["preview"] or raw_seen(r["preview"]) or "=" in r["preview"]), BODY_PREVIEW_WAIT_S)
    out["row_subject"] = (prev or {}).get("subject")
    out["preview"] = (prev or {}).get("preview") or ""
    out["preview_seconds"] = psecs
    return out


def drive_body():
    """Inside launch-one: add the local account (two mails), select the older mail (J) and then the newer one (K), read the reader's body rows and the list row's preview for each."""
    sys.path.insert(0, os.path.join(ROOT, "journeys"))
    from lib.ctl import Ctl  # noqa: E402
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import realcore_mailfake as fake  # noqa: E402
    c = Ctl().connect()
    answer = {"driver_errors": [], "core": None, "reads": {}}
    mv_add_account(c, fake, answer, 2)
    answer["core"] = c.uistate().get("core")
    shapes = {s["tag"]: s for s in body_shapes(fake)}
    for tag, key in (("sweetgreen", "J"), ("soyo", "K")):      # the list is newest first: the Soyo mail (uid 2) is on top, the sweetgreen mail (uid 1) below it
        answer["reads"][tag] = drive_step(answer, "select " + tag, lambda tag=tag, key=key: body_select(c, key, shapes[tag]))
    return answer


def judge_body(answer):
    """[(name, ok, detail)]. A negative case (no boundary, no header, no soft break) on an empty body would pass for nothing, so it is NOT JUDGED then; the case of the decoded words
    is the one that asks for the text. The details hold counts and booleans only, never a word of a mail."""
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import realcore_mailfake as fake
    errors = answer.get("driver_errors") or []
    out = [("account-added", answer.get("stage") in ("added", "summary"), "stage %r, core %r" % (answer.get("stage"), answer.get("core"))),
           ("list-shows-the-two-mails-newest-first", [r.get("subject") for r in answer.get("rows") or []] == [fake.SOYO_SUBJECT, fake.SWEET_SUBJECT],
            "the list holds %d row(s), subjects as wanted %s" % (len(answer.get("rows") or []), [r.get("subject") for r in answer.get("rows") or []] == [fake.SOYO_SUBJECT, fake.SWEET_SUBJECT]))]
    for shape in body_shapes(fake):
        tag, rd = shape["tag"], (answer.get("reads") or {}).get(shape["tag"])
        names = ["%s@%s" % (k, tag) for k in BODY_KINDS]
        if not rd:
            out += [(n, None, "the mail was not selected: %s" % errors) for n in names]
            continue
        paragraphs = rd.get("paragraphs") or []
        joined = "\n".join(paragraphs)
        flat = " ".join(joined.split())
        shown = "%d paragraph(s), %d characters, after %ss, focused row's subject %s" % (len(paragraphs), len(joined), rd.get("body_seconds"), "as wanted" if rd.get("row_subject") == shape["subject"] else "NOT as wanted")
        none_if_empty = lambda ok: None if not joined.strip() else ok
        no_boundary = ("--" + shape["bound"]) not in joined and shape["bound"] not in joined and not re.search(r"(?m)^--", joined)
        out.append((names[0], none_if_empty(no_boundary), "%s; boundary line present: %s" % (shown, not no_boundary) if joined.strip() else "the reader's body is empty (after %ss): nothing to judge" % rd.get("body_seconds")))
        out.append((names[1], none_if_empty("Content-" not in joined and "MIME-Version" not in joined), "%s; a header line present: %s" % (shown, "Content-" in joined or "MIME-Version" in joined)))
        soft = SOFT_BREAK_OR_ESCAPE.search(joined) is not None
        out.append((names[2], none_if_empty(not soft), "%s; a soft line break or an undecoded escape present: %s" % (shown, soft)))
        missing = [i for i, w in enumerate(shape["words"], 1) if w not in flat]
        out.append((names[3], bool(joined.strip()) and not missing, "%s; decoded sentence(s) missing: %s of %d" % (shown, missing, len(shape["words"]))))
        prev = rd.get("preview") or ""
        pbad = ("--" + shape["bound"]) in prev or shape["bound"] in prev or "Content-" in prev or "MIME-Version" in prev or SOFT_BREAK_OR_ESCAPE.search(prev) is not None
        out.append((names[4], bool(prev.strip()) and shape["lead"] in prev and not pbad,
                    "the list row's preview: %d characters after %ss, starts with the decoded words: %s, holds a boundary, header, soft break or escape: %s" % (len(prev), rd.get("preview_seconds"), shape["lead"] in prev, pbad)))
    out.append(("the-driver-reached-every-step", not errors, "driver errors %r" % errors))
    return out


def main_slice_body(bundle, mutant):
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import realcore_mailfake as fake
    try:
        mail, imap, smtp = fake.start(0, 0, inbox=0)
    except OSError as e:
        print("the local servers did not start: %s" % e)
        print("1 checks, 0 failed, 1 not judged"); return 1
    fake.load_body_mail(mail)
    ports = "%d:%d" % (imap.server_address[1], smtp.server_address[1])
    extra = ["WHOOSH_CORE_PLAIN_LOOPBACK=1", "WHOOSH_CORE_CHECK_SERVERS=" + ports] + (["WHOOSH_CORE_MUTANT=" + BODY_MUTANT_NAME] if mutant else [])
    answer, out, r = run_inner(bundle, extra, "--slice-body")
    imap.shutdown(); smtp.shutdown()
    if answer is None:
        return no_answer(out, r)
    print("launcher exit %d; the local server saw: %s; %s" % (r.returncode, sorted({l.split(" ")[0] for l in mail.log}), next((l for l in out.splitlines() if l.startswith("instances:")), "")))
    lines, rc = slice_lines(judge_body(answer), BODY_MUTANT_NAME if mutant else None, BODY_MUTANT_RED,
                            "the raw message was stored as the text: the list row's preview came back raw, and the reader still read the mail's words", "a raw message was stored and the list row's preview still showed decoded text")
    print("\n".join(lines)); return rc


def body_answer(mutant=None):
    """The self-test's stand-in for the app: what a correct engine shows (or one defect, `mutant`), in the shape drive_body returns. No app is launched."""
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import realcore_mailfake as fake
    reads = {}
    for shape in body_shapes(fake):
        tag = shape["tag"]
        raw = (fake.sweetgreen_raw() if tag == "sweetgreen" else fake.soyo_raw()).decode()
        paragraphs = [" ".join(shape["words"]) if tag == "sweetgreen" else shape["words"][0]]
        preview = (shape["lead"] + ": a short line of the decoded text.")
        if mutant == "raw-body":      # `body-stored-raw` under a reader with no rescue: the message is stored whole as the text and shown so, and the row's preview is made from it
            paragraphs = [p for p in raw.split("\r\n\r\n") if p.strip()]
            preview = " ".join(raw.split())[:120]
        if mutant == "raw-stored-rescued":   # `body-stored-raw` as the app shows it since 14 Nov: the reader reads the words, the core's preview is raw
            preview = " ".join(raw.split())[:120]
        if mutant == "boundary-only":
            paragraphs = paragraphs + ["--" + shape["bound"]]
        if mutant == "header-only":
            paragraphs = ["Content-Type: text/plain; charset=utf-8"] + paragraphs
        if mutant == "soft-break-only":
            paragraphs = [paragraphs[0].replace("almonds", "almo=\nnds").replace("quiet corner", "quiet cor=\nner")]
        if mutant == "escape-only":
            paragraphs = [paragraphs[0] + " class=3D\"x\""]
        if mutant == "empty-body":
            paragraphs = []
        if mutant == "words-cut":
            paragraphs = ["a body that is not the decoded text"]
        if mutant == "raw-preview":
            preview = "From: " + shape["lead"] + " Content-Type: multipart/alternative; boundary=" + shape["bound"]
        if mutant == "soft-preview":
            preview = shape["lead"] + " almo=\nnds over cor="
        if mutant == "empty-preview":
            preview = ""
        if mutant == "other-preview":
            preview = "A short line with the wrong words."
        reads[tag] = {"subject": shape["subject"], "focus": "m-" + tag, "paragraphs": paragraphs, "body_seconds": 1.2, "row_subject": shape["subject"], "preview": preview, "preview_seconds": 0.8}
    ans = {"stage": "added", "core": "rust", "driver_errors": [], "reads": reads,
           "rows": [{"id": "m-soyo", "subject": fake.SOYO_SUBJECT, "preview": ""}, {"id": "m-sweetgreen", "subject": fake.SWEET_SUBJECT, "preview": ""}]}
    if mutant == "list-order":
        ans["rows"] = list(reversed(ans["rows"]))
    if mutant == "not-selected":
        ans["reads"] = {"sweetgreen": None, "soyo": None}; ans["driver_errors"] = ["select sweetgreen: Unreadable: no row"]
    return ans


BODY_SELFTEST_MUTANTS = {   # defect -> the cases that must go red; the NOT JUDGED ones are listed apart
    "raw-body": tuple("%s@%s" % (k, m) for m in BODY_MAILS for k in BODY_KINDS[:4]),
    "boundary-only": ("body-has-no-boundary@sweetgreen", "body-has-no-boundary@soyo"),
    "header-only": ("body-has-no-content-type@sweetgreen", "body-has-no-content-type@soyo"),
    "soft-break-only": ("body-has-no-soft-break@sweetgreen", "body-has-no-soft-break@soyo", "body-holds-the-decoded-words@sweetgreen", "body-holds-the-decoded-words@soyo"),
    "escape-only": ("body-has-no-soft-break@sweetgreen", "body-has-no-soft-break@soyo"),
    "words-cut": ("body-holds-the-decoded-words@sweetgreen", "body-holds-the-decoded-words@soyo"),
    "raw-preview": ("preview-is-decoded@sweetgreen", "preview-is-decoded@soyo"),
    "soft-preview": ("preview-is-decoded@sweetgreen", "preview-is-decoded@soyo"),
    "empty-preview": ("preview-is-decoded@sweetgreen", "preview-is-decoded@soyo"),
    "other-preview": ("preview-is-decoded@sweetgreen", "preview-is-decoded@soyo"),
    "list-order": ("list-shows-the-two-mails-newest-first",),
}


def selftest_body():
    """The judge against a model of a correct engine (every case passes), one broken engine per behaviour (each case red, only the cases the defect touches), an empty body (the three
    negative cases NOT JUDGED, never a pass) and a mail that was not reached (every case of it NOT JUDGED). The fixture mails are read back by realcore_mailfake's own self-test."""
    verdict = lambda ans: {n: ok for n, ok, _ in judge_body(ans)}
    good = verdict(body_answer())
    ok_good = list(good) == list(BODY_ALL_CASES) + ["the-driver-reached-every-step"] and all(v is True for v in good.values())
    survived, touched_bad = [], []
    for mutant, targets in BODY_SELFTEST_MUTANTS.items():
        got = verdict(body_answer(mutant))
        targets = tuple(t for t in targets if t in got)
        if not targets or not all(got[t] is False for t in targets):
            survived.append(mutant)
        touched = sorted(k for k, v in got.items() if v is False and k not in targets)
        if touched and mutant != "raw-body":
            touched_bad.append("%s also reddens %s" % (mutant, touched))
    raw = verdict(body_answer("raw-body"))
    ok_raw = all(raw["%s@%s" % (k, m)] is False for m in BODY_MAILS for k in BODY_KINDS) and raw["preview-is-decoded@sweetgreen"] is False
    empty = verdict(body_answer("empty-body"))
    ok_empty = all(empty["%s@%s" % (k, m)] is None for m in BODY_MAILS for k in BODY_KINDS[:3]) and all(empty["body-holds-the-decoded-words@%s" % m] is False for m in BODY_MAILS)
    unreached = verdict(body_answer("not-selected"))
    ok_unreached = all(unreached["%s@%s" % (k, m)] is None for m in BODY_MAILS for k in BODY_KINDS) and unreached["the-driver-reached-every-step"] is False
    covered = {n for n in BODY_ALL_CASES if any(verdict(body_answer(m)).get(n) is False for m in BODY_SELFTEST_MUTANTS)}
    ok_cover = covered >= set(BODY_ALL_CASES) - {"account-added"}
    # the --mutant run on the model of today's app (raw stored, the reader rescues) ends `KILLED by` exactly BODY_MUTANT_RED (exit 0); on the correct
    # model it SURVIVES (exit 1); on a reader that lost its rescue the body cases are red too and the run says ALSO RED (exit 1)
    lost = mutant_verdict(BODY_MUTANT_NAME, judge_body(body_answer("raw-body")), BODY_MUTANT_RED)
    mutant_red = mutant_verdict(BODY_MUTANT_NAME, judge_body(body_answer("raw-stored-rescued")), BODY_MUTANT_RED)[1] == 0 \
        and mutant_verdict(BODY_MUTANT_NAME, judge_body(body_answer()), BODY_MUTANT_RED)[1] == 1 \
        and lost[1] == 1 and "ALSO RED" in lost[0]
    print("selftest slice body: model %s, %d broken models %s, an empty body %s, a mail not reached %s, the --mutant run's cases red on the raw model %s, cases covered by a broken model %d of %d" % (
        "all pass" if ok_good else "FAILS", len(BODY_SELFTEST_MUTANTS), "each red on its own cases" if not survived and not touched_bad else "SURVIVE %s; %s" % (survived, touched_bad),
        "NOT JUDGED on the three negative cases, the decoded words red" if ok_empty else "WRONG", "NOT JUDGED" if ok_unreached else "WRONG", "yes" if mutant_red else "NO", len(covered), len(BODY_ALL_CASES) - 1))
    return ok_good and not survived and not touched_bad and ok_raw and ok_empty and ok_unreached and ok_cover and mutant_red


# ---- slice rulecount (item f2dd23de): the label rule preview's count is real on the real engine (rule_set_count), not 0.
RC_NAMES = ("account-added", "rule-count-unread-and-replied-by-me-is-not-zero", "rule-count-unread-and-not-replied-by-me-is-two", "rule-count-rows-are-the-count")
RC_MUTANT_NAME = "rule-counts-zero"      # core/crates/engine/src/rust_core.rs (fix-core): the label rule count answers 0
RC_MUTANT_RED = RC_NAMES[1:]
RC_INS = "mac-labels.inspector"          # Sources/Features/LabelsContactsMac/LabelInspectorSocket.swift; checks/parity/par-labelscontactsmac.py INS
RC_PLUS = "mac-main.sidebar.labels.add"
RC_SETTLE_S = 20                         # a check bound: the count is read off the main thread, the inspector says `counting` until it is done


def rc_read(c):
    """The inspector's summary once it has stopped counting: {count, matches, rows}, or {'error': words}."""
    t0 = time.time()
    s = {}
    while time.time() - t0 < RC_SETTLE_S:
        rows = c.rows(RC_INS)
        s = rows[0] if rows else {}
        if s and not s.get("counting"):
            break
        time.sleep(0.15)
    if not s:
        return {"error": "no row in region %s" % RC_INS}
    if s.get("counting"):
        return {"error": "the count never settled in %ss" % RC_SETTLE_S}
    return {"count": s.get("count"), "matches": s.get("matches"), "rows": len(c.rows(RC_INS + ".matches")), "open": s.get("open"), "mode": s.get("mode")}


def rc_after(c, answer):
    """On the slice-3 mailbox, after the sync: the labels screen's new-label inspector, with 'unread' yes and 'replied by me' yes (one message: the one that follows the owner's Sent mail
    in its thread), then 'replied by me' no (the two that never had a reply from the owner)."""
    rc = answer["rulecount"] = {"driver_errors": []}
    drive_step(rc, "open the new-label inspector", lambda: c.click(RC_PLUS))
    if rc["driver_errors"]:
        return
    for key, doors in (("unread_and_replied", (".entry.fact.unread.yes", ".entry.fact.replied_by_me.yes")), ("unread_and_not_replied", (".entry.fact.replied_by_me.no",))):
        for d in doors:
            drive_step(rc, "click " + d, lambda d=d: c.click(RC_INS + d))
        rc[key] = drive_step(rc, "read the count for " + key, lambda: rc_read(c))


def drive_rulecount():
    return flow_drive(FLOW_SUBJECTS, (FLOW_SUBJECTS[0], FLOW_SUBJECTS[2]), after=rc_after)


def judge_rulecount(answer):
    rc = answer.get("rulecount") or {}
    errors = rc.get("driver_errors") or []
    out = [(RC_NAMES[0], answer.get("stage") == "added", "stage %r, error %r" % (answer.get("stage"), answer.get("error")))]
    a, b = rc.get("unread_and_replied") or {}, rc.get("unread_and_not_replied") or {}
    why = lambda x: "not reached: %s" % (x.get("error") or errors or "the inspector was not opened")
    if "count" not in a:
        out.append((RC_NAMES[1], None, why(a)))
        out.append((RC_NAMES[3], None, why(a)))
    else:
        out.append((RC_NAMES[1], a.get("count") == 1, "unread AND replied by me: the inspector's count %r (wanted 1: the message that follows the owner's Sent mail in its thread; 0 is the stub)" % (a.get("count"),)))
        out.append((RC_NAMES[3], a.get("count") == 1 and a.get("matches") == 1 and a.get("rows") == 1,
                    "count %r, matches %r, rows listed %r (each wanted 1)" % (a.get("count"), a.get("matches"), a.get("rows"))))
    if "count" not in b:
        out.append((RC_NAMES[2], None, why(b)))
    else:
        out.append((RC_NAMES[2], b.get("count") == 2, "unread AND NOT replied by me: the inspector's count %r (wanted 2: the contract mail and the lunch mail; a count that is always 1 fails here)" % (b.get("count"),)))
    return sorted(out, key=lambda x: RC_NAMES.index(x[0]))


def main_slice_rulecount(bundle, mutant):
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import realcore_mailfake as fake
    try:
        mail, imap, smtp = fake.start(0, 0, inbox=0)
    except OSError as e:
        print("the local servers did not start: %s" % e)
        print("1 checks, 0 failed, 1 not judged"); return 1
    load_flow_mail(fake, mail)
    ports = "%d:%d" % (imap.server_address[1], smtp.server_address[1])
    extra = ["WHOOSH_CORE_PLAIN_LOOPBACK=1", "WHOOSH_CORE_CHECK_SERVERS=" + ports, "WHOOSH_CORE_CHECK_FLOW=1"] + (["WHOOSH_CORE_MUTANT=" + RC_MUTANT_NAME] if mutant else [])
    answer, out, r = run_inner(bundle, extra, "--slice-rulecount")
    imap.shutdown(); smtp.shutdown()
    if answer is None:
        return no_answer(out, r)
    print("launcher exit %d; the local server saw: %s; %s" % (r.returncode, sorted({l.split(" ")[0] for l in mail.log}), next((l for l in out.splitlines() if l.startswith("instances:")), "")))
    lines, rc = slice_lines(judge_rulecount(answer), RC_MUTANT_NAME if mutant else None, RC_MUTANT_RED, "the label rule count answered 0 and both counts read wrong",
                            "the label rule counts were right although the engine counts nothing",
                            note="SEEDED-SLICE-RULECOUNT: the labels of slice 3 are SEEDED by the check (WHOOSH_CORE_CHECK_FLOW): this proves the engine's count, NOT a first run of the installed app")
    print("\n".join(lines)); return rc


def rulecount_answer(mutant=None):
    a = {"count": 1, "matches": 1, "rows": 1}
    b = {"count": 2, "matches": 2, "rows": 2}
    if mutant == "zero":
        a, b = {"count": 0, "matches": 0, "rows": 0}, {"count": 0, "matches": 0, "rows": 0}
    if mutant == "always-one":
        b = {"count": 1, "matches": 1, "rows": 1}
    if mutant == "count-not-rows":
        a = {"count": 1, "matches": 1, "rows": 0}
    ans = {"stage": "added", "rulecount": {"driver_errors": [], "unread_and_replied": a, "unread_and_not_replied": b}}
    if mutant == "no-door":
        ans["rulecount"] = {"driver_errors": ["open the new-label inspector: Refused: no such door"]}
    if mutant == "unsettled":
        ans["rulecount"] = {"driver_errors": [], "unread_and_replied": {"error": "the count never settled in 20s"}, "unread_and_not_replied": {"error": "the count never settled in 20s"}}
    if mutant == "no-account":
        ans["stage"] = "failed"
    return ans


def selftest_rulecount():
    verdict = lambda ans: {n: ok for n, ok, _ in judge_rulecount(ans)}
    good = verdict(rulecount_answer())
    ok_good = list(good) == list(RC_NAMES) and all(v is True for v in good.values())
    want = {"zero": RC_NAMES[1:], "always-one": (RC_NAMES[2],), "count-not-rows": (RC_NAMES[3],), "no-account": (RC_NAMES[0],)}
    survived, touched_bad = [], []
    for mutant, targets in want.items():
        got = verdict(rulecount_answer(mutant))
        if not all(got[t] is False for t in targets):
            survived.append(mutant)
        touched = sorted(k for k, v in got.items() if v is False and k not in targets)
        if touched:
            touched_bad.append("%s also reddens %s" % (mutant, touched))
    nj = [verdict(rulecount_answer(m)) for m in ("no-door", "unsettled")]
    ok_nj = all(v[RC_NAMES[1]] is None and v[RC_NAMES[2]] is None and v[RC_NAMES[3]] is None for v in nj)
    zero = verdict(rulecount_answer("zero"))
    mutant_red = mutant_verdict(RC_MUTANT_NAME, judge_rulecount(rulecount_answer("zero")), RC_MUTANT_RED)[1] == 0 \
        and mutant_verdict(RC_MUTANT_NAME, judge_rulecount(rulecount_answer()), RC_MUTANT_RED)[1] == 1      # fix-realcore round 2: exactly its cases
    print("selftest slice rulecount: model %s, %d broken models %s, no inspector door and a count that never settles %s, the --mutant run's cases red on the zero model %s" % (
        "all pass" if ok_good else "FAILS", len(want), "each red on its own cases" if not survived and not touched_bad else "SURVIVE %s; %s" % (survived, touched_bad),
        "NOT JUDGED, never a pass" if ok_nj else "WRONG", "yes" if mutant_red else "NO"))
    return ok_good and not survived and not touched_bad and ok_nj and mutant_red


# ---- slice crawl (items f4e1fbe8 and 0c272b3c piece 3): every body is downloaded and stored in the background, and every stored mail is indexed, on the real engine.
# The server holds INBOX 25, Archive 65, Sent 10 (realcore_mailfake.CRAWL_PLAN) and records every whole-message fetch with its connection and folder. The engine's own figures are read from
# `core state reply/progress` (one object per account and folder: messages, bodies, indexed).
CRAWL_CASES = ("every-body-of-every-folder-is-stored", "inbox-first-newest-first", "fetches-come-in-batches", "one-connection", "bodies-survive-a-relaunch",
               "crawled-mail-found-by-a-word", "crawled-mail-found-by-meaning", "index-progress-reaches-every-message")
CRAWL_PAUSED_CASES = ("paused-crawl-fetches-no-body-of-another-folder", "paused-crawl-leaves-bodies-unstored")
CRAWL_MUTANTS = {   # WHOOSH_CORE_MUTANT name -> the case indexes that must go red (core/crates/engine/src/rust_core.rs `crawl-never-runs`; rig.rs `bodies-never-stored`, `index-never-writes`)
    "crawl-never-runs": (0, 4, 5), "bodies-never-stored": (0, 4, 5), "index-never-writes": (5,)}
CRAWL_MUTANTS_WITH_MODEL = {"crawl-never-runs": (6, 7), "bodies-never-stored": (6, 7), "index-never-writes": ()}   # fix-realcore round 2: with a model folder the meaning mail
# (its sentence past the preview the sync fetches) is never embedded and the indexed count never reaches every message; the word index alone changes neither
CRAWL_DEFAULT_MUTANT = "crawl-never-runs"


def crawl_mutant_red(name, weights_given):
    """The cases a crawl mutant run must turn red, and only those."""
    return tuple(CRAWL_CASES[i] for i in CRAWL_MUTANTS.get(name, ()) + (CRAWL_MUTANTS_WITH_MODEL.get(name, ()) if weights_given else ()))
CRAWL_WAIT_S = 150            # a check bound: the sync, then the crawl of three folders over a local server
CRAWL_INDEX_WAIT_S = 90       # a check bound: a body's words are indexed after it is stored
CRAWL_PAUSE_WINDOW_S = 20     # a check bound: how long a held crawl is watched for a fetch it must not make
CRAWL_RELAUNCH_READ_S = 10    # a check bound: the engine's figures are served after the relaunch
CRAWL_STREAM_MAX = 2          # one connection for the bodies beside the sync's IDLE one (core/crates/engine/tests/engine_crawl_wire.rs states the same limit)
CRAWL_PAUSE_DEFAULT = "battery:15"
# fix-realcore: the kinds of hit the meaning ranking gives (search_cmd.rs `matched_word`). A mail that stands out by meaning is `related` (SPEC A150: matches, then
# related mail, then the rest by closeness), so neither kind is a word match, and the meaning group's first mail is the first hit of either kind.
CRAWL_MEANING_KINDS = ("related", "meaning")


def crawl_progress(c):
    """`core state reply/progress` as sums and per folder, None when the path is not served yet."""
    v = cv(c, "reply/progress")
    if not isinstance(v, list):
        return None
    rows = [r for r in v if isinstance(r, dict)]
    num = lambda r, k: int(r.get(k) or 0)
    return {"messages": sum(num(r, "messages") for r in rows), "bodies": sum(num(r, "bodies") for r in rows), "indexed": sum(num(r, "indexed") for r in rows),
            "folders": {str(r.get("folder")): [num(r, "messages"), num(r, "bodies"), num(r, "indexed")] for r in rows}}


def crawl_search(c, words, found, wait_s):
    """Asks for `words` (alternating the capital: a text already run is run once) until `found(state)`, the budget ends or the engine ends the search with no answer. Returns the last state, compact."""
    t0, i, st = time.time(), 0, None
    while True:
        st = run_search(c, words[i % 2])
        subj = lambda h: (st.get("screen") or {}).get(h.get("id"))
        hits = [{"matched_in": h.get("matched_in"), "subject": subj(h)} for h in st.get("hits") or []]
        res = {"stage": st.get("stage"), "count": st.get("count"), "hits": hits, "reason": st.get("reason"), "waited": round(time.time() - t0, 1)}
        if found(res) or time.time() - t0 > wait_s or st.get("stage") in ("model_failed", "failed"):
            return res
        i += 1
        time.sleep(2)


def drive_crawl():
    """Inside launch-one: add the local account, watch the crawl store every body (the engine's figures), search a word that is only in a crawled Archive body, search by meaning for another
    (with a model folder), read the indexed count, restart the copy and read the figures at once. With --paused only the first part: the held crawl is watched for CRAWL_PAUSE_WINDOW_S."""
    sys.path.insert(0, os.path.join(ROOT, "journeys"))
    from lib.ctl import Ctl  # noqa: E402
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import realcore_mailfake as fake  # noqa: E402
    c = Ctl().connect()
    answer = {"driver_errors": [], "core": None}
    total = sum(n for _, n in fake.CRAWL_PLAN)
    answer["total"] = total
    mv_add_account(c, fake, answer, 3)
    answer["core"] = c.uistate().get("core")
    paused = "--paused" in sys.argv
    if paused:
        time.sleep(CRAWL_PAUSE_WINDOW_S)
        answer["progress"] = crawl_progress(c)
        answer["background"] = drive_step(answer, "background row", lambda: (c.rows("mac-main.background") or [{}])[0])
        answer["engine_pause"] = cv(c, "background/pause")      # fix-realcore round 13: what the engine applies (the shell's row shows what the shell measured)
        return answer
    t0 = time.time()
    got, secs = mv_poll(lambda: crawl_progress(c), lambda p: p is not None and p["bodies"] >= total, CRAWL_WAIT_S)
    answer["progress"], answer["progress_seconds"] = got, secs
    answer["background"] = drive_step(answer, "background row", lambda: (c.rows("mac-main.background") or [{}])[0])
    weights = bool(os.environ.get("WHOOSH_EMBED_WEIGHTS", ""))
    word_found = lambda res: any(h["subject"] == fake.CRAWL_WORD_SUBJECT and h["matched_in"] not in CRAWL_MEANING_KINDS for h in res["hits"])
    answer["word"] = drive_step(answer, "search the word", lambda: crawl_search(c, (fake.CRAWL_WORD, fake.CRAWL_WORD.capitalize()), word_found, CRAWL_INDEX_WAIT_S))
    if weights:
        q = fake.CRAWL_MEANING_QUERY
        first = lambda res: next((h["subject"] for h in res["hits"] if h["matched_in"] in CRAWL_MEANING_KINDS), None)
        answer["meaning"] = drive_step(answer, "search by meaning", lambda: crawl_search(c, (q, q.capitalize()), lambda res: res["stage"] == "ranked" and first(res) == fake.CRAWL_MEANING_SUBJECT, SEARCH_SETTLE_S))
        got, secs = mv_poll(lambda: crawl_progress(c), lambda p: p is not None and p["indexed"] >= p["bodies"] >= total, SEARCH_SETTLE_S)
        answer["indexed"], answer["indexed_seconds"] = got, secs
    nc, why = restart(c)
    answer["restart"] = {"ok": nc is not None, "error": why}
    if nc is not None:
        t1 = time.time()
        first_read = None
        while first_read is None and time.time() - t1 < CRAWL_RELAUNCH_READ_S:
            first_read = crawl_progress(nc)
            if first_read is None:
                time.sleep(0.2)
        answer["after_relaunch"] = first_read
    return answer


def crawl_limit():
    sys.path.insert(0, os.path.join(ROOT, "journeys"))
    try:
        from lib.fixture import const  # noqa: E402
        return int(const("BODY_PREFETCH_BATCH"))
    except Exception:       # noqa: BLE001  the ledger is unreadable: the case is NOT JUDGED
        return None


def judge_crawl(answer, server, limit):
    """[(name, ok, detail)] of the unpaused run. `server`: {"fetches": [[conn, folder, [uids]], ...], "max_sessions": n} as the fake recorded them (this process); `limit`: BODY_PREFETCH_BATCH."""
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import realcore_mailfake as fake
    errors = answer.get("driver_errors") or []
    total = answer.get("total") or sum(n for _, n in fake.CRAWL_PLAN)
    prog = answer.get("progress")
    fetches = [(f[0], f[1], list(f[2])) for f in server.get("fetches") or []]
    out = []
    # 1: every body of every folder
    if prog is None:
        out.append((CRAWL_CASES[0], None, "`core state reply/progress` was never served (driver %r)" % errors))
    else:
        seen = {}
        for _, folder, uids in fetches:
            seen.setdefault(folder, set()).update(uids)
        wire = {folder: sorted(set(range(1, n + 1)) - seen.get(folder, set())) for folder, n in fake.CRAWL_PLAN}
        ok = prog["messages"] == total and prog["bodies"] == total and all(not v for v in wire.values())
        out.append((CRAWL_CASES[0], ok, "the engine stores %d bodies of %d messages (wanted %d of %d) after %ss; folders [messages, bodies, indexed] %r; uids never fetched whole, per folder %r" % (
            prog["bodies"], prog["messages"], total, total, answer.get("progress_seconds"), prog["folders"], {f: len(v) for f, v in wire.items()})))
    # 2: INBOX first, newest first
    if not fetches:
        out.append((CRAWL_CASES[1], None if prog is None else False, "the server saw no whole-message fetch at all (%d body fetches recorded)" % len(fetches)))
    else:
        inbox_tops = [max(u) for _, f, u in fetches if f == "INBOX" and u]
        first_other = next((i for i, (_, f, _) in enumerate(fetches) if f != "INBOX"), None)
        last_inbox = max((i for i, (_, f, _) in enumerate(fetches) if f == "INBOX"), default=-1)
        ok = fetches[0][1] == "INBOX" and all(a > b for a, b in zip(inbox_tops, inbox_tops[1:])) and (first_other is None or first_other > last_inbox)
        out.append((CRAWL_CASES[1], ok, "%d whole-message fetches; the first is on %r (wanted INBOX); the highest uid of each INBOX fetch %r (each below the last); INBOX finished before the other folders: %s" % (
            len(fetches), fetches[0][1], inbox_tops, first_other is None or first_other > last_inbox)))
    # 3: batches
    if limit is None:
        out.append((CRAWL_CASES[2], None, "the ledger's BODY_PREFETCH_BATCH could not be read"))
    elif not fetches:
        out.append((CRAWL_CASES[2], None if prog is None else False, "the server saw no whole-message fetch at all"))
    else:
        biggest = max(len(u) for _, _, u in fetches)
        inbox_fetches = sum(1 for _, f, _ in fetches if f == "INBOX")
        out.append((CRAWL_CASES[2], biggest <= limit and inbox_fetches >= 2, "%d fetches, the largest asked for %d uids (BODY_PREFETCH_BATCH %d); INBOX holds %d mails and took %d fetches (more than one)" % (
            len(fetches), biggest, limit, dict(fake.CRAWL_PLAN)["INBOX"], inbox_fetches)))
    # 4: one connection
    most = server.get("max_sessions")
    peak = server.get("peak") or []     # fix-realcore: the connections open at the peak, each with the kinds of command it sent (no mail text): names the extra session
    out.append((CRAWL_CASES[3], None if most is None else most <= CRAWL_STREAM_MAX, "at most %r IMAP sessions were open at once (limit %d: the sync's IDLE session and one for the bodies)%s" % (
        most, CRAWL_STREAM_MAX, "" if most is None or most <= CRAWL_STREAM_MAX else "; open at the peak: " + " | ".join("connection %s: %s" % (c, ", ".join(k)) for c, k in peak))))
    # 5: relaunch
    after = answer.get("after_relaunch")
    rs = answer.get("restart")
    if rs is None or not rs.get("ok"):
        out.append((CRAWL_CASES[4], None, "the copy did not come back: %s" % (rs or {}).get("error")))
    elif after is None:
        out.append((CRAWL_CASES[4], None, "`core state reply/progress` was not served within %ss of the relaunch" % CRAWL_RELAUNCH_READ_S))
    else:
        out.append((CRAWL_CASES[4], after["bodies"] == total and prog is not None and prog["bodies"] == total,
                    "read once at the first moment after the relaunch: %d bodies stored of %d messages (wanted %d, as before the relaunch: %r)" % (after["bodies"], after["messages"], total, (prog or {}).get("bodies"))))
    # 6: found by a word
    w = answer.get("word")
    if w is None:
        out.append((CRAWL_CASES[5], None, "the word search was not run: %s" % errors))
    else:
        found = [h for h in w["hits"] if h["subject"] == fake.CRAWL_WORD_SUBJECT and h["matched_in"] not in CRAWL_MEANING_KINDS]
        out.append((CRAWL_CASES[5], bool(found), "a word that is in the body of one Archive mail only (no subject, no other mail): stage %r, %r hits, that mail among the word hits: %s, after %ss" % (w["stage"], w["count"], bool(found), w["waited"])))
    # 7 and 8: the model
    if not answer.get("weights_given"):
        why = "no model folder was given (WHOOSH_EMBED_WEIGHTS): nothing is embedded without a model, so a meaning ranking and an indexed count cannot be judged; pass it to the check to give the copy one"
        out.append((CRAWL_CASES[6], None, why))
        out.append((CRAWL_CASES[7], None, why))
    else:
        m = answer.get("meaning")
        if m is None:
            out.append((CRAWL_CASES[6], None, "the meaning search was not run: %s" % errors))
        else:
            group = [h for h in m["hits"] if h["matched_in"] in CRAWL_MEANING_KINDS]
            first = group[0]["subject"] if group else None
            place = next((i for i, h in enumerate(m["hits"]) if h["subject"] == fake.CRAWL_MEANING_SUBJECT), None)
            # fix-realcore round 8 (main103/104 s2-003, main105 again-019: the harbour mail ranked first, "related", in 0.3 s while its body was never
            # fetched, Archive [65, 50, 50]): a mail whose body is not on this Mac is ranked from its preview and title alone (the crawl's padding), so its
            # place is noise between near-equal mails. The case is about a CRAWLED mail: it holds only when every body is stored (the engine's own count,
            # read before the search, the same figure every-body-of-every-folder-is-stored reads), then the harbour mail must lead the meaning group.
            crawled = prog is not None and prog.get("bodies") == total
            out.append((CRAWL_CASES[6], crawled and m["stage"] == "ranked" and first == fake.CRAWL_MEANING_SUBJECT,
                        "a query that shares no word with any mail: every body stored first: %s (%s of %d); stage %r, the first mail of the meaning group (related, then meaning) is the harbour mail (an Archive mail, crawled): %s; "
                        "the harbour mail is hit %s of %d, matched_in %r; %d related, %d word hits; after %ss" % (
                            crawled, None if prog is None else prog.get("bodies"), total,
                            m["stage"], first == fake.CRAWL_MEANING_SUBJECT, "none" if place is None else place + 1, len(m["hits"]),
                            None if place is None else m["hits"][place]["matched_in"], sum(1 for h in m["hits"] if h["matched_in"] == "related"),
                            sum(1 for h in m["hits"] if h["matched_in"] not in CRAWL_MEANING_KINDS), m["waited"])))
        ix = answer.get("indexed")
        if prog is None:
            out.append((CRAWL_CASES[7], None, "`core state reply/progress` was never served"))
        elif ix is None:
            out.append((CRAWL_CASES[7], False, "the indexed count was never read within %ss" % SEARCH_SETTLE_S))
        else:
            out.append((CRAWL_CASES[7], ix["indexed"] >= ix["bodies"] == total, "indexed %d of %d stored bodies of %d messages after %ss; folders [messages, bodies, indexed] %r" % (ix["indexed"], ix["bodies"], total, answer.get("indexed_seconds"), ix["folders"])))
    out.append(("the-driver-reached-every-step", not errors, "driver errors %r" % errors))
    return out


def judge_crawl_paused(answer, server):
    """The held crawl (WHOOSH_CORE_CHECK_CONDITIONS): in the window the server saw no whole-message fetch of another folder and the engine had stored fewer bodies than messages."""
    prog = answer.get("progress")
    total = answer.get("total")
    fetches = [(f[0], f[1], list(f[2])) for f in server.get("fetches") or []]
    other = [f for _, f, _ in fetches if f != "INBOX"]
    bg = answer.get("background") or {}
    note = "the engine's own pause (background/pause) %r; the shell's own row says crawl %r, reason %r, conditions it sent %r%% (read, not judged: it shows what the shell told the engine)" % (
        answer.get("engine_pause"), bg.get("crawl"), bg.get("reason"), bg.get("sent_battery_percent"))
    if prog is None:
        return [(CRAWL_PAUSED_CASES[0], None, "`core state reply/progress` was never served"), (CRAWL_PAUSED_CASES[1], None, "`core state reply/progress` was never served")]
    return [(CRAWL_PAUSED_CASES[0], not other, "%d whole-message fetches in %ss, %d on a folder other than INBOX; %s" % (len(fetches), CRAWL_PAUSE_WINDOW_S, len(other), note)),
            (CRAWL_PAUSED_CASES[1], prog["bodies"] < total, "%d bodies stored of %d mails in all (the unpaused run stores all %d); folders [messages, bodies, indexed] %r" % (prog["bodies"], total, total, prog["folders"]))]


def main_slice_crawl(bundle, mutant, mutant_name, paused):
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import realcore_mailfake as fake
    try:
        mail, imap, smtp = fake.start(0, 0, inbox=0, act=True)
    except OSError as e:
        print("the local servers did not start: %s" % e)
        print("1 checks, 0 failed, 1 not judged"); return 1
    fake.load_crawl_mail(mail)
    ports = "%d:%d" % (imap.server_address[1], smtp.server_address[1])
    weights = os.environ.get("WHOOSH_EMBED_WEIGHTS", "")     # the lead's model folder, only when the lead gives one
    refused = weights_refusal(weights)      # fix-realcore round 4 (183fe10a): never a copy on a symbolic-link model folder
    if refused:
        imap.shutdown(); smtp.shutdown()
        print("FAIL       %s  %s" % (WEIGHTS_CASE, refused))
        print("1 checks, 1 failed, 0 not judged"); return 1
    extra = ["WHOOSH_CORE_PLAIN_LOOPBACK=1", "WHOOSH_CORE_CHECK_SERVERS=" + ports]
    if weights:
        extra.append("WHOOSH_EMBED_WEIGHTS=" + weights)
    if mutant:
        extra.append("WHOOSH_CORE_MUTANT=" + mutant_name)
    if paused:
        extra.append("WHOOSH_CORE_CHECK_CONDITIONS=" + paused)
    answer, out, r = run_inner(bundle, extra, "--slice-crawl" + (" --paused" if paused else ""))
    with mail.lock:
        server = {"fetches": [list(f) for f in mail.body_fetches or []], "max_sessions": mail.max_sessions, "peak": fake.peak_sessions(mail.conn_spans or {})[1]}
    imap.shutdown(); smtp.shutdown()
    if answer is None:
        return no_answer(out, r)
    answer["weights_given"] = bool(weights)
    print("launcher exit %d; the local server saw: %s; %d whole-message fetches; model folder %s; %s" % (r.returncode, sorted({l.split(" ")[0] for l in mail.log}), len(server["fetches"]),
          "given by the lead" if weights else "NOT given", next((l for l in out.splitlines() if l.startswith("instances:")), "")))
    if paused:
        results = judge_crawl_paused(answer, server)
        lines, rc = slice_lines(results, note="HELD-CRAWL: the rig put the engine on %s (WHOOSH_CORE_CHECK_CONDITIONS); the same slice run plain is the control: it stores every body" % paused)
    else:
        results = judge_crawl(answer, server, crawl_limit())
        lines, rc = slice_lines(results, mutant_name if mutant else None, crawl_mutant_red(mutant_name, bool(weights)),
                                "the crawl stored nothing, so the stored count, the relaunch read and the word search went red", "bodies were stored and found by a crawl that never runs")
    print("\n".join(lines)); return rc


def crawl_answer(mutant=None, model=True, paused=False):
    """The self-test's stand-in for the app and the server: what a correct engine and a correct server show (or one defect), as (answer, server)."""
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import realcore_mailfake as fake
    plan = dict(fake.CRAWL_PLAN)
    total = sum(plan.values())
    fetches = []
    conn = 2
    for folder in ("INBOX", "Archive", "Sent"):
        n = plan[folder]
        top = n
        while top >= 1:
            lo = max(1, top - 9)
            fetches.append([conn, folder, list(range(lo, top + 1))])
            top = lo - 1
    stored = {f: [plan[f], plan[f], plan[f] if model else 0] for f in plan}
    prog = {"messages": total, "bodies": total, "indexed": total if model else 0, "folders": dict(stored)}
    ans = {"driver_errors": [], "core": "rust", "total": total, "progress": prog, "progress_seconds": 6.0, "weights_given": model, "restart": {"ok": True, "error": None},
           "after_relaunch": dict(prog), "word": {"stage": "ranked", "count": 3, "waited": 2.0, "hits": [{"matched_in": "body", "subject": fake.CRAWL_WORD_SUBJECT}]}}
    if model:
        # the harbour mail stands out by meaning: the engine marks it related (SPEC A150), the rest follow as meaning
        ans["meaning"] = {"stage": "ranked", "count": 50, "waited": 9.0, "hits": [{"matched_in": "related", "subject": fake.CRAWL_MEANING_SUBJECT}, {"matched_in": "meaning", "subject": "Archive note 3"}]}
        ans["indexed"], ans["indexed_seconds"] = dict(prog), 20.0
    server = {"fetches": fetches, "max_sessions": 2}
    if mutant == "crawl-never-runs":     # fix-realcore round 2: what main95 s1-003 showed: the prefetch still stores the newest 50 of each folder, in batches
        kept = {f: min(plan[f], 50) for f in plan}
        prog["bodies"] = sum(kept.values()); prog["folders"] = {f: [plan[f], kept[f], kept[f] if model else 0] for f in plan}
        prog["indexed"] = prog["bodies"] if model else 0
        ans["progress"] = prog; ans["after_relaunch"] = dict(prog)
        ans["word"] = {"stage": "ranked", "count": 85, "waited": 90.0, "hits": [{"matched_in": "meaning", "subject": "Archive note 40"}]}
        server["fetches"] = [fe for fe in fetches if max(fe[2]) > plan[fe[1]] - 50]
        if model:      # main105 again-019: the harbour mail can come first by noise with its body never stored: the case must still be red
            ans["meaning"] = {"stage": "ranked", "count": 85, "waited": 0.3, "hits": [{"matched_in": "related", "subject": fake.CRAWL_MEANING_SUBJECT}]}
            ans["indexed"] = dict(prog)
    if mutant == "everything-in-one-fetch":
        server["fetches"] = [[2, f, list(range(1, plan[f] + 1))] for f in ("INBOX", "Archive", "Sent")]
    if mutant == "oldest-first":
        server["fetches"] = [[2, "INBOX", list(range(1, 11))], [2, "INBOX", list(range(11, 21))], [2, "INBOX", list(range(21, 26))]] + fetches[3:]
    if mutant == "other-folder-first":
        server["fetches"] = fetches[3:] + fetches[:3]
    if mutant == "two-streams":
        server["max_sessions"] = 5
    if mutant == "not-kept-after-relaunch":
        ans["after_relaunch"] = dict(prog, bodies=0, folders={f: [plan[f], 0, 0] for f in plan})
    if mutant == "word-not-found":
        ans["word"] = {"stage": "ranked", "count": 0, "waited": 90.0, "hits": []}
    if mutant == "word-found-by-meaning-only" :
        ans["word"] = {"stage": "ranked", "count": 50, "waited": 5.0, "hits": [{"matched_in": "meaning", "subject": fake.CRAWL_WORD_SUBJECT}]}
    if mutant == "word-found-as-related-only":     # main92 s1-008.log: with the word index never written, the mail came back related by meaning
        ans["word"] = {"stage": "ranked", "count": 50, "waited": 5.0, "hits": [{"matched_in": "related", "subject": fake.CRAWL_WORD_SUBJECT}]}
    if mutant == "meaning-first-only-as-meaning" and model:     # the harbour mail listed after a related decoy
        ans["meaning"]["hits"] = [{"matched_in": "related", "subject": "Archive note 3"}, {"matched_in": "meaning", "subject": fake.CRAWL_MEANING_SUBJECT}]
    if mutant == "meaning-first-is-a-decoy" and model:
        ans["meaning"]["hits"][0]["subject"] = "Archive note 3"
    if mutant == "index-stuck" and model:
        ans["indexed"] = dict(prog, indexed=12, folders={f: [plan[f], plan[f], 0] for f in plan})
    if mutant == "no-progress-path":
        ans["progress"] = None; ans["after_relaunch"] = None; ans["indexed"] = None
    if mutant == "no-restart":
        ans["restart"] = {"ok": False, "error": "no socket after the relaunch"}; ans.pop("after_relaunch")
    if paused:
        held = {"messages": 25, "bodies": 0, "indexed": 0, "folders": {"INBOX": [25, 0, 0]}}
        ans = {"driver_errors": [], "total": total, "progress": held, "background": {"crawl": "paused", "reason": "battery", "sent_battery_percent": 15}}
        server = {"fetches": [], "max_sessions": 2}
        if mutant == "crawl-runs":
            ans["progress"] = dict(prog); server["fetches"] = fetches
        if mutant == "other-folder-fetched":
            server["fetches"] = [[2, "Archive", [1, 2, 3]]]
        if mutant == "no-progress-path":
            ans["progress"] = None
    return ans, server


CRAWL_SELFTEST_MUTANTS = {   # defect -> the cases that must go red
    "crawl-never-runs": crawl_mutant_red("crawl-never-runs", True),
    "everything-in-one-fetch": (CRAWL_CASES[2],),
    "oldest-first": (CRAWL_CASES[1],),
    "other-folder-first": (CRAWL_CASES[1],),
    "two-streams": (CRAWL_CASES[3],),
    "not-kept-after-relaunch": (CRAWL_CASES[4],),
    "word-not-found": (CRAWL_CASES[5],),
    "word-found-by-meaning-only": (CRAWL_CASES[5],),
    "word-found-as-related-only": (CRAWL_CASES[5],),
    "meaning-first-only-as-meaning": (CRAWL_CASES[6],),
    "meaning-first-is-a-decoy": (CRAWL_CASES[6],),
    "index-stuck": (CRAWL_CASES[7],),
}
CRAWL_PAUSED_SELFTEST_MUTANTS = {"crawl-runs": CRAWL_PAUSED_CASES, "other-folder-fetched": (CRAWL_PAUSED_CASES[0],)}


def selftest_crawl():
    """The judge against a model of a correct engine and server (every case passes; with no model folder the two model cases are NOT JUDGED), one broken model per case (each case red, only the
    cases the defect touches), a path the build does not serve and a copy that does not come back (NOT JUDGED), and the held crawl. The fixture mailbox is read back by realcore_mailfake's own self-test."""
    verdict = lambda a, s: {n: ok for n, ok, _ in judge_crawl(a, s, 10)}
    ans, srv = crawl_answer()
    good = verdict(ans, srv)
    ok_good = list(good) == list(CRAWL_CASES) + ["the-driver-reached-every-step"] and all(v is True for v in good.values())
    ans, srv = crawl_answer(model=False)
    nomodel = verdict(ans, srv)
    ok_nomodel = nomodel[CRAWL_CASES[6]] is None and nomodel[CRAWL_CASES[7]] is None and all(v is True for k, v in nomodel.items() if k not in CRAWL_CASES[6:8])
    survived, touched_bad = [], []
    for mutant, targets in CRAWL_SELFTEST_MUTANTS.items():
        a, s = crawl_answer(mutant)
        got = verdict(a, s)
        if not all(got[t] is False for t in targets):
            survived.append(mutant)
        touched = sorted(k for k, v in got.items() if v is False and k not in targets)
        if touched:
            touched_bad.append("%s also reddens %s" % (mutant, touched))
    a, s = crawl_answer("no-progress-path")
    nj = verdict(a, s)
    ok_nj = nj[CRAWL_CASES[0]] is None and nj[CRAWL_CASES[4]] is None and nj[CRAWL_CASES[7]] is None
    a, s = crawl_answer("no-restart")
    ok_norestart = verdict(a, s)[CRAWL_CASES[4]] is None
    covered = {t for ts in CRAWL_SELFTEST_MUTANTS.values() for t in ts}
    ok_cover = covered >= set(CRAWL_CASES)
    red = {}
    # fix-realcore round 2: each --mutant run on its model ends `KILLED by` exactly its cases, exit 0, with and without a model folder; the correct engine's run
    # under a mutant name SURVIVES (exit 1), and a red case the mutant does not explain is named (exit 1)
    model_of = {"crawl-never-runs": "crawl-never-runs", "bodies-never-stored": "crawl-never-runs", "index-never-writes": "word-found-as-related-only"}
    for name in CRAWL_MUTANTS:
        for model in (True, False):
            a, s = crawl_answer(model_of[name], model=model)
            line, rc = mutant_verdict(name, judge_crawl(a, s, 10), crawl_mutant_red(name, model))
            red["%s%s" % (name, "" if model else " (no model)")] = rc == 0 and line.startswith("mutant %s: KILLED by" % name)
    a, s = crawl_answer()
    red["a correct engine survives"] = mutant_verdict("crawl-never-runs", judge_crawl(a, s, 10), crawl_mutant_red("crawl-never-runs", True))[1] == 1
    a, s = crawl_answer("crawl-never-runs")
    a["restart"] = {"ok": True, "error": None}; s["max_sessions"] = 3
    line, rc = mutant_verdict("crawl-never-runs", judge_crawl(a, s, 10), crawl_mutant_red("crawl-never-runs", True))
    red["an unexplained red is named"] = rc == 1 and "ALSO RED" in line and CRAWL_CASES[3] in line
    # fix-realcore round 7: the printed run of a correct KILL reads clean (KILLED lines, "0 failed", exit 0); a survivor keeps its FAIL lines and exit 1
    a, s = crawl_answer("crawl-never-runs")
    kl, krc = slice_lines(judge_crawl(a, s, 10), "crawl-never-runs", crawl_mutant_red("crawl-never-runs", True))
    red["a KILL prints clean"] = krc == 0 and not any(l.startswith("FAIL") for l in kl) and kl[-1].startswith("%d checks, 0 failed" % len(CRAWL_CASES + ("x",))) \
        and sum(1 for l in kl if l.startswith("KILLED")) == len(crawl_mutant_red("crawl-never-runs", True))
    a, s = crawl_answer()
    sl, src = slice_lines(judge_crawl(a, s, 10), "crawl-never-runs", crawl_mutant_red("crawl-never-runs", True))
    red["a survivor exits 1"] = src == 1 and "SURVIVED" in sl[-2]
    pa, ps = crawl_answer(paused=True)
    pv = {n: ok for n, ok, _ in judge_crawl_paused(pa, ps)}
    ok_paused = list(pv) == list(CRAWL_PAUSED_CASES) and all(v is True for v in pv.values())
    for mutant, targets in CRAWL_PAUSED_SELFTEST_MUTANTS.items():
        a, s = crawl_answer(mutant, paused=True)
        got = {n: ok for n, ok, _ in judge_crawl_paused(a, s)}
        if not all(got[t] is False for t in targets):
            survived.append("paused:" + mutant)
    a, s = crawl_answer("no-progress-path", paused=True)
    ok_paused = ok_paused and all(ok is None for _, ok, _ in judge_crawl_paused(a, s))
    print("selftest slice crawl: model %s, a copy with no model folder %s, %d broken models %s, a path not served and a copy that does not come back %s, every case covered by a broken model %s, "
          "the --mutant runs' cases red on the model of a crawl that never runs %s, held crawl %s" % (
              "all pass" if ok_good else "FAILS", "two cases NOT JUDGED, the rest pass" if ok_nomodel else "WRONG", len(CRAWL_SELFTEST_MUTANTS),
              "each red on its own cases" if not survived and not touched_bad else "SURVIVE %s; %s" % (survived, touched_bad), "NOT JUDGED, never a pass" if ok_nj and ok_norestart else "WRONG",
              "yes" if ok_cover else "NO", "yes" if all(red.values()) else "NO %s" % red, "judged (both cases pass held, both red when it runs)" if ok_paused else "WRONG"))
    return ok_good and ok_nomodel and not survived and not touched_bad and ok_nj and ok_norestart and ok_cover and all(red.values()) and ok_paused


# ---- slice aikey (item ee20d7cc): a provider key is saved through the app's Settings, against a fake provider on 127.0.0.1, and the quick reply gets a draft written by it.
# The rig core routes the key providers' hosts to the local fake (WHOOSH_CORE_CHECK_PROVIDERS=<port>: core/crates/engine/src/rig.rs, ai_settings::route_key_providers_to_loopback); a build without that
# string is NOT asked anything (a pasted key would go to the real provider): the run says NOT JUDGED and launches nothing. The key is the fake's made-up stand-in (realcore_aifake.GOOD_KEY), typed
# into the field and never printed.
AIKEY_SUBJECT = "Table for four on Friday?"
AIKEY_CASES = ("account-added", "before-the-key-the-draft-is-refused-in-words", "the-key-is-saved-and-the-provider-reads-connected", "a-saved-key-gets-a-draft-written-by-the-provider",
               "the-provider-answered-a-completion-for-the-saved-key",
               "jev-test-on-the-real-engine-reads-connected", "jev-bad-key-reads-refused-in-words")      # the last two: fix-jevrig, Jev's key Test (the rig routes Jev's host to the same fake)
AIKEY_JEV_PATH = "/v1/systemone"                 # decision::adapters::jev::JEV_URL's path: the fake answers it in Jev's own shape (realcore_aifake.jev_post)
AIKEY_SEAM = "WHOOSH_CORE_CHECK_PROVIDERS"
AIK = "mac-settings-ai"
AIKEY_REPLY = "mac-main.quickreply.ai"          # checks/parity/par-quickreply.py AI
AIKEY_DRAFT_WAIT_S = 60                          # a check bound: ⌘J to the draft in the field, over the loopback fake
AIKEY_CHECK_WAIT_S = 30                          # a check bound: the provider check and the model list over the loopback fake
AIKEY_FAKE_ANSWER = re.compile(r"A reply from [^\s]+\.")   # realcore_aifake: a completion answers "A reply from <model>."


def aikey_bundle_has(bundle, needle):
    """True when `needle` is a string literal in the bundle's executables (the same reading as checks/firstrun-check.py bundle_has): how a seam the build must carry is told without launching it."""
    needle = needle.encode()
    for sub in ("Contents/MacOS", "Contents/Frameworks"):
        for root, _d, files in os.walk(os.path.join(bundle, sub)):
            for f in files:
                try:
                    with open(os.path.join(root, f), "rb") as fh:
                        tail = b""
                        while True:
                            chunk = fh.read(8 << 20)
                            if not chunk:
                                break
                            if needle in tail + chunk:
                                return True
                            tail = chunk[-len(needle):]
                except OSError:
                    continue
    return False


def aikey_mail(fake, mail):
    mail.boxes["INBOX"] = [(1, flow_raw("Pat <pat@example.test>", fake.ADDRESS, AIKEY_SUBJECT, "aikey-1", 1, body="Can you confirm the table for four on Friday at eight? Tell me if the time does not suit you."))]


def aikey_write(c):
    """⌘J in the open quick reply (Return when the instruction field is asked first), then until the reply field says `ready` or `failed`: what the engine served and the field shows."""
    c.key("⌘J")
    t0 = time.time()
    row = {}
    while time.time() - t0 < AIKEY_DRAFT_WAIT_S:
        rows = c.rows(AIKEY_REPLY)
        row = rows[0] if rows else {}
        if row.get("phase") == "asking":
            c.key("Enter")
        if row.get("phase") in ("ready", "failed"):
            break
        time.sleep(0.2)
    field = (c.rows(FIELD) or [{}])[0]
    thread = field.get("thread")
    draft = cv(c, "reply/draft/%s" % thread) if thread else None
    text = draft.get("text") if isinstance(draft, dict) else None
    return {"phase": row.get("phase"), "words_chars": len((row.get("words") or "").strip()), "mark": row.get("mark"), "thread": bool(thread), "draft_served": isinstance(draft, dict),
            "draft_is_the_providers": bool(text) and AIKEY_FAKE_ANSWER.fullmatch(text.strip()) is not None, "field_is_the_draft": bool(text) and (field.get("text") or "").strip() == text.strip(),
            "seconds": round(time.time() - t0, 1)}


def aikey_save(c, key):
    """Settings, AI, the Anthropic card: click the key field, type the key, Connect, wait for the check to end. The state, the models' count and the stored flag; never the key."""
    c.click("mac-palette.result.ai-settings")
    try:
        c.click(AIK + ".nav.providers")
    except Exception:       # noqa: BLE001  the pane may open on it already
        pass
    c.click(AIK + ".provider.anthropic.key"); c.type(key)
    c.click(AIK + ".provider.anthropic.connect")
    t0 = time.time()
    while time.time() - t0 < AIKEY_CHECK_WAIT_S:
        chk = c.rows(AIK + ".provider.anthropic.check")
        if chk and chk[0].get("state") == "idle":
            break
        time.sleep(0.3)
    row = (c.rows(AIK + ".provider.anthropic") or [{}])[0]
    return {"state": row.get("state"), "models": len(row.get("models") or []), "stored": cv(c, "provider_credential/anthropic/stored"), "seconds": round(time.time() - t0, 1)}


def aikey_jev(c, key):
    """Settings, AI, Decisions, the Jev key: click the field, type the key, Connect (saves, sends nothing: `ai_provider/jev/state` reads "not checked"), then Test once and wait until the
    core says "connected" or "refused". The core's state, the row's words and the core's refusal text length; never the key."""
    c.click(AIK + ".nav.decisions")
    c.click(AIK + ".decisions.jev.key.field"); c.type(key)
    c.click(AIK + ".decisions.jev.connect")
    t0 = time.time()
    while time.time() - t0 < AIKEY_CHECK_WAIT_S and cv(c, "ai_provider/jev/state") != "not checked":
        time.sleep(0.2)
    saved = cv(c, "ai_provider/jev/state")
    c.click(AIK + ".decisions.jev.test")
    while time.time() - t0 < AIKEY_CHECK_WAIT_S and cv(c, "ai_provider/jev/state") not in ("connected", "refused"):
        time.sleep(0.2)
    row = (c.rows(AIK + ".decisions.jev.key") or [{}])[0]
    return {"saved_state": saved, "state": cv(c, "ai_provider/jev/state"), "note_chars": len((row.get("note") or "").strip()), "refusal_chars": len((cv(c, "ai_provider/jev/refusal") or "").strip()),
            "seconds": round(time.time() - t0, 1)}


def drive_aikey():
    """Inside launch-one: add the local account (one mail), open its thread and the quick reply, ask for a draft with no key (refused in words), save the key in Settings, ask again."""
    sys.path.insert(0, os.path.join(ROOT, "journeys"))
    from lib.ctl import Ctl  # noqa: E402
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import realcore_mailfake as fake  # noqa: E402
    import realcore_aifake as aifake  # noqa: E402
    c = Ctl().connect()
    answer = {"driver_errors": [], "core": None}
    mv_add_account(c, fake, answer, 1)
    answer["core"] = c.uistate().get("core")

    def open_reply():
        for _ in range(4):                                  # back to the mail window from wherever Settings left the screen
            if c.uistate().get("surface") == "mac-main":
                break
            c.key("Escape"); time.sleep(0.3)
        if not c.rows(FIELD):
            c.key("Enter"); time.sleep(0.5)
            c.key("R")
        t0 = time.time()
        while time.time() - t0 < 10 and not c.rows(FIELD):
            time.sleep(0.2)
        if not c.rows(FIELD):
            raise RuntimeError("the quick reply never opened (no %s row)" % FIELD)
    drive_step(answer, "open the quick reply", open_reply)
    answer["before"] = drive_step(answer, "ask for a draft with no key", lambda: aikey_write(c))
    answer["key"] = drive_step(answer, "save the key in Settings", lambda: aikey_save(c, aifake.GOOD_KEY))
    drive_step(answer, "back to the quick reply", open_reply)
    answer["after"] = drive_step(answer, "ask for a draft with the key saved", lambda: aikey_write(c))
    answer["jev_good"] = drive_step(answer, "Jev: paste the accepted key and Test", lambda: aikey_jev(c, aifake.GOOD_KEY))
    answer["jev_bad"] = drive_step(answer, "Jev: paste the bad key and Test", lambda: aikey_jev(c, aifake.BAD_KEY))
    return answer


def judge_aikey(answer, seen):
    """[(name, ok, detail)]. `seen`: the fake provider's own record of the requests it got (path, provider, model, accepted: never a key), read in this process."""
    errors = answer.get("driver_errors") or []
    b, k, a = answer.get("before"), answer.get("key"), answer.get("after")
    out = [(AIKEY_CASES[0], answer.get("stage") in ("added", "summary"), "stage %r, core %r" % (answer.get("stage"), answer.get("core")))]
    if b is None:
        out.append((AIKEY_CASES[1], None, "the quick reply was not reached: %s" % errors))
    else:
        out.append((AIKEY_CASES[1], b.get("phase") == "failed" and b.get("words_chars", 0) > 0 and not b.get("draft_is_the_providers"),
                    "with no key saved: the reply field says %r with %d characters of words, a draft served: %s, the provider's words in it: %s, after %ss" % (b.get("phase"), b.get("words_chars", 0), b.get("draft_served"), b.get("draft_is_the_providers"), b.get("seconds"))))
    if k is None:
        out.append((AIKEY_CASES[2], None, "the Settings pane was not driven: %s" % errors))
    else:
        out.append((AIKEY_CASES[2], k.get("state") == "connected" and k.get("models", 0) > 0 and k.get("stored") is True,
                    "the Anthropic card reads %r with %d model(s) listed, `core state provider_credential/anthropic/stored` %r, after %ss" % (k.get("state"), k.get("models", 0), k.get("stored"), k.get("seconds"))))
    if a is None:
        out.append((AIKEY_CASES[3], None, "the draft was not asked for after the key: %s" % errors))
    else:
        out.append((AIKEY_CASES[3], a.get("phase") == "ready" and a.get("draft_is_the_providers") and a.get("field_is_the_draft"),
                    "after the key: the reply field says %r, `reply/draft/<thread>` served %s and is the text the provider answered %s, the field holds it %s, after %ss" % (
                        a.get("phase"), a.get("draft_served"), a.get("draft_is_the_providers"), a.get("field_is_the_draft"), a.get("seconds"))))
    done = [p for p in seen if str(p.get("path", "")).endswith(("/v1/messages", "/v1/responses"))]
    out.append((AIKEY_CASES[4], any(p.get("accepted") for p in done), "the fake provider got %d completion request(s), %d with the saved key accepted; all its requests: %d" % (len(done), sum(1 for p in done if p.get("accepted")), len(seen))))
    jev = [p for p in seen if p.get("path") == AIKEY_JEV_PATH]
    g, jb = answer.get("jev_good"), answer.get("jev_bad")
    if g is None:
        out.append((AIKEY_CASES[5], None, "the Jev key was not driven: %s" % errors))
    else:
        out.append((AIKEY_CASES[5], g.get("saved_state") == "not checked" and g.get("state") == "connected" and any(p.get("accepted") for p in jev),
                    "after Connect `ai_provider/jev/state` read %r (nothing sent); after Test it reads %r in %ss; the fake saw %d request(s) at %s, %d with the key accepted" % (
                        g.get("saved_state"), g.get("state"), g.get("seconds"), len(jev), AIKEY_JEV_PATH, sum(1 for p in jev if p.get("accepted")))))
    if jb is None:
        out.append((AIKEY_CASES[6], None, "the bad Jev key was not driven: %s" % errors))
    else:
        out.append((AIKEY_CASES[6], jb.get("state") == "refused" and jb.get("note_chars", 0) > 0 and jb.get("refusal_chars", 0) > 0 and any(not p.get("accepted") for p in jev),
                    "the bad key: `ai_provider/jev/state` reads %r, the row says %d characters of words, `ai_provider/jev/refusal` holds %d characters; the fake refused %d request(s)" % (
                        jb.get("state"), jb.get("note_chars", 0), jb.get("refusal_chars", 0), sum(1 for p in jev if not p.get("accepted")))))
    out.append(("the-driver-reached-every-step", not errors, "driver errors %r" % errors))
    return out


def main_slice_aikey(bundle):
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import realcore_mailfake as fake
    import realcore_aifake
    if not aikey_bundle_has(bundle, AIKEY_SEAM):
        print("NOT JUDGED  the-build-carries-the-providers-seam  the build has no %s (core/crates/engine/src/rig.rs): a pasted key would be sent to the real provider, so none is pasted and nothing was launched" % AIKEY_SEAM)
        print("0 checks, 0 failed, 1 not judged"); return 1
    try:
        mail, imap, smtp = fake.start(0, 0, inbox=0)
        provider = realcore_aifake.start(0)
    except OSError as e:
        print("the local servers did not start: %s" % e)
        print("1 checks, 0 failed, 1 not judged"); return 1
    aikey_mail(fake, mail)
    ports = "%d:%d" % (imap.server_address[1], smtp.server_address[1])
    extra = ["WHOOSH_CORE_PLAIN_LOOPBACK=1", "WHOOSH_CORE_CHECK_SERVERS=" + ports, "%s=%d" % (AIKEY_SEAM, provider.server_address[1])]
    answer, out, r = run_inner(bundle, extra, "--slice-aikey")
    imap.shutdown(); smtp.shutdown()
    seen = [dict(p) for p in getattr(provider, "provider_seen", [])] + [dict(p, provider="jev") for p in getattr(provider, "jev_seen", [])]
    provider.shutdown()
    if answer is None:
        return no_answer(out, r)
    print("launcher exit %d; the local mail server saw: %s; the fake provider saw %d request(s); %s" % (r.returncode, sorted({l.split(" ")[0] for l in mail.log}), len(seen), next((l for l in out.splitlines() if l.startswith("instances:")), "")))
    lines, rc = slice_lines(judge_aikey(answer, seen), note="LOOPBACK-PROVIDER: the key providers' hosts are routed to a fake on 127.0.0.1 (%s); the key is the fake's made-up stand-in and is never printed" % AIKEY_SEAM)
    print("\n".join(lines)); return rc


def aikey_answer(mutant=None):
    before = {"phase": "failed", "words_chars": 41, "mark": "0", "thread": True, "draft_served": False, "draft_is_the_providers": False, "field_is_the_draft": False, "seconds": 0.6}
    key = {"state": "connected", "models": 2, "stored": True, "seconds": 3.2}
    after = {"phase": "ready", "words_chars": 0, "mark": "1", "thread": True, "draft_served": True, "draft_is_the_providers": True, "field_is_the_draft": True, "seconds": 2.1}
    seen = [{"path": "/v1/models", "provider": "anthropic", "model": None, "accepted": True}, {"path": "/v1/messages", "provider": "anthropic", "model": "model-a", "accepted": True},
            {"path": AIKEY_JEV_PATH, "provider": "jev", "accepted": True}, {"path": AIKEY_JEV_PATH, "provider": "jev", "accepted": False}]
    jev_good = {"saved_state": "not checked", "state": "connected", "note_chars": 0, "refusal_chars": 0, "seconds": 0.4}
    jev_bad = {"saved_state": "not checked", "state": "refused", "note_chars": 31, "refusal_chars": 44, "seconds": 0.4}
    ans = {"stage": "added", "core": "rust", "driver_errors": [], "before": before, "key": key, "after": after, "jev_good": jev_good, "jev_bad": jev_bad}
    if mutant == "no-draft":
        ans["after"] = dict(after, phase="failed", words_chars=30, draft_served=False, draft_is_the_providers=False, field_is_the_draft=False)
        seen = seen[:1] + seen[2:]
    if mutant == "other-text":
        ans["after"] = dict(after, draft_is_the_providers=False)
    if mutant == "field-empty":
        ans["after"] = dict(after, field_is_the_draft=False)
    if mutant == "never-asked-the-provider":
        seen = seen[:1] + seen[2:]
    if mutant == "refused-key-used":
        seen = [p if p.get("provider") == "jev" else dict(p, accepted=False) for p in seen]
    if mutant == "key-not-connected":
        ans["key"] = dict(key, state="refused", models=0, stored=False)
    if mutant == "key-not-stored":
        ans["key"] = dict(key, stored=False)
    if mutant == "draft-before-the-key":
        ans["before"] = dict(before, phase="ready", words_chars=0, draft_served=True, draft_is_the_providers=True, field_is_the_draft=True)
    if mutant == "silent-refusal":
        ans["before"] = dict(before, words_chars=0)
    if mutant == "jev-accepted-request-missing":
        seen = [p for p in seen if not (p.get("provider") == "jev" and p.get("accepted"))]
    if mutant == "jev-state-stuck-checking":
        ans["jev_good"] = dict(jev_good, state="checking")
    if mutant == "jev-bad-key-reads-connected":
        ans["jev_bad"] = dict(jev_bad, state="connected")
    if mutant == "jev-refusal-has-no-words":
        ans["jev_bad"] = dict(jev_bad, note_chars=0, refusal_chars=0)
    if mutant == "jev-bad-key-never-sent":
        seen = [p for p in seen if not (p.get("provider") == "jev" and not p.get("accepted"))]
    if mutant == "no-settings":
        ans["jev_good"] = None; ans["jev_bad"] = None
        ans["key"] = None; ans["after"] = None; ans["driver_errors"] = ["save the key in Settings: Refused: no door"]
    return ans, seen


AIKEY_SELFTEST_MUTANTS = {
    "no-draft": (AIKEY_CASES[3], AIKEY_CASES[4]), "other-text": (AIKEY_CASES[3],), "field-empty": (AIKEY_CASES[3],), "never-asked-the-provider": (AIKEY_CASES[4],), "refused-key-used": (AIKEY_CASES[4],),
    "key-not-connected": (AIKEY_CASES[2],), "key-not-stored": (AIKEY_CASES[2],), "draft-before-the-key": (AIKEY_CASES[1],), "silent-refusal": (AIKEY_CASES[1],),
    "jev-accepted-request-missing": (AIKEY_CASES[5],), "jev-state-stuck-checking": (AIKEY_CASES[5],), "jev-bad-key-reads-connected": (AIKEY_CASES[6],),
    "jev-refusal-has-no-words": (AIKEY_CASES[6],), "jev-bad-key-never-sent": (AIKEY_CASES[6],)}


def selftest_aikey():
    verdict = lambda p: {n: ok for n, ok, _ in judge_aikey(*p)}
    good = verdict(aikey_answer())
    ok_good = list(good) == list(AIKEY_CASES) + ["the-driver-reached-every-step"] and all(v is True for v in good.values())
    survived, touched_bad = [], []
    for mutant, targets in AIKEY_SELFTEST_MUTANTS.items():
        got = verdict(aikey_answer(mutant))
        if not all(got[t] is False for t in targets):
            survived.append(mutant)
        touched = sorted(k for k, v in got.items() if v is False and k not in targets)
        if touched:
            touched_bad.append("%s also reddens %s" % (mutant, touched))
    ns = verdict(aikey_answer("no-settings"))
    ok_ns = ns[AIKEY_CASES[2]] is None and ns[AIKEY_CASES[3]] is None and ns["the-driver-reached-every-step"] is False
    covered = {t for ts in AIKEY_SELFTEST_MUTANTS.values() for t in ts}
    ok_cover = covered == set(AIKEY_CASES[1:])
    # the seam probe reads a literal in an executable's bytes (a fake bundle in a throw-away folder of the system's temporary area, removed after)
    import shutil
    import tempfile
    d = tempfile.mkdtemp(prefix="aikey-bundle-")
    try:
        os.makedirs(os.path.join(d, "Contents", "MacOS"))
        open(os.path.join(d, "Contents", "MacOS", "App"), "wb").write(b"\0" * 100 + AIKEY_SEAM.encode() + b"\0")
        has = aikey_bundle_has(d, AIKEY_SEAM)
        open(os.path.join(d, "Contents", "MacOS", "App"), "wb").write(b"\0" * 100)
        lacks = not aikey_bundle_has(d, AIKEY_SEAM)
    finally:
        shutil.rmtree(d, ignore_errors=True)
    print("selftest slice aikey: model %s, %d broken models %s, no Settings door %s, every case covered by a broken model %s, the seam probe %s" % (
        "all pass" if ok_good else "FAILS", len(AIKEY_SELFTEST_MUTANTS), "each red on its own cases" if not survived and not touched_bad else "SURVIVE %s; %s" % (survived, touched_bad),
        "NOT JUDGED, never a pass" if ok_ns else "WRONG", "yes" if ok_cover else "NO", "finds the seam and misses it when absent" if has and lacks else "WRONG"))
    return ok_good and not survived and not touched_bad and ok_ns and ok_cover and has and lacks


# ---- slice attachsend (fix-realcore round 3, lead task 1ed1447c, SPEC A160): the whole attachment path on the real engine, against the loopback fakes only.
# Two files (one with a non-ASCII name, one of 3 MB) are copied onto the rig's private pasteboard and pasted with ⌘V into the full composer's reply to the one
# INBOX mail (To and Subject filled by the reply, the text focused); the message is sent;
# the fake SMTP server's DATA must hold both, multipart/mixed, base64, the non-ASCII name as an RFC 2231 parameter, the bytes equal (sha256); the Sent copy
# APPENDed holds the same files; the same message, delivered back into INBOX by this process (its Message-ID prefixed, its Subject AS_SUBJECT and its thread
# headers left out, so it is a mail and a list row of its own), is
# stored with both file rows (`message/<id>/files`); a double-click on each chip downloads it, and the file on this Mac (`attachment/<id>/local`) has
# the same sha256. `--mutant`: this process delivers the copy back with one base64 line of the big file removed: the rows' size and the download go red.
AS_PDF = "Offre Téléfonica été 2026.pdf"
AS_BIG = "scan-3mb.bin"
AS_BIG_BYTES = 3 * 1024 * 1024
AS_SUBJECT = "Two files back"      # the copy back's subject (as_back)
AS_BOARD = "io.nexus.whoosh.rig.attachments"   # Sources/Features/AttachmentsMac/AttachmentActs.swift AttachmentPasteboard: the rig copy pastes from this board only
AS_CASES = ("account-added", "pasted-files-become-two-ready-tiles", "smtp-message-is-multipart-mixed-with-base64-parts", "non-ascii-name-goes-as-rfc2231",
            "smtp-holds-both-files-with-equal-bytes", "sent-copy-appended-holds-both-files", "fetched-back-mail-shows-both-file-rows",
            "download-gives-equal-bytes", "the-driver-reached-every-step")
AS_MUTANT_NAME = "copy-back-cut"
AS_MUTANT_RED = ("fetched-back-mail-shows-both-file-rows", "download-gives-equal-bytes")
AS_SEND_WAIT_S = 60       # a check bound: the undo window (UNDO_SEND_DEFAULT_S) then the SMTP delivery
AS_BACK_WAIT_S = 90       # a check bound: the copy back is announced by IDLE, stored, its body fetched and its file rows written
AS_DOWNLOAD_WAIT_S = 60   # a check bound: each file fetched as one numbered part


def as_files():
    """The two files' bytes: a small PDF-like file with every byte class, and 3 MB of pseudo-random bytes (bare CR and LF included)."""
    import random
    pdf = b"%PDF-1.4\n% \xe2\xe3\xcf\xd3\nTelvora offer for the check\r\nendobj\n" + bytes(range(256))
    return {AS_PDF: pdf, AS_BIG: random.Random(7).randbytes(AS_BIG_BYTES)}


def nfc(name):
    """A file name in its composed form (macOS hands file names back decomposed, NFD): two spellings of one name compare equal."""
    import unicodedata
    return unicodedata.normalize("NFC", name) if isinstance(name, str) else name


def nfd_seen(names):
    """How many of `names` are not in the composed form (a fact for the detail line)."""
    import unicodedata
    return sum(1 for n in names if isinstance(n, str) and not unicodedata.is_normalized("NFC", n))


def as_sha(b):
    import hashlib
    return hashlib.sha256(b).hexdigest()


def as_mime(raw):
    """What a raw message carries: content type, and per attachment (name, sha256, size, transfer encoding, the raw Content-Disposition text)."""
    import email
    import email.policy
    m = email.message_from_bytes(raw, policy=email.policy.default)
    out = []
    for p in m.iter_attachments():
        data = p.get_payload(decode=True) or b""
        out.append({"name": nfc(p.get_filename()), "raw_name_decomposed": nfd_seen([p.get_filename()]) > 0, "sha": as_sha(data), "size": len(data), "cte": (p.get("Content-Transfer-Encoding") or "").strip().lower(),
                    "disposition": str(p.get("Content-Disposition", ""))})
    return {"type": m.get_content_type(), "files": out, "raw_dispositions": re.findall(r"(?im)^content-disposition:[^\r\n]*(?:\r?\n[ \t][^\r\n]*)*", raw.decode(errors="replace"))}


def as_stage(paths):
    """Puts file URLs on the rig's private pasteboard, as a person copying files in Finder would (JXA, this Mac's own pasteboard server; nothing else)."""
    script = ('function run(argv) { ObjC.import("AppKit"); var pb = $.NSPasteboard.pasteboardWithName("%s"); pb.clearContents;'
              ' var a = $.NSMutableArray.alloc.init; argv.forEach(function (p) { a.addObject($.NSURL.fileURLWithPath(p)); });'
              ' return pb.writeObjects(a) ? "ok" : "refused"; }' % AS_BOARD)
    r = subprocess.run(["osascript", "-l", "JavaScript", "-e", script] + list(paths), capture_output=True, text=True, timeout=30)
    return (r.stdout + r.stderr).strip()


def drive_attachsend():
    """Inside launch-one: add the account, a new message with the two files pasted, send it, then the copy back: its rows, and each file downloaded."""
    sys.path.insert(0, os.path.join(ROOT, "journeys"))
    from lib.ctl import Ctl  # noqa: E402
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import realcore_mailfake as fake  # noqa: E402
    c = Ctl().connect()
    answer = {"driver_errors": [], "core": None}
    folder = os.environ.get("WHOOSH_CHECK_ATTACH_DIR", "")
    paths = [os.path.join(folder, n) for n in (AS_PDF, AS_BIG)]
    mv_add_account(c, fake, answer, 1)
    answer["core"] = c.uistate().get("core")
    answer["staged"] = drive_step(answer, "stage the pasteboard", lambda: as_stage(paths))
    drive_step(answer, "open the mail", lambda: c.key("Enter"))
    time.sleep(0.6)
    drive_step(answer, "close the reply field", lambda: c.key("Escape"))
    drive_step(answer, "the full composer's reply", lambda: c.click("mac-main.reader.open-full-composer"))
    time.sleep(0.8)
    drive_step(answer, "text", lambda: c.type("Both files attached. "))
    drive_step(answer, "⌘V", lambda: c.key("⌘V"))
    tiles, secs = mv_poll(lambda: c.rows("mac-composer.attachments"), lambda r: len(r or []) >= 2 and all((x.get("state") or "") not in ("adding", "pending") for x in r), 20)
    answer["tiles"] = {"names": [t.get("name") for t in tiles or []], "states": [t.get("state") for t in tiles or []], "seconds": secs}
    drive_step(answer, "⌘Enter sends", lambda: c.key("⌘Enter"))
    # the copy back: the row with the subject in INBOX, its files, then each one downloaded by a double-click on its chip
    row, secs = mv_poll(lambda: next((r for r in c.rows("mac-main.list") if r.get("subject") == AS_SUBJECT), None), lambda r: r is not None, AS_SEND_WAIT_S + AS_BACK_WAIT_S)
    answer["back_row_seconds"] = secs
    if row is None:
        answer["driver_errors"].append("the copy back never reached the list")
        return answer
    mid = row.get("id")
    files, secs = mv_poll(lambda: cv(c, "message/%s/files" % mid), lambda v: isinstance(v, list) and len([f for f in v if not f.get("inline")]) >= 2, AS_BACK_WAIT_S)
    answer["files"] = [f for f in files or [] if not f.get("inline")] if isinstance(files, list) else None
    answer["files_seconds"] = secs
    drive_step(answer, "focus the copy back", lambda: c.click("mac-main.attachments.dev.focus.%s" % mid))
    drive_step(answer, "open it", lambda: c.key("Enter"))
    time.sleep(0.8)
    drive_step(answer, "close the reply field", lambda: c.key("Escape"))
    chips, _ = mv_poll(lambda: c.rows("mac-main.attachments"), lambda r: len(r or []) >= 2, 20)
    names = [x.get("name") for x in chips or []]
    answer["chips"] = names
    answer["downloads"] = {}
    for f in answer["files"] or []:
        if f.get("file_name") not in names:
            answer["driver_errors"].append("no chip for %r" % f.get("file_name"))
            continue
        i = names.index(f["file_name"])
        drive_step(answer, "double-click %s" % f["file_name"], lambda i=i: c.click("mac-main.attachments.%d.double" % i))
        here, secs = mv_poll(lambda: cv(c, "attachment/%s/local" % f["id"]), lambda v: isinstance(v, dict) and v.get("path"), AS_DOWNLOAD_WAIT_S)
        got = {"seconds": secs, "transfer": mv_tstate(c, f["id"])}
        if isinstance(here, dict) and here.get("path"):
            try:
                with open(here["path"], "rb") as fh:
                    data = fh.read()
                got.update(sha=as_sha(data), size=len(data))
            except OSError as e:
                got["error"] = str(e)
        answer["downloads"][f["file_name"]] = got
    return answer


def judge_attachsend(answer, server, want):
    """`want`: {name: (sha256, size)} of the two files as written. `server`: {"smtp": [raw...], "appends": [[box, raw]...]}. Counts, names and digests only."""
    errors = answer.get("driver_errors") or []
    out = [("account-added", answer.get("stage") in ("added", "summary"), "stage %r, core %r" % (answer.get("stage"), answer.get("core")))]
    tiles = answer.get("tiles") or {}
    out.append(("pasted-files-become-two-ready-tiles", sorted(nfc(n) for n in tiles.get("names") or []) == sorted(want) and bool(tiles.get("states")) and all(s == "ready" for s in tiles.get("states")),     # ComposerModel.FileTile: the core's draft holds the file
                "pasteboard %r; tiles %r states %r after %ss; names not in composed form %d" % (answer.get("staged"), tiles.get("names"), tiles.get("states"), tiles.get("seconds"), nfd_seen(tiles.get("names") or []))))
    sent = [m for m in server.get("smtp") or [] if AS_BIG.encode() in m]
    if not sent:
        why = "the SMTP server received no message carrying %s (%d messages in all)" % (AS_BIG, len(server.get("smtp") or []))
        out += [(n, None if not tiles.get("names") else False, why) for n in AS_CASES[2:6]]
    else:
        mime = as_mime(sent[-1])
        files = {f["name"]: f for f in mime["files"]}
        out.append(("smtp-message-is-multipart-mixed-with-base64-parts", mime["type"] == "multipart/mixed" and len(files) == 2 and all(f["cte"] == "base64" for f in files.values()),
                    "type %s, %d file parts, encodings %r" % (mime["type"], len(files), sorted(f["cte"] for f in files.values()))))
        rfc2231 = [d for d in mime["raw_dispositions"] if re.search(r"filename\*(?:0\*)?=", d)]
        out.append(("non-ascii-name-goes-as-rfc2231", AS_PDF in files and bool(rfc2231),
                    "the non-ASCII name read back %s (sent decomposed, NFD: %s); Content-Disposition with filename*= %d of %d" % (
                        AS_PDF in files, (files.get(AS_PDF) or {}).get("raw_name_decomposed"), len(rfc2231), len(mime["raw_dispositions"]))))
        same = {n: (n in files and files[n]["sha"] == want[n][0]) for n in want}
        out.append(("smtp-holds-both-files-with-equal-bytes", all(same.values()), "sha256 equal per file %r; sizes %r" % (same, {n: files.get(n, {}).get("size") for n in want})))
        sent_copy = [raw for box, raw in server.get("appends") or [] if box.lower().startswith("sent") and AS_BIG.encode() in raw]
        if sent_copy:
            cf = {f["name"]: f["sha"] for f in as_mime(sent_copy[-1])["files"]}
            out.append(("sent-copy-appended-holds-both-files", all(cf.get(n) == want[n][0] for n in want), "%d Sent copies; sha256 equal per file %r" % (len(sent_copy), {n: cf.get(n) == want[n][0] for n in want})))
        else:
            out.append(("sent-copy-appended-holds-both-files", False, "no APPEND to Sent carrying %s (%d APPENDs)" % (AS_BIG, len(server.get("appends") or []))))
    files = answer.get("files")
    if files is None:
        out.append(("fetched-back-mail-shows-both-file-rows", None if not sent else False, "the copy back's files were never read: %s" % errors))
        out.append(("download-gives-equal-bytes", None, "no file rows to download"))
    else:
        rows = {nfc(f.get("file_name")): f.get("size_bytes") for f in files}
        out.append(("fetched-back-mail-shows-both-file-rows", rows == {n: want[n][1] for n in want},
                    "rows %r after %ss (wanted %r)" % (rows, answer.get("files_seconds"), {n: want[n][1] for n in want})))
        dl = {nfc(k): v for k, v in (answer.get("downloads") or {}).items()}
        good = {n: (dl.get(n) or {}).get("sha") == want[n][0] for n in want}
        out.append(("download-gives-equal-bytes", all(good.values()), "sha256 equal per file %r; transfer %r; seconds %r" % (good, {n: (dl.get(n) or {}).get("transfer") for n in want}, {n: (dl.get(n) or {}).get("seconds") for n in want})))
    out.append(("the-driver-reached-every-step", not errors, "driver errors %r" % errors))
    return out


def as_back(raw, cut):
    """The message as this process delivers it back into INBOX: its Message-ID prefixed, its Subject AS_SUBJECT, no In-Reply-To or References (a mail and a
    list row of its own); with `cut`, one base64 line of the big file removed."""
    head, sep, body = raw.partition(b"\r\n\r\n")
    head = re.sub(rb"(?im)^(?:in-reply-to|references):[^\r\n]*(?:\r\n[ \t][^\r\n]*)*\r\n?", b"", head)
    head = re.sub(rb"(?im)^subject:[^\r\n]*(?:\r\n[ \t][^\r\n]*)*", b"Subject: " + AS_SUBJECT.encode(), head, count=1)
    back = re.sub(rb"(?im)^(message-id:\s*<)", rb"\1back-", head, count=1) + sep + body
    if cut:
        at = back.find(b"filename=\"%s\"" % AS_BIG.encode())
        at = back.find(b"\r\n\r\n", at) + 4 if at >= 0 else -1
        if at > 3:
            end = back.find(b"\r\n", at + 80 * 100)
            nxt = back.find(b"\r\n", end + 2)
            if end > 0 and nxt > end:
                back = back[:end] + back[nxt:]
    return back


def main_slice_attachsend(bundle, mutant):
    import json
    import shutil
    import tempfile
    import threading
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import realcore_mailfake as fake
    files = as_files()
    want = {n: (as_sha(b), len(b)) for n, b in files.items()}
    folder = tempfile.mkdtemp(prefix="whoosh-attachsend-")
    for n, b in files.items():
        with open(os.path.join(folder, n), "wb") as fh:
            fh.write(b)
    try:
        mail, imap, smtp = fake.start(0, 0, inbox=0, act=True)
    except OSError as e:
        shutil.rmtree(folder, ignore_errors=True)
        print("the local servers did not start: %s" % e)
        print("1 checks, 0 failed, 1 not judged"); return 1
    part_log = []
    put_back = mv_patch_fetch(fake, part_log)
    mail.boxes["INBOX"] = [(1, fake.plain_message())]
    done = threading.Event()

    def watch():      # the sent message comes back into INBOX once (a mail to oneself)
        while not done.is_set():
            got = [m for m in smtp.messages if AS_BIG.encode() in m]
            if got:
                fake.deliver(mail, as_back(got[-1], mutant))
                return
            time.sleep(0.3)
    watcher = threading.Thread(target=watch, daemon=True)
    watcher.start()
    ports = "%d:%d" % (imap.server_address[1], smtp.server_address[1])
    extra = ["WHOOSH_CORE_PLAIN_LOOPBACK=1", "WHOOSH_CORE_CHECK_SERVERS=" + ports, "WHOOSH_ATTACH_AUTO_FETCH=off"]
    env = dict(os.environ, WHOOSH_RIG_HIDDEN="1", LAUNCH_ENV=" ".join(["WHOOSH_RIG_HIDDEN=1"] + extra), WHOOSH_CHECK_ATTACH_DIR=folder)
    env.pop("WHOOSH_FIXTURE_CORE", None)
    cmd = ["sh", os.path.join(ROOT, "scripts", "launch-one.sh"), bundle, "--", "--rig-hidden",
           "sh", "-c", 'WHOOSH_SOCKET_DIR="$D" python3 "$0" --inner --slice-attachsend', os.path.abspath(__file__)]
    try:
        r = launch(cmd, env)
    finally:
        done.set(); watcher.join(2)
        put_back()
        imap.shutdown(); smtp.shutdown()
        shutil.rmtree(folder, ignore_errors=True)
    out = r.stdout + r.stderr
    line = next((l for l in out.splitlines() if l.startswith("ANSWER ")), None)
    if line is None:
        print(out.rstrip()[-3000:])
        print("launcher exit %d" % r.returncode)
        print("1 checks, 0 failed, 1 not judged"); return 1
    answer = json.loads(line[len("ANSWER "):])
    print("launcher exit %d; the local server saw: %s; %d SMTP message(s), %d APPEND(s), %d part fetch(es); %s" % (
        r.returncode, sorted({l.split(" ")[0] for l in mail.log}), len(smtp.messages), len(mail.appends), len(part_log), next((l for l in out.splitlines() if l.startswith("instances:")), "")))
    sent_raw = [m for m in smtp.messages if AS_BIG.encode() in m]
    if sent_raw:      # the header bytes the wire held (the check's own invented file names), so a name's form is read, not guessed
        print("ATTACHSEND-WIRE: Content-Disposition as the SMTP server received it: %r" % (as_mime(sent_raw[-1])["raw_dispositions"],))
    results = judge_attachsend(answer, {"smtp": list(smtp.messages), "appends": [[b, raw] for b, _f, raw in mail.appends]}, want)
    lines, rc = slice_lines(results, AS_MUTANT_NAME if mutant else None, AS_MUTANT_RED, "the copy back lost a line of the big file: its row and its download read the change",
                            "a copy back with a line of the big file removed still read as the file sent")
    print("\n".join(lines)); return rc


def as_model(defect=None):
    """The self-test's stand-in: a correct engine and server (built with Python's own email package), or one defect. No app is launched."""
    import email.message
    import email.policy
    files = as_files()
    want = {n: (as_sha(b), len(b)) for n, b in files.items()}

    def message(names=(AS_PDF, AS_BIG), cte_text=False, plain_name=False):
        m = email.message.EmailMessage(policy=email.policy.SMTP)
        m["From"], m["To"], m["Subject"], m["Message-ID"] = CHECK_ADDRESS, "ann@friends.example", "Re: a question", "<as-1@whoosh-check.invalid>"
        m["In-Reply-To"], m["References"] = "<plain-1@friends.example>", "<plain-1@friends.example>"
        m.set_content("Both files attached.")
        for n in names:
            b = files[n] if not (defect == "smtp-bytes" and n == AS_BIG) else files[n][:-1]
            m.add_attachment(b, maintype="application", subtype="octet-stream", filename=n if not (plain_name and n == AS_PDF) else "Offre.pdf")
        raw = m.as_bytes()
        if cte_text:
            raw = raw.replace(b"Content-Transfer-Encoding: base64", b"Content-Transfer-Encoding: 8bit", 1)
        return raw
    raw = message(names=(AS_BIG,) if defect == "one-file" else (AS_PDF, AS_BIG), cte_text=defect == "not-base64", plain_name=defect == "ascii-name")
    if defect == "no-2231":
        raw = re.sub(rb"filename\*=[^\r\n]*", b"filename=\"x.pdf\"", raw)
    server = {"smtp": [] if defect == "nothing-sent" else [raw], "appends": [] if defect == "no-append" else [["Sent", raw]]}
    answer = {"stage": "added", "core": "rust", "driver_errors": [], "staged": "ok",
              "tiles": {"names": [AS_PDF, AS_BIG], "states": ["ready", "ready"], "seconds": 1.0},
              "files": [{"id": "m:att:000", "file_name": AS_PDF, "size_bytes": want[AS_PDF][1], "inline": False}, {"id": "m:att:001", "file_name": AS_BIG, "size_bytes": want[AS_BIG][1], "inline": False}],
              "files_seconds": 3.0, "downloads": {n: {"sha": want[n][0], "size": want[n][1], "seconds": 1.0, "transfer": "on_this_mac"} for n in want}}
    if defect == "no-tiles":
        answer["tiles"] = {"names": [], "states": [], "seconds": 20.0}
    if defect == "rows-size":
        answer["files"][1]["size_bytes"] -= 57
    if defect == "rows-missing":
        answer["files"] = answer["files"][:1]
    if defect == "download-bytes":
        answer["downloads"][AS_BIG]["sha"] = as_sha(b"other")
    if defect == "copy-back-cut":      # the --mutant run's model: the copy back lost a base64 line of the big file
        answer["files"][1]["size_bytes"] -= 57
        answer["downloads"][AS_BIG]["sha"] = as_sha(files[AS_BIG][:-57])
    if defect == "driver-error":
        answer["driver_errors"] = ["⌘V: Unreadable: no such key"]
    if defect == "decomposed":      # main100: macOS handed the name back decomposed; one name, two spellings: every case still passes
        import unicodedata
        d = lambda n: unicodedata.normalize("NFD", n)
        answer["tiles"]["names"] = [d(n) for n in answer["tiles"]["names"]]
        for f in answer["files"]:
            f["file_name"] = d(f["file_name"])
        answer["downloads"] = {d(k): v for k, v in answer["downloads"].items()}
    return answer, server, want


AS_SELFTEST_DEFECTS = {
    "no-tiles": ("pasted-files-become-two-ready-tiles",),
    "not-base64": ("smtp-message-is-multipart-mixed-with-base64-parts", "smtp-holds-both-files-with-equal-bytes", "sent-copy-appended-holds-both-files"),   # the part no longer decodes to the file (base64 read as text; the name lost)
    "one-file": ("smtp-message-is-multipart-mixed-with-base64-parts", "non-ascii-name-goes-as-rfc2231", "smtp-holds-both-files-with-equal-bytes", "sent-copy-appended-holds-both-files"),   # the non-ASCII file is the one left out
    "ascii-name": ("non-ascii-name-goes-as-rfc2231", "smtp-holds-both-files-with-equal-bytes", "sent-copy-appended-holds-both-files"),
    "no-2231": ("non-ascii-name-goes-as-rfc2231", "smtp-holds-both-files-with-equal-bytes", "sent-copy-appended-holds-both-files"),   # the part no longer decodes to the file (base64 read as text; the name lost)
    "smtp-bytes": ("smtp-holds-both-files-with-equal-bytes", "sent-copy-appended-holds-both-files"),
    "no-append": ("sent-copy-appended-holds-both-files",),
    "rows-size": ("fetched-back-mail-shows-both-file-rows",),
    "rows-missing": ("fetched-back-mail-shows-both-file-rows",),
    "download-bytes": ("download-gives-equal-bytes",),
    "driver-error": ("the-driver-reached-every-step",),
}


def selftest_attachsend():
    """The judge against a correct model (every case passes), one defect per case (each red, only on its own cases), nothing sent (the SMTP cases NOT JUDGED
    or red, never a pass), the --mutant run's model KILLED exactly by its cases; the as_back cut really removes a line; the pasteboard is not touched."""
    verdict = lambda d: {n: ok for n, ok, _ in judge_attachsend(*as_model(d))}
    good = verdict(None)
    ok_good = list(good) == list(AS_CASES) and all(v is True for v in good.values())
    survived, noisy = [], []
    for d, targets in AS_SELFTEST_DEFECTS.items():
        got = verdict(d)
        if not all(got[t] is False for t in targets):
            survived.append(d)
        extra = sorted(k for k, v in got.items() if v is False and k not in targets)
        if extra:
            noisy.append("%s also reddens %s" % (d, extra))
    decomposed = verdict("decomposed")
    ok_good = ok_good and all(v is True for v in decomposed.values())
    none_sent = verdict("nothing-sent")
    ok_nothing = all(none_sent[n] is not True for n in AS_CASES[2:6])
    a, s, w = as_model("copy-back-cut")
    killed = mutant_verdict(AS_MUTANT_NAME, judge_attachsend(a, s, w), AS_MUTANT_RED)
    a, s, w = as_model()
    survives = mutant_verdict(AS_MUTANT_NAME, judge_attachsend(a, s, w), AS_MUTANT_RED)
    raw = s["smtp"][0]
    cut = as_back(raw, True)
    plain = as_back(raw, False)
    ok_back = (b"Message-ID: <back-" in plain and b"Subject: " + AS_SUBJECT.encode() in plain and b"In-Reply-To" not in plain and b"References" not in plain
               and as_mime(plain)["files"] == as_mime(raw)["files"] and len(cut) < len(raw) and next(f for f in as_mime(cut)["files"] if f["name"] == AS_BIG)["size"] < AS_BIG_BYTES)
    covered = {t for ts in AS_SELFTEST_DEFECTS.values() for t in ts} | {"account-added"}
    ok_cover = covered == set(AS_CASES)
    print("selftest slice attachsend: model %s, %d defects %s, nothing sent %s, the --mutant model %s and a correct one %s, the copy back %s, every case covered %s" % (
        "all pass" if ok_good else "FAILS %r" % good, len(AS_SELFTEST_DEFECTS), "each red on its own cases" if not survived and not noisy else "SURVIVE %s; %s" % (survived, noisy),
        "never a pass" if ok_nothing else "WRONG", "KILLED" if killed[1] == 0 else "NOT KILLED: %s" % killed[0], "kept" if survives[1] == 1 else "WRONG",      # "kept", never the failure word SURVIVES (three proofs fail on it)
        "renamed and cut as meant" if ok_back else "WRONG", "yes" if ok_cover else "NO %s" % sorted(set(AS_CASES) - covered)))
    return ok_good and not survived and not noisy and ok_nothing and killed[1] == 0 and survives[1] == 1 and ok_back and ok_cover



# ---- slice bodygone (fix-realcore round 9, item 7e1fc235): a body the server never sends, on the real engine, against the loopback fake only.
# INBOX holds four mails. The "gone" one (uid BG_GONE) leaves the folder the moment its whole body is first asked for (as when another device
# deletes or moves it): every later ask gets no FETCH line and `UID SEARCH` no longer lists it. The "stuck" one (uid BG_STUCK) stays listed but
# its whole-body asks are answered with no body (the owner's case words). The run reads: the gone mail leaves the list, the server saw ONE
# existence check for it and no body ask after that check; the stuck mail keeps its row, its body was asked in ONE try (at most three asks: whole,
# alone, by sections) although the reader opened it twice, and `message/<id>/body_unavailable` names the reason.
BG_GONE, BG_STUCK = 2, 3
BG_SUBJECTS = {1: "Bodygone first", BG_GONE: "Bodygone leaves", BG_STUCK: "Bodygone stuck", 4: "Bodygone last"}
BG_CASES = ("account-added", "gone-mail-leaves-the-list", "gone-uid-checked-once", "gone-uid-never-asked-after-the-check",
            "stuck-mail-keeps-its-row", "stuck-uid-asked-in-one-try", "reader-says-why-the-stuck-mail-has-no-text", "the-driver-reached-every-step")
BG_WAIT_S = 90     # a check bound: the sync, the prefetch's three asks and the existence check


def bg_full_body(items):
    """A whole-body ask (the engine's three forms), not the sync's preview (`BODY.PEEK[TEXT]<0.n>`)."""
    up = items.upper()
    return "BODY.PEEK[]" in up or ("BODY.PEEK[TEXT]" in up and "BODY.PEEK[TEXT]<" not in up)


def bg_patch_fetch(fake, mail):
    """The gone mail leaves INBOX at its first whole-body ask (that ask and all later ones get no FETCH line); the stuck mail answers with no body."""
    original = fake.fetch_line

    def fetch_line(seq, uid, raw, items, flags=()):
        if bg_full_body(items) and uid == BG_GONE:
            with mail.lock:
                mail.boxes["INBOX"] = [(u, r) for u, r in mail.boxes["INBOX"] if u != BG_GONE]
            return b""
        if bg_full_body(items) and uid == BG_STUCK:
            return b"* %d FETCH (UID %d FLAGS ())\r\n" % (seq, uid)
        return original(seq, uid, raw, items, flags)
    fake.fetch_line = fetch_line
    return lambda: setattr(fake, "fetch_line", original)


def drive_bodygone():
    """Inside launch-one: add the account, wait for the gone mail to leave the list, open the stuck mail twice, read why it has no text."""
    sys.path.insert(0, os.path.join(ROOT, "journeys"))
    from lib.ctl import Ctl  # noqa: E402
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import realcore_mailfake as fake  # noqa: E402
    c = Ctl().connect()
    answer = {"driver_errors": [], "core": None}
    mv_add_account(c, fake, answer, 4)
    answer["core"] = c.uistate().get("core")
    subjects = lambda: [r.get("subject") for r in c.rows("mac-main.list")]
    answer["first_list"] = subjects()
    left, secs = mv_poll(subjects, lambda s: BG_SUBJECTS[BG_GONE] not in (s or []), BG_WAIT_S)
    answer["after_list"], answer["left_seconds"] = left, secs
    stuck = next((r for r in c.rows("mac-main.list") if r.get("subject") == BG_SUBJECTS[BG_STUCK]), None)
    if stuck is None:
        answer["driver_errors"].append("the stuck mail is not on the list")
        return answer
    sid = stuck.get("id")
    answer["stuck_id"] = sid
    for n in range(2):      # the reader opens it twice: a reader ask is a pass too
        drive_step(answer, "focus the stuck mail %d" % n, lambda: c.click("mac-main.attachments.dev.focus.%s" % sid))
        drive_step(answer, "open it %d" % n, lambda: c.key("Enter"))
        time.sleep(2.0)
        drive_step(answer, "close the reply field %d" % n, lambda: c.key("Escape"))
        drive_step(answer, "back to the list %d" % n, lambda: c.key("Escape"))
    why, secs = mv_poll(lambda: cv(c, "message/%s/body_unavailable" % sid), lambda v: isinstance(v, dict), 30)
    answer["why"], answer["why_seconds"] = why, secs
    answer["final_list"] = subjects()
    return answer


def bg_asks(log, uid):
    """(index of each whole-body UID FETCH naming `uid`, index of each `UID SEARCH UID <uid>`) in the fake's command log."""
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import realcore_mailfake as fake
    fetches, searches = [], []
    for i, line in enumerate(log):
        words = line.split(" ")
        if line.upper().startswith("UID FETCH ") and len(words) > 2 and uid in fake.uid_set(words[2], list(BG_SUBJECTS)) and bg_full_body(line):
            fetches.append(i)
        if re.match(r"(?i)^UID SEARCH UID %d$" % uid, line.strip()):
            searches.append(i)
    return fetches, searches


def judge_bodygone(answer, log):
    errors = answer.get("driver_errors") or []
    out = [("account-added", answer.get("stage") in ("added", "summary"), "stage %r, core %r" % (answer.get("stage"), answer.get("core")))]
    first, after = answer.get("first_list") or [], answer.get("after_list") or []
    out.append(("gone-mail-leaves-the-list", BG_SUBJECTS[BG_GONE] not in after and len(after) == 3,
                "first list %d rows (the gone mail on it: %s); after %ss %d rows, the gone mail on it: %s" % (
                    len(first), BG_SUBJECTS[BG_GONE] in first, answer.get("left_seconds"), len(after), BG_SUBJECTS[BG_GONE] in after)))
    gf, gs = bg_asks(log, BG_GONE)
    out.append(("gone-uid-checked-once", len(gs) == 1, "UID SEARCH UID %d lines: %d; whole-body asks of it: %d" % (BG_GONE, len(gs), len(gf))))
    late = [i for i in gf if gs and i > gs[0]]
    out.append(("gone-uid-never-asked-after-the-check", bool(gs) and not late, "whole-body asks after the check: %d (of %d)" % (len(late), len(gf))))
    final = answer.get("final_list") or []
    out.append(("stuck-mail-keeps-its-row", BG_SUBJECTS[BG_STUCK] in final, "the stuck mail on the final list: %s (%d rows)" % (BG_SUBJECTS[BG_STUCK] in final, len(final))))
    sf, _ = bg_asks(log, BG_STUCK)
    out.append(("stuck-uid-asked-in-one-try", 1 <= len(sf) <= 3, "whole-body asks of UID %d over the run (one try is at most 3: whole, alone, by sections): %d; the reader opened it twice" % (BG_STUCK, len(sf))))
    why = answer.get("why")
    out.append(("reader-says-why-the-stuck-mail-has-no-text", isinstance(why, dict) and why.get("code") == "body_not_sent" and why.get("tries") == 1 and why.get("stopped") is False,
                "message/<id>/body_unavailable %r after %ss" % (why, answer.get("why_seconds"))))
    out.append(("the-driver-reached-every-step", not errors, "driver errors %r" % errors))
    return out


def main_slice_bodygone(bundle):
    import json
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import realcore_mailfake as fake
    try:
        mail, imap, smtp = fake.start(0, 0, inbox=0)
    except OSError as e:
        print("the local servers did not start: %s" % e)
        print("1 checks, 0 failed, 1 not judged"); return 1
    mail.boxes["INBOX"] = [(u, flow_raw("Pat <pat@example.test>", fake.ADDRESS, s, "bodygone-%d" % u, u, body="Body of %s." % s.lower())) for u, s in sorted(BG_SUBJECTS.items())]
    put_back = bg_patch_fetch(fake, mail)
    ports = "%d:%d" % (imap.server_address[1], smtp.server_address[1])
    extra = ["WHOOSH_CORE_PLAIN_LOOPBACK=1", "WHOOSH_CORE_CHECK_SERVERS=" + ports, "WHOOSH_ATTACH_AUTO_FETCH=off"]
    env = dict(os.environ, WHOOSH_RIG_HIDDEN="1", LAUNCH_ENV=" ".join(["WHOOSH_RIG_HIDDEN=1"] + extra))
    env.pop("WHOOSH_FIXTURE_CORE", None)
    cmd = ["sh", os.path.join(ROOT, "scripts", "launch-one.sh"), bundle, "--", "--rig-hidden",
           "sh", "-c", 'WHOOSH_SOCKET_DIR="$D" python3 "$0" --inner --slice-bodygone', os.path.abspath(__file__)]
    try:
        r = launch(cmd, env)
    finally:
        put_back()
        imap.shutdown(); smtp.shutdown()
    out = r.stdout + r.stderr
    line = next((l for l in out.splitlines() if l.startswith("ANSWER ")), None)
    if line is None:
        print(out.rstrip()[-3000:])
        print("launcher exit %d" % r.returncode)
        print("1 checks, 0 failed, 1 not judged"); return 1
    answer = json.loads(line[len("ANSWER "):])
    with mail.lock:
        log = list(mail.log)
    print("launcher exit %d; the local server saw: %s; %s" % (r.returncode, sorted({l.split(" ")[0] for l in log}), next((l for l in out.splitlines() if l.startswith("instances:")), "")))
    lines, rc = slice_lines(judge_bodygone(answer, log))
    print("\n".join(lines)); return rc


def bg_model(defect=None):
    """The self-test's stand-in: a correct engine's answer and the server's log, or one defect. No app is launched."""
    rest = [BG_SUBJECTS[u] for u in (4, BG_STUCK, 1)]
    answer = {"stage": "added", "core": "rust", "driver_errors": [], "first_list": [BG_SUBJECTS[u] for u in (4, BG_STUCK, BG_GONE, 1)], "after_list": list(rest),
              "left_seconds": 4.0, "stuck_id": "m3", "why": {"code": "body_not_sent", "tries": 1, "stopped": False}, "why_seconds": 0.2, "final_list": list(rest)}
    log = ["SELECT INBOX", "UID FETCH 1:4 (UID FLAGS BODY.PEEK[TEXT]<0.2048>)", "UID FETCH 4,3,2,1 (UID RFC822.SIZE BODY.PEEK[])",
           "UID FETCH 3 (UID RFC822.SIZE BODY.PEEK[])", "UID FETCH 3 (UID RFC822.SIZE BODY.PEEK[HEADER] BODY.PEEK[TEXT])",
           "UID FETCH 2 (UID RFC822.SIZE BODY.PEEK[])", "UID FETCH 2 (UID RFC822.SIZE BODY.PEEK[HEADER] BODY.PEEK[TEXT])", "UID SEARCH UID 2", "IDLE"]
    if defect == "row-stays":
        answer["after_list"] = answer["final_list"] = [BG_SUBJECTS[u] for u in (4, BG_STUCK, BG_GONE, 1)]
    if defect == "no-check":
        log = [l for l in log if not l.startswith("UID SEARCH")]
    if defect == "checked-every-pass":
        log += ["UID SEARCH UID 2", "UID SEARCH UID 2"]
    if defect == "asked-after-the-check":
        log += ["UID FETCH 2 (UID RFC822.SIZE BODY.PEEK[])"]
    if defect == "stuck-row-removed":
        answer["final_list"] = [BG_SUBJECTS[u] for u in (4, 1)]
    if defect == "stuck-every-pass":
        log += ["UID FETCH 3 (UID RFC822.SIZE BODY.PEEK[])"] * 3
    if defect == "no-reason":
        answer["why"] = None
    if defect == "driver-error":
        answer["driver_errors"] = ["focus the stuck mail 0: Unreadable: no such door"]
    return answer, log


BG_SELFTEST_DEFECTS = {
    "row-stays": ("gone-mail-leaves-the-list",),
    "no-check": ("gone-uid-checked-once", "gone-uid-never-asked-after-the-check"),
    "checked-every-pass": ("gone-uid-checked-once",),
    "asked-after-the-check": ("gone-uid-never-asked-after-the-check",),
    "stuck-row-removed": ("stuck-mail-keeps-its-row",),
    "stuck-every-pass": ("stuck-uid-asked-in-one-try",),
    "no-reason": ("reader-says-why-the-stuck-mail-has-no-text",),
    "driver-error": ("the-driver-reached-every-step",),
}


def selftest_bodygone():
    """The judge on a correct model (every case passes), one defect per case (each red on its own cases); the fetch patch hides the gone mail and the
    stuck body, and leaves the sync's preview fetch alone."""
    verdict = lambda d: {n: ok for n, ok, _ in judge_bodygone(*bg_model(d))}
    good = verdict(None)
    ok_good = list(good) == list(BG_CASES) and all(v is True for v in good.values())
    survived, noisy = [], []
    for d, targets in BG_SELFTEST_DEFECTS.items():
        got = verdict(d)
        if not all(got[t] is False for t in targets):
            survived.append(d)
        extra = sorted(k for k, v in got.items() if v is False and k not in targets)
        if extra:
            noisy.append("%s also reddens %s" % (d, extra))
    covered = {t for ts in BG_SELFTEST_DEFECTS.values() for t in ts} | {"account-added"}
    ok_cover = covered == set(BG_CASES)
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import realcore_mailfake as fake

    class M:
        import threading
        lock = threading.Lock()
        boxes = {"INBOX": [(2, b"x\r\n\r\ny"), (3, b"x\r\n\r\ny")]}
    put_back = bg_patch_fetch(fake, M)
    try:
        ok_patch = (fake.fetch_line(1, 3, b"H: v\r\n\r\nbody", "(UID BODY.PEEK[TEXT]<0.2048>)") != b"* 1 FETCH (UID 3 FLAGS ())\r\n"
                    and fake.fetch_line(1, 3, b"H: v\r\n\r\nbody", "(UID RFC822.SIZE BODY.PEEK[])") == b"* 1 FETCH (UID 3 FLAGS ())\r\n"
                    and fake.fetch_line(1, 2, b"H: v\r\n\r\nbody", "(UID RFC822.SIZE BODY.PEEK[])") == b"" and [u for u, _ in M.boxes["INBOX"]] == [3])
    finally:
        put_back()
    print("selftest slice bodygone: model %s, %d defects %s, every case covered %s, the fetch patch %s" % (
        "all pass" if ok_good else "FAILS %r" % good, len(BG_SELFTEST_DEFECTS), "each red on its own cases" if not survived and not noisy else "SURVIVE %s; %s" % (survived, noisy),
        "yes" if ok_cover else "NO %s" % sorted(set(BG_CASES) - covered), "hides the gone mail and the stuck body, not the preview" if ok_patch else "WRONG"))
    return ok_good and not survived and not noisy and ok_cover and ok_patch



# ---- slice labeltab (fix-realcore round 10, item c7c52ea4, owner 12 Nov 23:44 "when quitting, it doesn't seem to keep the mails"): a built-in label's
# filed mail is on its tab, in the strip's count and in the label's own list, and stays there across relaunches, on the real engine, against the
# loopback fake only, with the rig's scripted Apple judge (WHOOSH_RIG_APPLE_JUDGE=stand-in: it picks the label whose description shares a word of five
# letters or more with the mail: the three "question" mails go to Needs Reply, the two lunch mails to none). Reads, each time the same three:
#   tab    the list when the label's sidebar row is clicked (a built-in label is pinned: the click opens its tab), ids
#   strip  `inbox/tabs`, the entry of the label: its waiting figure (what the strip shows; "All done" is 0), which opening a mail does not change
#   label  `label_assignment/<id>` of every mail of All inboxes, the ids that name the label (the filing record itself)
# Steps: after the flow filed; a Save of the label's description while mails are filed (at once and after a pause); relaunch 1 (at once, then after the
# sync); relaunch 2 with the owner's main106 state (`WHOOSH_CORE_CHECK_DROP_TABS=3`: the built-in tab rows deleted before the open's backfill, engine
# check_tabs.rs; `rig/builtin_tabs_dropped` says how many); then ⇧Y on one filed mail: it leaves the tab and the count.
LT_LABEL = "needs-reply"
LT_FILED = ("Quick question about Tuesday", "Question about the venue", "Question on the schedule")
LT_OTHER = ("Lunch on Friday", "Notes from the walk")
LT_BODIES = {LT_FILED[0]: "I have a question for you: can we meet on Tuesday? Please reply when you can.",
             LT_FILED[1]: "One question before I book: is the venue free? I wait for your reply.",
             LT_FILED[2]: "A question on the schedule: does nine work for you? Your reply decides it.",
             LT_OTHER[0]: "Lunch is at noon, see you there.", LT_OTHER[1]: "The walk was fine, nothing to do."}
LT_DROP_OPEN = 3            # the third open of the data folder: the second relaunch
LT_FILE_WAIT_S = 150        # a check bound: the sync, then the label flow asks the scripted judge for each new mail
LT_SETTLE_S = 15            # a check bound: a pause after which a read must not have changed (the Save, the relaunch's sync)
LT_STEPS = ("filed", "save-at-once", "save-later", "relaunch-at-once", "relaunch-after-sync", "backfill-at-once", "backfill-after-sync")
LT_CASES = ("account-added", "the-flow-files-the-question-mails") + tuple("tab-strip-and-label-agree@%s" % s for s in LT_STEPS) + \
           ("the-ids-stay-the-same-across-the-relaunches", "a-save-of-the-rules-never-empties-the-tab", "the-owners-state-was-made", "the-backfill-puts-the-filings-on-the-tab",
            "unfiled-mail-leaves-the-tab-and-the-count", "the-driver-reached-every-step")


def lt_split(c, tab_id):
    """Opens the strip's tab `tab_id` (a view "primary"/"all" or a pinned label) through its own door `mac-main.splits.<i>`; answers the list's ids.
    round 11 (main108: the driver read 2 of 5): the list follows the open tab, and the Primary tab holds only the mail no label took."""
    rows = c.rows("mac-main.splits")
    i = next((k for k, r in enumerate(rows) if r.get("id") == tab_id), None)
    if i is None:
        raise RuntimeError("the strip has no tab %r (it holds %r)" % (tab_id, [r.get("id") for r in rows]))
    c.click("mac-main.splits.%d" % i)
    time.sleep(0.8)
    return [r.get("id") for r in c.rows("mac-main.list") if r.get("id")]


def lt_read(c, ids_by_subject, name):
    """The three reads (ids sorted), as the app and the engine serve them: the split tab of the label, the strip's figure, the filing record."""
    out = {}
    try:
        out["tab"] = sorted(lt_split(c, LT_LABEL))
        out["ui_tab"] = c.uistate().get("tab")
    except Exception as e:      # noqa: BLE001
        out["tab_error"] = str(e)[:200]
    tabs = cv(c, "inbox/tabs")
    entry = next((t for t in tabs if t.get("id") == LT_LABEL), None) if isinstance(tabs, list) else None
    out["strip"] = None if entry is None else entry.get("waiting")     # the strip's figure (store counts.rs: threads whose last mail is inbound; reading changes nothing)
    label = []
    for sid, mid in ids_by_subject.items():
        names = cv(c, "label_assignment/%s" % mid)
        if isinstance(names, list) and name in names:
            label.append(mid)
    out["label"] = sorted(label)
    out["filed_count"] = cv(c, "labels/%s/rerun_count" % LT_LABEL)
    return out


def drive_labeltab():
    """Inside launch-one: add the account (five mails), wait for the flow to file, then the reads after each step, two relaunches, and ⇧Y."""
    sys.path.insert(0, os.path.join(ROOT, "journeys"))
    from lib.ctl import Ctl  # noqa: E402
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import realcore_mailfake as fake  # noqa: E402
    c = Ctl().connect()
    answer = {"driver_errors": [], "core": None, "reads": {}}
    mv_add_account(c, fake, answer, 2)      # the Primary list waits for the two mails no label takes; the five are read on the All view below
    answer["core"] = c.uistate().get("core")

    def all_view():
        try:
            lt_split(c, "all")
        except Exception as e:      # noqa: BLE001
            return {"error": str(e)[:200]}
        return {r.get("subject"): r.get("id") for r in c.rows("mac-main.list") if r.get("subject") in LT_FILED + LT_OTHER}
    seen, secs = mv_poll(all_view, lambda v: "error" not in v and len(v) == len(LT_FILED + LT_OTHER), LT_FILE_WAIT_S)
    ids = {} if not seen or "error" in seen else seen
    answer["ids_by_subject"], answer["all_seconds"] = ids, secs
    name = cv(c, "label/%s/name" % LT_LABEL)
    answer["label_name"] = name
    if len(ids) < len(LT_FILED + LT_OTHER) or not name:
        answer["stopped_at"] = "read the five mails on the All view"
        answer["driver_errors"].append("step 'read the five mails on the All view' gave up after %ss: read %r (%d of %d subjects), the label's name %r" % (
            secs, seen if seen and "error" in seen else sorted(ids), len(ids), len(LT_FILED + LT_OTHER), name))
        return answer
    got, secs = mv_poll(lambda: lt_read(c, ids, name), lambda r: len(r.get("label") or []) >= len(LT_FILED) and len(r.get("tab") or []) >= len(LT_FILED), LT_FILE_WAIT_S)
    answer["reads"]["filed"], answer["filed_seconds"] = got, secs
    if len((got or {}).get("label") or []) < len(LT_FILED):
        answer["stopped_at"] = "wait for the flow to file the question mails"
        answer["driver_errors"].append("step 'wait for the flow to file the question mails' gave up after %ss: the last read %r" % (secs, got))
        return answer
    # a Save of the label's description while mails are filed (the inspector's own doors): the filings stay until a new answer replaces them
    ins = "mac-labels.inspector"
    drive_step(answer, "open the inspector", lambda: c.click("%s.open.modify.%s" % (ins, LT_LABEL)))
    drive_step(answer, "type the meaning", lambda: c.type(contracts_reply_description() + " Thanks."))
    drive_step(answer, "set the meaning", lambda: c.click(ins + ".meaning"))
    drive_step(answer, "Save", lambda: c.click(ins + ".apply"))
    answer["reads"]["save-at-once"] = lt_read(c, ids, name)
    time.sleep(LT_SETTLE_S)
    answer["reads"]["save-later"] = lt_read(c, ids, name)
    drive_step(answer, "close the inspector", lambda: c.click(ins + ".close"))
    for n, (now, later) in enumerate((("relaunch-at-once", "relaunch-after-sync"), ("backfill-at-once", "backfill-after-sync")), 1):
        nc, why = restart(c)
        if nc is None:
            answer["driver_errors"].append("relaunch %d: %s" % (n, why))
            return answer
        c = nc
        answer["reads"][now] = lt_read(c, ids, name)
        if n == 2:
            answer["dropped"] = cv(c, "rig/builtin_tabs_dropped")
        time.sleep(LT_SETTLE_S)
        answer["reads"][later] = lt_read(c, ids, name)
    # ⇧Y on the first filed mail of the tab: every label off it (the owner's own key), so it leaves the tab
    first = (answer["reads"]["backfill-after-sync"].get("tab") or [None])[0]
    answer["unfiled"] = first
    if first:
        drive_step(answer, "focus it", lambda: c.click("mac-main.attachments.dev.focus.%s" % first))
        drive_step(answer, "open it", lambda: c.key("Enter"))
        time.sleep(0.6)
        drive_step(answer, "close the reply field", lambda: c.key("Escape"))
        drive_step(answer, "⇧Y", lambda: c.key("⇧Y"))
        time.sleep(2.0)
        answer["reads"]["unfiled"] = lt_read(c, ids, name)
    return answer


def contracts_reply_description():
    """The built-in Needs Reply description as the engine seeds it (contracts reply_ai_logic.rs default_reply_ai_description), read from the source."""
    src = open(os.path.join(ROOT, "core", "crates", "contracts", "src", "reply_ai_logic.rs"), encoding="utf-8").read()
    m = re.search(r'fn default_reply_ai_description\(\) -> String \{\s*"([^"]+)"', src)
    return m.group(1) if m else "Emails that wait for a reply from me: a question, a request, a decision that someone is waiting for."


def judge_labeltab(answer):
    errors = answer.get("driver_errors") or []
    reads = answer.get("reads") or {}
    ids = answer.get("ids_by_subject") or {}
    want = sorted(ids[s] for s in LT_FILED if s in ids)
    out = [("account-added", answer.get("stage") in ("added", "summary"), "stage %r, core %r" % (answer.get("stage"), answer.get("core")))]
    filed = reads.get("filed")
    out.append(("the-flow-files-the-question-mails", None if filed is None else (filed.get("label") == want and len(want) == len(LT_FILED)),
                "not read: %s" % errors if filed is None else "the label's filings %d (wanted the %d question mails: %s) after %ss" % (len(filed.get("label") or []), len(LT_FILED), filed.get("label") == want, answer.get("filed_seconds"))))
    for step in LT_STEPS:
        r = reads.get(step)
        if r is None:
            out.append(("tab-strip-and-label-agree@%s" % step, None, "not read: %s" % errors)); continue
        agree = bool(r.get("label")) and r.get("tab") == r.get("label") and r.get("strip") == len(r.get("label") or [])
        out.append(("tab-strip-and-label-agree@%s" % step, agree, "tab %d rows (the app's tab %r), strip %r, label %d, the same ids in tab and label: %s, filed count %r" % (
            len(r.get("tab") or []), r.get("ui_tab"), r.get("strip"), len(r.get("label") or []), r.get("tab") == r.get("label"), r.get("filed_count"))))
    seen = [reads[s].get("tab") for s in LT_STEPS if s in reads]
    out.append(("the-ids-stay-the-same-across-the-relaunches", None if len(seen) < len(LT_STEPS) else all(t == want for t in seen),
                "tab ids per step equal to the filed ones: %r" % [t == want for t in seen]))
    sa, sl = reads.get("save-at-once"), reads.get("save-later")
    out.append(("a-save-of-the-rules-never-empties-the-tab", None if sa is None or sl is None else bool(sa.get("tab")) and bool(sl.get("tab")) and set(want) <= set(sa.get("tab")) and set(want) <= set(sl.get("tab")),
                "after the Save: at once %r rows, after %ss %r rows (filed %d)" % (len((sa or {}).get("tab") or []), LT_SETTLE_S, len((sl or {}).get("tab") or []), len(want))))
    dropped = answer.get("dropped")
    out.append(("the-owners-state-was-made", None if dropped is None and "backfill-at-once" not in reads else (isinstance(dropped, int) and dropped >= len(want)),
                "rig/builtin_tabs_dropped %r at the second relaunch (the open %d of the data folder; wanted at least %d)" % (dropped, LT_DROP_OPEN, len(want))))
    bf = reads.get("backfill-at-once")
    out.append(("the-backfill-puts-the-filings-on-the-tab", None if bf is None or not isinstance(dropped, int) else bf.get("tab") == want and bf.get("strip") == len(want),
                "after the tab rows were dropped: tab %r rows (equal to the filed: %s), strip %r" % (len((bf or {}).get("tab") or []), (bf or {}).get("tab") == want, (bf or {}).get("strip"))))
    un, gone = reads.get("unfiled"), answer.get("unfiled")
    before = reads.get("backfill-after-sync") or {}
    out.append(("unfiled-mail-leaves-the-tab-and-the-count", None if un is None or not gone else (gone not in (un.get("tab") or []) and gone not in (un.get("label") or [])
                                                                                                    and un.get("strip") == (before.get("strip") or 0) - 1),
                "the mail ⇧Y took off: on the tab %s, on the label %s; strip %r (before %r)" % (gone in ((un or {}).get("tab") or []), gone in ((un or {}).get("label") or []), (un or {}).get("strip"), before.get("strip"))))
    out.append(("the-driver-reached-every-step", not errors, "driver errors %r" % errors))
    return out


def load_labeltab_mail(fake, mail):
    mail.boxes["INBOX"] = [(i + 1, flow_raw("Pat <pat@example.test>", fake.ADDRESS, s, "labeltab-%d" % (i + 1), i + 1, body=LT_BODIES[s])) for i, s in enumerate(LT_OTHER + LT_FILED)]
    mail.boxes["Sent"] = []


def main_slice_labeltab(bundle):
    import json
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import realcore_mailfake as fake
    try:
        mail, imap, smtp = fake.start(0, 0, inbox=0)
    except OSError as e:
        print("the local servers did not start: %s" % e)
        print("1 checks, 0 failed, 1 not judged"); return 1
    load_labeltab_mail(fake, mail)
    ports = "%d:%d" % (imap.server_address[1], smtp.server_address[1])
    extra = ["WHOOSH_CORE_PLAIN_LOOPBACK=1", "WHOOSH_CORE_CHECK_SERVERS=" + ports, "WHOOSH_CORE_CHECK_FLOW=1", "WHOOSH_RIG_APPLE_JUDGE=stand-in",
             "WHOOSH_CORE_CHECK_DROP_TABS=%d" % LT_DROP_OPEN]
    env = dict(os.environ, WHOOSH_RIG_HIDDEN="1", LAUNCH_ENV=" ".join(["WHOOSH_RIG_HIDDEN=1"] + extra))
    env.pop("WHOOSH_FIXTURE_CORE", None)
    cmd = ["sh", os.path.join(ROOT, "scripts", "launch-one.sh"), bundle, "--", "--rig-hidden",
           "sh", "-c", 'WHOOSH_SOCKET_DIR="$D" python3 "$0" --inner --slice-labeltab', os.path.abspath(__file__)]
    try:
        r = launch(cmd, env)
    finally:
        imap.shutdown(); smtp.shutdown()
    out = r.stdout + r.stderr
    line = next((l for l in out.splitlines() if l.startswith("ANSWER ")), None)
    if line is None:
        print(out.rstrip()[-3000:])
        print("launcher exit %d" % r.returncode)
        print("1 checks, 0 failed, 1 not judged"); return 1
    answer = json.loads(line[len("ANSWER "):])
    print("SEEDED-SLICE-LABELTAB: the mail is the check's and the judge is the rig's scripted stand-in (WHOOSH_RIG_APPLE_JUDGE=stand-in); the tab rows are dropped by the rig on the third open (WHOOSH_CORE_CHECK_DROP_TABS)")
    print("launcher exit %d; the local server saw: %s; %s" % (r.returncode, sorted({l.split(" ")[0] for l in mail.log}), next((l for l in out.splitlines() if l.startswith("instances:")), "")))
    lines, rc = slice_lines(judge_labeltab(answer))
    print("\n".join(lines)); return rc


def lt_model(defect=None):
    """The self-test's stand-in for a correct engine (or one defect): the reads in the shape drive_labeltab returns. No app is launched."""
    ids = {s: "m-%d" % i for i, s in enumerate(LT_OTHER + LT_FILED)}
    filed = sorted(ids[s] for s in LT_FILED)
    one = lambda tab=None, strip=None, label=None: {"tab": list(filed if tab is None else tab), "strip": len(filed) if strip is None else strip,
                                                    "label": list(filed if label is None else label), "ui_tab": LT_LABEL, "filed_count": len(filed)}
    reads = {s: one() for s in LT_STEPS}
    rest = filed[1:]
    reads["unfiled"] = one(tab=rest, strip=len(rest), label=rest)
    ans = {"stage": "added", "core": "rust", "driver_errors": [], "ids_by_subject": ids, "label_name": "Needs Reply", "reads": reads, "filed_seconds": 12.0,
           "dropped": len(filed), "unfiled": filed[0]}
    if defect == "no-tab-row-at-filing":      # H1: the flow files by label_assignment alone: the label holds them, the tab and the strip do not
        for s in LT_STEPS[:5]:
            reads[s] = one(tab=[], strip=0)
    if defect == "no-backfill-at-open":       # H2: the dropped rows never come back
        for s in LT_STEPS[5:]:
            reads[s] = one(tab=[], strip=0)
        reads["unfiled"] = one(tab=[], strip=0, label=rest)
    if defect == "unfile-keeps-the-tab-row":  # H3
        reads["unfiled"] = one(tab=filed, strip=len(filed), label=rest)
    if defect == "save-empties-the-tab":
        reads["save-at-once"] = one(tab=[], strip=0, label=[])
    if defect == "relaunch-loses-them":       # the owner's report
        reads["relaunch-at-once"] = reads["relaunch-after-sync"] = one(tab=[], strip=0)
    if defect == "nothing-filed":
        for s in list(reads):
            reads[s] = one(tab=[], strip=0, label=[])
    if defect == "nothing-dropped":
        ans["dropped"] = 0
    if defect == "driver-error":
        ans["driver_errors"] = ["⇧Y: Unreadable: no such key"]
    return ans


LT_SELFTEST_DEFECTS = {
    "no-tab-row-at-filing": tuple("tab-strip-and-label-agree@%s" % s for s in LT_STEPS[:5]) + ("the-ids-stay-the-same-across-the-relaunches", "a-save-of-the-rules-never-empties-the-tab"),   # the tab is empty from the start
    "no-backfill-at-open": tuple("tab-strip-and-label-agree@%s" % s for s in LT_STEPS[5:]) + ("the-ids-stay-the-same-across-the-relaunches", "the-backfill-puts-the-filings-on-the-tab",
                            "unfiled-mail-leaves-the-tab-and-the-count"),   # with an empty tab nothing can leave it
    "unfile-keeps-the-tab-row": ("unfiled-mail-leaves-the-tab-and-the-count",),
    "save-empties-the-tab": ("tab-strip-and-label-agree@save-at-once", "the-ids-stay-the-same-across-the-relaunches", "a-save-of-the-rules-never-empties-the-tab"),
    "relaunch-loses-them": ("tab-strip-and-label-agree@relaunch-at-once", "tab-strip-and-label-agree@relaunch-after-sync", "the-ids-stay-the-same-across-the-relaunches"),
    "nothing-filed": ("the-flow-files-the-question-mails",) + tuple("tab-strip-and-label-agree@%s" % s for s in LT_STEPS) + ("the-ids-stay-the-same-across-the-relaunches",
                     "a-save-of-the-rules-never-empties-the-tab", "the-backfill-puts-the-filings-on-the-tab", "unfiled-mail-leaves-the-tab-and-the-count"),
    "nothing-dropped": ("the-owners-state-was-made",),
    "driver-error": ("the-driver-reached-every-step",),
}


def selftest_labeltab():
    """The judge on a correct model (every case passes) and one defect per case, H1 (no tab row at filing) and H2 (no backfill at open) among them, each red on
    its own cases only; the description the driver types is the engine's own."""
    verdict = lambda d: {n: ok for n, ok, _ in judge_labeltab(lt_model(d))}
    good = verdict(None)
    ok_good = list(good) == list(LT_CASES) and all(v is True for v in good.values())
    survived, noisy = [], []
    for d, targets in LT_SELFTEST_DEFECTS.items():
        got = verdict(d)
        if not all(got[t] is False for t in targets):
            survived.append(d)
        extra = sorted(k for k, v in got.items() if v is False and k not in targets)
        if extra:
            noisy.append("%s also reddens %s" % (d, extra))
    covered = {t for ts in LT_SELFTEST_DEFECTS.values() for t in ts} | {"account-added"}
    ok_cover = covered == set(LT_CASES)
    ok_desc = contracts_reply_description().startswith("Emails that wait for a reply")
    print("selftest slice labeltab: model %s, %d defects %s, every case covered %s, the seeded description read from the engine's source %s" % (
        "all pass" if ok_good else "FAILS %r" % good, len(LT_SELFTEST_DEFECTS), "each red on its own cases" if not survived and not noisy else "SURVIVE %s; %s" % (survived, noisy),
        "yes" if ok_cover else "NO %s" % sorted(set(LT_CASES) - covered), "yes" if ok_desc else "NO"))
    return ok_good and not survived and not noisy and ok_cover and ok_desc



# ---- slice sendfail (fix-realcore round 15, owner main115: a reply failed on "Connection reset by peer", attempts 1, never tried again). On the real engine,
# against the loopback fakes only: the local SMTP server resets the connection at the FIRST MAIL FROM (SO_LINGER 0: the client reads "Connection reset by
# peer") and serves every later session. The app sends a reply (the full composer, ⌘Enter); the driver never asks for a retry. The entry must show
# `retrying` with the step (mail_from), attempts 1 and its next try time (`outbox/<id>/send_state`), then `sent` by itself with attempts 2, and the server
# must hold one mail after two MAIL FROM. The entry is found through `outbox/open` (the open entries and their sending state).
SF_CASES = ("account-added", "first-try-reset-at-mail-from-shown-retrying", "entry-sent-by-itself-attempts-2", "one-mail-at-the-server", "the-driver-reached-every-step")
SF_ENTRY_WAIT_S = 40       # a check bound: the undo window (UNDO_SEND_DEFAULT_S) and the first try
SF_SENT_WAIT_S = 90        # a check bound: the first pause (SEND_RETRY_START_S, 30 s) and the second try, with room


def drive_sendfail():
    """Inside launch-one: add the account, answer the one INBOX mail from the full composer, then read the entry's sending state until it is sent or failed."""
    sys.path.insert(0, os.path.join(ROOT, "journeys"))
    from lib.ctl import Ctl  # noqa: E402
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import realcore_mailfake as fake  # noqa: E402
    c = Ctl().connect()
    answer = {"driver_errors": [], "core": None, "trail": []}
    mv_add_account(c, fake, answer, 1)
    answer["core"] = c.uistate().get("core")
    drive_step(answer, "open the mail", lambda: c.key("Enter"))
    time.sleep(0.6)
    drive_step(answer, "close the reply field", lambda: c.key("Escape"))
    drive_step(answer, "the full composer's reply", lambda: c.click("mac-main.reader.open-full-composer"))
    time.sleep(0.8)
    drive_step(answer, "text", lambda: c.type("Yes, that works for me."))
    drive_step(answer, "⌘Enter sends", lambda: c.key("⌘Enter"))
    open_, secs = mv_poll(lambda: cv(c, "outbox/open"), lambda v: isinstance(v, list) and len(v) > 0, SF_ENTRY_WAIT_S)
    answer["open_seconds"] = secs
    answer["open_served"] = open_ is not None
    if not open_:
        answer["driver_errors"].append("no open outbox entry was read within %ss (outbox/open %s)" % (SF_ENTRY_WAIT_S, "served, empty" if open_ is not None else "not served"))
        return answer
    entry = open_[0].get("id")
    answer["entry"] = entry
    t0, last = time.time(), None
    while time.time() - t0 < SF_SENT_WAIT_S:
        st = cv(c, "outbox/%s/send_state" % entry)
        if isinstance(st, dict) and st != last:
            answer["trail"].append(dict(st, seconds=round(time.time() - t0, 2)))
            last = st
        if isinstance(st, dict) and st.get("state") in ("sent", "failed"):
            break
        time.sleep(0.25)
    answer["final"] = last
    return answer


def judge_sendfail(answer, server):
    """`server`: {"resets": [step...], "mail_froms": n, "messages": n} as the local SMTP server counted them. States, counts and steps only."""
    errors = answer.get("driver_errors") or []
    out = [("account-added", answer.get("stage") in ("added", "summary"), "stage %r, core %r" % (answer.get("stage"), answer.get("core")))]
    trail = answer.get("trail") or []
    final = answer.get("final") or {}
    seen = " -> ".join("%s(%s%s)" % (t.get("state"), t.get("attempts"), ", " + t["stage"] if t.get("stage") else "") for t in trail) or "nothing read"
    if answer.get("entry") is None:
        why = "no entry to read: %s" % errors
        unjudged = None if not answer.get("open_served") else False
        out += [("first-try-reset-at-mail-from-shown-retrying", unjudged, why), ("entry-sent-by-itself-attempts-2", unjudged, why)]
    else:
        retry = next((t for t in trail if t.get("state") == "retrying"), None)
        ok_retry = (server.get("resets") == ["mail_from"] and retry is not None and retry.get("stage") == "mail_from" and retry.get("attempts") == 1
                    and isinstance(retry.get("next_try_ms"), int) and "MAIL FROM" in (retry.get("words") or ""))
        out.append(("first-try-reset-at-mail-from-shown-retrying", ok_retry, "server resets %r; states read %s; retrying words %r, next try %r" % (
            server.get("resets"), seen, (retry or {}).get("words"), (retry or {}).get("next_try_ms"))))
        out.append(("entry-sent-by-itself-attempts-2", final.get("state") == "sent" and final.get("attempts") == 2,
                    "last state %r, attempts %r, sent at %r, after %ss" % (final.get("state"), final.get("attempts"), final.get("sent_at_ms"), trail[-1].get("seconds") if trail else None)))
    out.append(("one-mail-at-the-server", server.get("messages") == 1 and server.get("mail_froms") == 2,
                "%r message(s) kept, %r MAIL FROM (one reset, one sent)" % (server.get("messages"), server.get("mail_froms"))))
    out.append(("the-driver-reached-every-step", not errors, "driver errors %r" % errors))
    return out


def main_slice_sendfail(bundle):
    import json
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import realcore_mailfake as fake
    try:
        mail, imap, smtp = fake.start(0, 0, inbox=0, act=True)
    except OSError as e:
        print("the local servers did not start: %s" % e)
        print("1 checks, 0 failed, 1 not judged"); return 1
    mail.boxes["INBOX"] = [(1, fake.plain_message())]
    smtp.reset_mail = 1
    ports = "%d:%d" % (imap.server_address[1], smtp.server_address[1])
    extra = ["WHOOSH_CORE_PLAIN_LOOPBACK=1", "WHOOSH_CORE_CHECK_SERVERS=" + ports]
    env = dict(os.environ, WHOOSH_RIG_HIDDEN="1", LAUNCH_ENV=" ".join(["WHOOSH_RIG_HIDDEN=1"] + extra))
    env.pop("WHOOSH_FIXTURE_CORE", None)
    cmd = ["sh", os.path.join(ROOT, "scripts", "launch-one.sh"), bundle, "--", "--rig-hidden",
           "sh", "-c", 'WHOOSH_SOCKET_DIR="$D" python3 "$0" --inner --slice-sendfail', os.path.abspath(__file__)]
    try:
        r = launch(cmd, env)
    finally:
        imap.shutdown(); smtp.shutdown()
    out = r.stdout + r.stderr
    line = next((l for l in out.splitlines() if l.startswith("ANSWER ")), None)
    if line is None:
        print(out.rstrip()[-3000:])
        print("launcher exit %d" % r.returncode)
        print("1 checks, 0 failed, 1 not judged"); return 1
    answer = json.loads(line[len("ANSWER "):])
    print("launcher exit %d; the SMTP server: resets %r, %d MAIL FROM, %d message(s); %s" % (
        r.returncode, smtp.resets, smtp.mail_froms, len(smtp.messages), next((l for l in out.splitlines() if l.startswith("instances:")), "")))
    lines, rc = slice_lines(judge_sendfail(answer, {"resets": list(smtp.resets), "mail_froms": smtp.mail_froms, "messages": len(smtp.messages)}))
    print("\n".join(lines)); return rc


def sf_model(defect=None):
    """The self-test's stand-in for a correct engine and server (or one defect), in the shape drive_sendfail returns. No app is launched."""
    words = "the connection to the mail server broke at MAIL FROM: network error: Connection reset by peer (os error 54)"
    trail = [{"state": "queued", "attempts": 0, "next_try_ms": None, "stage": None, "words": None, "sent_at_ms": None, "seconds": 0.0},
             {"state": "retrying", "attempts": 1, "next_try_ms": 1794552443858, "stage": "mail_from", "words": words, "sent_at_ms": None, "seconds": 10.2},
             {"state": "sent", "attempts": 2, "next_try_ms": None, "stage": None, "words": None, "sent_at_ms": 1794552444017, "seconds": 40.3}]
    server = {"resets": ["mail_from"], "mail_froms": 2, "messages": 1}
    answer = {"stage": "added", "core": "rust", "driver_errors": [], "open_served": True, "entry": "ob-1", "trail": trail, "final": trail[-1]}
    if defect == "never-retried":       # main115: failed after one try, no next time
        answer["trail"] = trail[:1] + [dict(trail[1], state="failed", next_try_ms=None)]
        answer["final"] = answer["trail"][-1]
    if defect == "no-step":              # retrying, but the step is not named
        answer["trail"] = [trail[0], dict(trail[1], stage=None, words="the mail server did not accept the message: network error"), trail[2]]
    if defect == "sent-twice":
        server["messages"] = 2
    if defect == "no-reset":             # the server never reset: the first try simply passed
        server.update(resets=[], mail_froms=1)
        answer["trail"] = [trail[0], dict(trail[2], attempts=1)]
        answer["final"] = answer["trail"][-1]
    if defect == "driver-error":
        answer["driver_errors"] = ["⌘Enter sends: Unreadable: no such key"]
    if defect == "not-served":           # a build without outbox/open: NOT JUDGED, never a pass
        answer.update(open_served=False, entry=None, trail=[], final=None, driver_errors=["no open outbox entry was read within 40s (outbox/open not served)"])
        server.update(resets=["mail_from"], mail_froms=2, messages=1)
    return answer, server


SF_SELFTEST_DEFECTS = {
    "never-retried": ("first-try-reset-at-mail-from-shown-retrying", "entry-sent-by-itself-attempts-2"),
    "no-step": ("first-try-reset-at-mail-from-shown-retrying",),
    "sent-twice": ("one-mail-at-the-server",),
    "no-reset": ("first-try-reset-at-mail-from-shown-retrying", "entry-sent-by-itself-attempts-2", "one-mail-at-the-server"),
    "driver-error": ("the-driver-reached-every-step",),
}


def selftest_sendfail():
    """The judge against a correct model (every case passes), one defect per case (each red, only on its own cases), and a build that does not serve
    outbox/open (its two entry cases NOT JUDGED, never a pass)."""
    verdict = lambda d: {n: ok for n, ok, _ in judge_sendfail(*sf_model(d))}
    good = verdict(None)
    ok_good = list(good) == list(SF_CASES) and all(v is True for v in good.values())
    survived, noisy = [], []
    for d, targets in SF_SELFTEST_DEFECTS.items():
        got = verdict(d)
        if not all(got[t] is False for t in targets):
            survived.append(d)
        extra = sorted(k for k, v in got.items() if v is False and k not in targets)
        if extra:
            noisy.append("%s also reddens %s" % (d, extra))
    ns = verdict("not-served")
    ok_ns = ns["first-try-reset-at-mail-from-shown-retrying"] is None and ns["entry-sent-by-itself-attempts-2"] is None and ns["the-driver-reached-every-step"] is False
    covered = {t for ts in SF_SELFTEST_DEFECTS.values() for t in ts} | {"account-added"}
    ok_cover = covered == set(SF_CASES)
    print("selftest slice sendfail: model %s, %d defects %s, outbox/open not served %s, every case covered %s" % (
        "all pass" if ok_good else "FAILS %r" % good, len(SF_SELFTEST_DEFECTS), "each red on its own cases" if not survived and not noisy else "SURVIVE %s; %s" % (survived, noisy),
        "NOT JUDGED as meant" if ok_ns else "WRONG %r" % ns, "yes" if ok_cover else "NO %s" % sorted(set(SF_CASES) - covered)))
    return ok_good and not survived and not noisy and ok_ns and ok_cover



SLICE_FLAGS = ("--slice-vip", "--slice-sources", "--slice-sendfail", "--slice-labeltab", "--slice-bodygone", "--slice-attachsend", "--slice-aisort", "--slice-body", "--slice-rulecount", "--slice-crawl", "--slice-aikey", "--slice-mailview", "--slice-labels",
               "--slice-moved", "--slice-search", "--slice-labelclick", "--slice-accountmark", "--slice6", "--slice5", "--slice-bye", "--slice4", "--slice3b", "--slice3",
               "--slice2b", "--slice2", "--slice-detect")


def normal_argv(argv):
    """fix-realcore round 7 (main100 lead-slice-attachsend-mutant.log: slice 1's detector line in one second): a runner that hands the flags as ONE
    argument ("--slice-attachsend --mutant") matched no slice and ran slice 1. Each argument is split at its spaces; a bundle path keeps its spaces
    (it is a folder that exists). Returns the new list and the --slice word that matches no slice, if any."""
    out = argv[:1]
    for a in argv[1:]:
        out += [a] if (os.path.exists(a) or " " not in a) else a.split()
    unknown = next((a.split("=", 1)[0] for a in out[1:] if a.startswith("--slice") and a.split("=", 1)[0] not in SLICE_FLAGS), None)
    return out, unknown


def main():
    sys.argv[:], unknown = normal_argv(sys.argv)
    if unknown:
        print("usage: %s is not a slice of this check (one of %s)" % (unknown, ", ".join(SLICE_FLAGS))); return 2
    if "--selftest" in sys.argv:
        return selftest()
    if "--inner" in sys.argv:
        import json
        pick = [("--slice-vip", drive_vip), ("--slice-sources", drive_sources), ("--slice-sendfail", drive_sendfail), ("--slice-labeltab", drive_labeltab), ("--slice-bodygone", drive_bodygone), ("--slice-attachsend", drive_attachsend), ("--slice-aisort", drive_aisort), ("--slice-body", drive_body), ("--slice-rulecount", drive_rulecount), ("--slice-crawl", drive_crawl), ("--slice-aikey", drive_aikey), ("--slice-mailview", drive_mailview), ("--slice-labels", drive_labels), ("--slice-moved", drive_moved), ("--slice-search", drive_search), ("--slice-labelclick", drive_labelclick), ("--slice-accountmark", drive_accountmark), ("--slice6", drive6), ("--slice5", drive5), ("--slice-bye", drive_bye), ("--slice4", drive4), ("--slice3b", drive3b), ("--slice3", drive3),
                ("--slice2b", drive2b), ("--slice2", drive2), ("--slice-detect", drive_detect)]
        fn = next((f for flag, f in pick if flag in sys.argv), drive)
        print("ANSWER " + json.dumps(fn()))
        print(refused_line())
        return 0
    if len(sys.argv) < 2 or not os.path.isdir(sys.argv[1]):
        print("usage: realcore-check.py <bundle> [--slice2 | --slice2b | --slice3 | --slice3b | --slice4 | --slice5 | --slice6 | --slice-search | --slice-labelclick | --slice-accountmark | --slice-bye [--refuse] | --slice-detect | --slice-mailview | --slice-labels | --slice-sources | --slice-vip | --slice-moved | --slice-body | --slice-rulecount | --slice-crawl [--paused[=battery:<percent>|heat]] | --slice-aikey | --slice-attachsend | --slice-bodygone | --slice-labeltab | --slice-sendfail] [--mutant[=<name>]] | --selftest"); return 2
    bundle, mutant = os.path.abspath(sys.argv[1]), "--mutant" in sys.argv
    has = lambda f: f in sys.argv
    if has("--slice-vip"):
        return with_outside_case(lambda: main_slice_vip(bundle))
    if has("--slice-sources"):
        return with_outside_case(lambda: main_slice_sources(bundle))
    if has("--slice-sendfail"):
        return with_outside_case(lambda: main_slice_sendfail(bundle))
    if has("--slice-labeltab"):
        return with_outside_case(lambda: main_slice_labeltab(bundle))
    if has("--slice-bodygone"):
        return with_outside_case(lambda: main_slice_bodygone(bundle))
    if has("--slice-attachsend"):
        return with_outside_case(lambda: main_slice_attachsend(bundle, mutant))
    if has("--slice-aisort"):
        return with_outside_case(lambda: main_slice_aisort(bundle, mutant))
    if has("--slice-body"):
        return with_outside_case(lambda: main_slice_body(bundle, mutant))
    if has("--slice-rulecount"):
        return with_outside_case(lambda: main_slice_rulecount(bundle, mutant))
    if has("--slice-crawl"):
        named = next((a.split("=", 1)[1] for a in sys.argv if a.startswith("--mutant=")), None) or CRAWL_DEFAULT_MUTANT
        mutant = mutant or any(a.startswith("--mutant=") for a in sys.argv)
        held = next((a.split("=", 1)[1] if "=" in a else CRAWL_PAUSE_DEFAULT for a in sys.argv if a == "--paused" or a.startswith("--paused=")), None)
        if mutant and named not in CRAWL_MUTANTS:
            print("usage: --slice-crawl --mutant[=%s]" % "|".join(CRAWL_MUTANTS)); return 2
        if mutant and held:
            print("usage: --paused and --mutant are two runs, not one"); return 2
        return with_outside_case(lambda: main_slice_crawl(bundle, mutant, named, held))
    if has("--slice-aikey"):
        return with_outside_case(lambda: main_slice_aikey(bundle))
    if has("--slice-mailview"):
        return with_outside_case(lambda: main_slice_mailview(bundle))
    if has("--slice-labels"):
        return with_outside_case(lambda: main_slice_labels(bundle))
    if has("--slice-moved"):
        return with_outside_case(lambda: main_slice_moved(bundle, mutant))
    if has("--slice-search"):
        return with_outside_case(lambda: main_slice_search(bundle, mutant))
    if has("--slice-accountmark"):
        return with_outside_case(lambda: main_slice_accountmark(bundle))
    if has("--slice-labelclick"):
        return with_outside_case(lambda: main_slice_labelclick(bundle, mutant))
    if has("--slice6"):
        return with_outside_case(lambda: main_slice6(bundle, mutant))
    if has("--slice5"):
        return with_outside_case(lambda: main_slice5(bundle, mutant))
    if has("--slice-bye"):
        return with_outside_case(lambda: main_slice_bye(bundle, has("--refuse")))
    if has("--slice4"):
        return with_outside_case(lambda: main_slice4(bundle, mutant))
    if has("--slice3b"):
        return with_outside_case(lambda: main_slice3(bundle, mutant, ai=True))
    if has("--slice3"):
        return with_outside_case(lambda: main_slice3(bundle, mutant))
    if has("--slice2b"):
        named = next((a.split("=", 1)[1] for a in sys.argv if a.startswith("--mutant=")), None)
        return with_outside_case(lambda: main_slice2b(bundle, named or ("oauth-never-exchanges" if mutant else None)))
    if has("--slice2"):
        return with_outside_case(lambda: main_slice2(bundle, mutant))
    if has("--slice-detect"):
        return with_outside_case(lambda: main_slice_detect(bundle))
    return with_outside_case(lambda: main_slice1(bundle, mutant))


def main_slice2(bundle, mutant):
    """The local servers live in THIS process (threads) for the whole launch; the app reaches them on 127.0.0.1 only."""
    import json
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import realcore_mailfake as fake
    try:
        mail, imap, smtp = fake.start(0, 0)
    except OSError as e:
        print("the local servers did not start: %s" % e)
        print("1 checks, 0 failed, 1 not judged"); return 1
    ports = "%d:%d" % (imap.server_address[1], smtp.server_address[1])
    extra = ["WHOOSH_CORE_PLAIN_LOOPBACK=1", "WHOOSH_CORE_CHECK_SERVERS=" + ports] + (["WHOOSH_CORE_MUTANT=sync-never-stores"] if mutant else [])
    env = dict(os.environ, WHOOSH_RIG_HIDDEN="1", LAUNCH_ENV=" ".join(["WHOOSH_RIG_HIDDEN=1"] + extra))
    env.pop("WHOOSH_FIXTURE_CORE", None)
    cmd = ["sh", os.path.join(ROOT, "scripts", "launch-one.sh"), bundle, "--", "--rig-hidden",
           "sh", "-c", 'WHOOSH_SOCKET_DIR="$D" python3 "$0" --inner --slice2', os.path.abspath(__file__)]
    r = launch(cmd, env)
    imap.shutdown(); smtp.shutdown()
    out = r.stdout + r.stderr
    line = next((l for l in out.splitlines() if l.startswith("ANSWER ")), None)
    if line is None:
        print(out.rstrip()[-3000:])
        print("launcher exit %d" % r.returncode)
        print("1 checks, 0 failed, 1 not judged"); return 1
    answer = json.loads(line[len("ANSWER "):])
    print("launcher exit %d; list waited %ss; the local server saw: %s; %s" % (r.returncode, answer.get("seconds"),
          sorted({l.split(" ")[0] for l in mail.log}), next((l for l in out.splitlines() if l.startswith("instances:")), "")))
    results = judge2(answer)
    lines = ["%-10s %s  %s" % ("PASS" if ok else ("NOT JUDGED" if ok is None else "FAIL"), n, d) for n, ok, d in results]
    failed = sum(1 for _, ok, _ in results if ok is False)
    if mutant:
        red = all(ok is False for n, ok, _ in results if n in ("list-shows-the-synced-inbox", "rows-belong-to-the-added-account", "counts-above-zero"))
        lines.append("mutant sync-never-stores: %s" % ("RED as it must be (no stored mail, no list, no counts)" if red else "SURVIVED: the list or the counts showed mail the sync never stored"))
        lines.append("%d checks, %d failed, 0 not judged (mutant run)" % (len(results), failed))
        print("\n".join(lines)); return 0 if red else 1
    lines.append("%d checks, %d failed, 0 not judged" % (len(results), failed))
    print("\n".join(lines)); return 0 if failed == 0 else 1


if __name__ == "__main__":
    sys.exit(main())

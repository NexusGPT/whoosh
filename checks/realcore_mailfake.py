#!/usr/bin/env python3
"""A local IMAP and SMTP server for checks/realcore-check.py --slice2: clear text on 127.0.0.1, nothing leaves the Mac.

IMAP speaks what the real core's sync sends (sync-imap over async-imap): the greeting, CAPABILITY, LOGIN, LIST with
SPECIAL-USE, SELECT/EXAMINE, UID SEARCH, UID FETCH and FETCH (UID FLAGS INTERNALDATE RFC822.SIZE BODY.PEEK[HEADER]
BODY.PEEK[TEXT]<0.n> BODY.PEEK[]), IDLE/DONE, ENABLE, NOOP, LOGOUT. No CONDSTORE/QRESYNC: the core then re-reads the newest
page on a change, its documented fallback. SMTP speaks EHLO, AUTH PLAIN/LOGIN/XOAUTH2, NOOP, RSET, QUIT: the sign-in check
only. The account is ADDRESS with PASSWORD, or (slice 2b) ADDRESS signed in by SASL XOAUTH2 with ACCESS_TOKEN, the token
checks/realcore_authfake.py issues; any other password or token is refused in the server's words.

    python3 checks/realcore_mailfake.py --selftest   drive both servers with Python's own imaplib/smtplib clients
    python3 checks/realcore_mailfake.py --serve [--latency-ms N] [--messages N]   serve until interrupted, printing the two ports

perf-realcore-r1 adds two options, both off by default: `latency_ms` (the server waits that long before it answers each SELECT and
each FETCH, the two round trips a body fetch costs; nothing else is slowed) and `messages` (the number of INBOX messages seeded, each
with a multipart/alternative body, text and html, quoted-printable, so the real core has bodies to fetch).
"""
import base64
import email.message
import email.policy
import os
import re
import select
import socket
import socketserver
import struct
import sys
import threading
import time

ADDRESS = "owner@whoosh-check.invalid"
PASSWORD = "check-password"
ACCESS_TOKEN = "check-access-token"   # a stand-in issued by the local authority; never a real token
CAPS = "IMAP4rev1 IDLE SPECIAL-USE ENABLE UIDPLUS AUTH=XOAUTH2"
IDLE_POLL_S = 0.5   # how often an IDLE connection looks for new mail to announce (a fake's own pace, not a ledger number)


def xoauth2_ok(b64):
    """SASL XOAUTH2: base64 of "user=" ADDRESS ^A "auth=Bearer " ACCESS_TOKEN ^A ^A."""
    try:
        got = base64.b64decode(b64.strip())
    except Exception:
        return False
    return got == ("user=%s\x01auth=Bearer %s\x01\x01" % (ADDRESS, ACCESS_TOKEN)).encode()
UIDVALIDITY = 7
SECOND_ADDRESS = "second@whoosh-check.invalid"   # firstrun: the second account, a server with no SPECIAL-USE (Sent Items, Receipts)
FIRSTRUN_SENT = "Éléments envoyés"               # firstrun: the first account's Sent folder, SPECIAL-USE \\Sent, a non-ASCII name (modified UTF-7 on the wire)


def mutf7(name):
    """IMAP modified UTF-7 (RFC 3501 5.1.3): the form a folder name has on the wire. ASCII names pass unchanged."""
    out, buf = "", ""
    def flush():
        nonlocal out, buf
        if buf:
            out += "&" + base64.b64encode(buf.encode("utf-16-be")).decode().rstrip("=").replace("/", ",") + "-"
            buf = ""
    for ch in name:
        if 0x20 <= ord(ch) <= 0x7e:
            flush()
            out += "&-" if ch == "&" else ch
        else:
            buf += ch
    flush()
    return out


def mutf7_decode(name):
    def one(m):
        if m.group(1) == "":
            return "&"
        b = m.group(1).replace(",", "/")
        return base64.b64decode(b + "=" * (-len(b) % 4)).decode("utf-16-be")
    return re.sub(r"&([^-]*)-", one, name)


def rich_body(n):
    """A multipart/alternative body, text then html, both quoted-printable (a non-ASCII word, a soft line break, an =3D)."""
    text = "Body of check message %d.=0D=0AThis is the text part, line one is long enough to need a soft=\r\n line break =E2=80=94 and an equals sign: a=3Db.\r\n" % n
    html = "<html><body><p>Body of check message %d.</p><p style=3D\"color:#333\">The html part, with a soft=\r\n line break.</p></body></html>\r\n" % n
    b = "check-boundary-%d" % n
    return ("MIME-Version: 1.0\r\nContent-Type: multipart/alternative; boundary=\"%s\"\r\n\r\n"
            "--%s\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Transfer-Encoding: quoted-printable\r\n\r\n%s\r\n"
            "--%s\r\nContent-Type: text/html; charset=utf-8\r\nContent-Transfer-Encoding: quoted-printable\r\n\r\n%s\r\n--%s--\r\n"
            % (b, b, text, b, html, b))


def message_id(n, folder="INBOX"):
    """The Message-ID of check message n (what raw_message writes), with its angle brackets."""
    return "<check-%s-%d@whoosh-check.invalid>" % (folder.lower(), n)


def ref_chain(n):
    """The References chain of check message n when the fake is started with `refs` (two older ids, oldest first)."""
    return ["<ref-%d-a@whoosh-check.invalid>" % n, "<ref-%d-b@whoosh-check.invalid>" % n]


def raw_message(n, folder, rich=False, refs=False):
    to_from = ("From: Owner <%s>\r\nTo: friend%d@example.test\r\n" % (ADDRESS, n)) if folder == "Sent" else \
        ("From: Sender %d <sender%d@example.test>\r\nTo: %s\r\n" % (n, n, ADDRESS))
    chain = ("References: %s\r\n" % " ".join(ref_chain(n))) if refs else ""
    return (to_from + "Subject: Check message %d\r\nMessage-ID: <check-%s-%d@whoosh-check.invalid>\r\n%s"
            "Date: Mon, 09 Nov 2026 10:%02d:00 +0000\r\n%s" % (n, folder.lower(), n, chain, n % 60,
            rich_body(n) if rich else "\r\nBody of check message %d.\r\n" % n)).encode()


# ---------------------------------------------------------------- slice 6 (junk-r2): the spam filter's mail
# Invented messages on reserved names (RFC 2606 .example and .invalid): no real sender, no real mail.
CORPUS_PER_CLASS = 24   # above the junk model's JUNK_MODEL_MIN_PER_CLASS (docs/CONSTANTS.md), so the first-run training gives it an opinion
JUNK_WORDS = ["verify your account now", "your account is locked", "confirm your password today", "urgent security alert",
              "claim your refund", "unusual sign-in activity", "update your payment details", "final notice before closure"]
GOOD_WORDS = ["lunch on friday", "notes from the planning meeting", "the slides for tuesday", "photos from the weekend",
              "the draft for review", "agenda for the team call", "dinner at our place", "the invoice you asked about"]
PHISH_SUBJECT = "Your account is locked: verify your account now"
PLAIN_SUBJECT = "Lunch on Friday with the team"


def junk_corpus_message(n):
    w = JUNK_WORDS[n % len(JUNK_WORDS)]
    return ("From: Security Team <alert%d@secure-login.example>\r\nTo: %s\r\nSubject: %s %d\r\nMessage-ID: <junk-%d@secure-login.example>\r\n"
            "Date: Sun, 08 Nov 2026 09:%02d:00 +0000\r\n\r\n%s: click the link to verify your account and confirm your password now.\r\n"
            % (n, ADDRESS, w.capitalize(), n, n, n % 60, w.capitalize())).encode()


def good_corpus_message(n):
    w = GOOD_WORDS[n % len(GOOD_WORDS)]
    return ("From: Ann Lee <ann%d@friends.example>\r\nTo: %s\r\nSubject: %s %d\r\nMessage-ID: <good-%d@friends.example>\r\n"
            "Date: Sun, 08 Nov 2026 08:%02d:00 +0000\r\n\r\nHello, about %s: shall we meet at noon with the team?\r\n"
            % (n, ADDRESS, w.capitalize(), n, n, n % 60, w)).encode()


def phish_message():
    """SPF, DKIM and DMARC fail in the receiving server's own (topmost) Authentication-Results; the display name carries a bank's
    address while the mail comes from another site. A lower, forged header says pass and must not be believed."""
    return ("Authentication-Results: mx.whoosh-check.invalid; spf=fail smtp.mailfrom=secure-login.example;\r\n"
            " dkim=fail header.d=secure-login.example; dmarc=fail header.from=secure-login.example\r\n"
            "Authentication-Results: forged.example; spf=pass; dkim=pass; dmarc=pass\r\n"
            "From: \"service@bank.example\" <alerts@secure-login.example>\r\nTo: %s\r\nSubject: %s\r\n"
            "Message-ID: <phish-1@secure-login.example>\r\nDate: Mon, 09 Nov 2026 11:00:00 +0000\r\n\r\n"
            "Urgent security alert: verify your account now and confirm your password today.\r\n" % (ADDRESS, PHISH_SUBJECT)).encode()


def plain_message():
    return ("From: Ann Lee <ann@friends.example>\r\nTo: %s\r\nSubject: %s\r\nMessage-ID: <plain-1@friends.example>\r\n"
            "Date: Mon, 09 Nov 2026 11:01:00 +0000\r\n\r\nHello, shall we meet at noon on friday with the team?\r\n" % (ADDRESS, PLAIN_SUBJECT)).encode()


# ---------------------------------------------------------------- tail-realcore: the mail shapes of the hidden `--slice-body` and `--slice-crawl`
# Invented text on reserved names. The two body shapes are the ones core/crates/engine/tests/engine_body_qp.rs writes (sweetgreen_raw, soyo_raw), except that
# here the sweetgreen html part carries both sentences (fix-realcore: the reader draws the html alternative, so the words the slice asks for are in it too):
# a multipart/alternative mail whose parts are quoted-printable with soft line breaks inside words, and one whose text part is base64.
BODY_BOUNDARY = "bnd-7c1f-omega"
SWEET_SUBJECT, SOYO_SUBJECT = "Omega Glow Bowl", "Weekly picks"
SWEET_WORDS = ("toasted almonds over warm quinoa", "on your way home.")        # the decoded text: the soft breaks sit inside "almonds" and "home" in both raw parts
SOYO_WORDS = ("Soyo World weekly picks: lanterns, kites and a quiet corner of the market.",)   # the base64 text part, decoded
SOYO_BOUNDARY = "soyo-bound"


def sweetgreen_raw(minute=1):
    return ("From: sweetgreen <hello@sweetgreen.example>\r\nTo: %s\r\nSubject: %s\r\nMessage-ID: <omega@sweetgreen.example>\r\nMIME-Version: 1.0\r\n"
            "Date: Mon, 09 Nov 2026 10:%02d:00 +0000\r\nContent-Type: multipart/alternative; boundary=\"%s\"\r\n\r\n"
            "--%s\r\nContent-Transfer-Encoding: quoted-printable\r\nContent-Type: text/plain; charset=us-ascii\r\n\r\n"
            "The Omega Glow Bowl: miso glazed salmon, sweet corn salsa, and toasted almo=\r\nnds over warm quinoa. Order now and pick it up on your way h=\r\nome.\r\n\r\n"
            "--%s\r\nContent-Transfer-Encoding: quoted-printable\r\nContent-Type: text/html; charset=us-ascii\r\n\r\n"
            "<html><body><p>The Omega Glow Bowl: miso glazed salmon, sweet co=\r\nrn salsa, and toasted almo=\r\nnds over warm quinoa. Order now and pick it up on your way h=\r\nome.</p></body></html>\r\n"
            "--%s--\r\n" % (ADDRESS, SWEET_SUBJECT, minute, BODY_BOUNDARY, BODY_BOUNDARY, BODY_BOUNDARY, BODY_BOUNDARY)).encode()


def soyo_raw(minute=2):
    text_b64 = base64.b64encode(SOYO_WORDS[0].encode()).decode()
    text_b64 = "\r\n".join(text_b64[i:i + 40] for i in range(0, len(text_b64), 40))
    return ("From: Soyo World <news@soyo.example>\r\nTo: %s\r\nSubject: %s\r\nMessage-ID: <picks@soyo.example>\r\nMIME-Version: 1.0\r\n"
            "Date: Mon, 09 Nov 2026 10:%02d:00 +0000\r\nContent-Type: multipart/alternative; boundary=\"%s\"\r\n\r\n"
            "--%s\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Transfer-Encoding: base64\r\n\r\n%s\r\n"
            "--%s\r\nContent-Type: text/html; charset=utf-8\r\nContent-Transfer-Encoding: quoted-printable\r\n\r\n"
            "<html><body><p class=3D\"picks\">Soyo World weekly picks: lanterns, kites and a quiet cor=\r\nner of the market.</p></body></html>\r\n"
            "--%s--\r\n" % (ADDRESS, SOYO_SUBJECT, minute, SOYO_BOUNDARY, SOYO_BOUNDARY, text_b64, SOYO_BOUNDARY, SOYO_BOUNDARY)).encode()


def load_body_mail(mail):
    """INBOX uid 1 the sweetgreen-shaped mail (older), uid 2 the Soyo-World-shaped mail (newer, first on the list)."""
    mail.boxes["INBOX"] = [(1, sweetgreen_raw(1)), (2, soyo_raw(2))]


# The crawl's mailbox: INBOX 25, Archive 65, Sent 10 (needs a server started with act=True). fix-realcore: Archive holds more than BODY_PREFETCH_COUNT (50, the newest
# bodies of a folder the sync's prefetch downloads, store `bodies_missing`), so its oldest 15 (the word and the meaning mail among them) are the crawl's alone: with
# Archive 15 the prefetch stored every body and the mutant crawl-never-runs survived (main92 s1-014.log). One Archive mail holds a word that is in no subject and no other mail
# (CRAWL_WORD), another holds a meaning that no other mail shares and none of the query's words (CRAWL_MEANING_BODY, asked by CRAWL_MEANING_QUERY).
CRAWL_PLAN = (("INBOX", 25), ("Archive", 65), ("Sent", 10))
CRAWL_WORD = "zeppelin"
CRAWL_WORD_SUBJECT, CRAWL_WORD_UID = "Archive note 7", 7
CRAWL_MEANING_SUBJECT, CRAWL_MEANING_UID = "Archive note 11", 11
CRAWL_MEANING_BODY = "The little harbour wakes at dawn. Fishing boats come home with the night's catch and gulls wheel over the quay."
CRAWL_MEANING_QUERY = "seaside port early morning"
CRAWL_WORD_BODY = "Remember that the zeppelin hangar key is under the green mat by the side door."


# fix-realcore round 2 (main95 s1-003: the mutant crawl-never-runs found the word and the meaning mail anyway): the sync fetches the first IMAP_PREVIEW_BYTES
# (docs/CONSTANTS.md, 2048) of every text for the row preview, and the indexes read it, so a short body was found with no crawl. The word and the meaning
# sentence now sit after CRAWL_PADDING, past those bytes: only the whole body, which only the crawl fetches for these two (the oldest of Archive), holds them.
CRAWL_PADDING = "This paragraph is ordinary padding for the check and says nothing in particular. " * 30


def ledger_number(name):
    """A number of docs/CONSTANTS.md (`| NAME | value | ...`), None when the row is missing."""
    path = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "docs", "CONSTANTS.md")
    with open(path, encoding="utf-8") as f:
        for line in f:
            cells = [c.strip() for c in line.split("|")]
            if len(cells) > 2 and cells[1] == name:
                return int(cells[2].replace(",", "").replace("_", ""))
    return None


def crawl_raw(folder, n):
    name = folder if folder != "Sent" else "Sent"
    body = "Plain filler text number %d for the crawl check, nothing to see here." % n
    if folder == "Archive" and n == CRAWL_WORD_UID:
        body = CRAWL_WORD_BODY
    if folder == "Archive" and n == CRAWL_MEANING_UID:
        body = CRAWL_MEANING_BODY
    # fix-realcore round 7 (main100 again-020: the meaning case PASSED under crawl-never-runs): EVERY mail starts with the same padding, so the
    # previews the sync fetched are alike for all 100 and nothing but the body past them sets the harbour mail apart (before, only the two
    # special mails were padded, and that padding alone made them stand out by meaning without their bodies)
    body = CRAWL_PADDING + "\r\n\r\n" + body
    who = ("From: Owner <%s>\r\nTo: friend%d@example.test\r\n" % (ADDRESS, n)) if folder == "Sent" else ("From: Sender %d <sender%d@example.test>\r\nTo: %s\r\n" % (n, n, ADDRESS))
    return ("%sSubject: %s note %d\r\nMessage-ID: <crawl-%s-%d@whoosh-check.invalid>\r\nDate: Mon, 09 Nov 2026 %02d:%02d:00 +0000\r\n\r\n%s\r\n"
            % (who, name, n, folder.lower(), n, 10 + n // 60, n % 60, body)).encode()     # newest = highest UID past 59 too (the hour moves on)


def load_crawl_mail(mail):
    """Every folder of CRAWL_PLAN, UIDs 1..N, newest = highest UID. Returns the total number of messages."""
    for folder, count in CRAWL_PLAN:
        mail.boxes[folder] = [(n, crawl_raw(folder, n)) for n in range(1, count + 1)]
    return sum(c for _, c in CRAWL_PLAN)


# ---- firstrun (checks/firstrun-check.py): the first-run corpus. 60 INBOX messages of real-looking mail, built by Python's own `email`
# package so the accents, the curly apostrophes and the HTML entities reach the core exactly as a real server sends them (RFC 2047 headers,
# quoted-printable or base64 bodies). Every fact the first-run steps read back is named here once.
FR_QUESTIONS = ["Question sur le devis n° 4471", "Pouvez-vous confirmer la date de livraison ?"]   # 2 unread, unanswered, from QUESTION_SENDER (T-10)
QUESTION_SENDER = "client@example.test"
FR_NEWSLETTER = "Les offres de la semaine"           # List-Unsubscribe: T-10 (a rule label with no AI)
FR_ANSWERED = "Rendez-vous jeudi ?"                  # the owner answered it: a Sent copy sits LATER in its thread (T-10)
FR_CADEAU_SUBJECT, FR_CADEAU_WORD = "Une idée pour samedi", "cadeau"    # a word that is only in the BODY (T-09)
FR_REUNION_SUBJECT, FR_REUNION_WORD = "Point rapide", "réunion"         # likewise
FR_NAMED = [("L’offre d’Oranta", "Hélène Dupré", "helene.dupre@example.test"),
            ("Réunion de l’équipe", "François L’Écuyer", "francois@example.test"),
            ("RE: Devis n° 4471", "Zoë Müller", "zoe@example.test")]   # accents and curly apostrophes, as sent (T-07)
FR_HTML_SUBJECTS = ["Votre café est prêt", "Une très bonne journée", "L’agenda du mois", "Prochaine étape du dossier", "Résumé de votre commande"]  # 5 HTML bodies with entities (T-08)
FR_FILES = [("Facture du mois", "facture-4471.pdf"), ("Photos de la fête", "photos.txt")]   # 2 messages with one file each
FR_FILLERS = 44                                       # 44 filler + 16 named = 60 INBOX messages
FR_FIRST_SENTENCE = "Bonjour, voici la première phrase du corps."   # the reader's first sentence of every filler body (T-08 reads it with the server down)


def _fr_msg(subject, name, addr, body, n, html=None, attach=None, headers=(), to=ADDRESS, in_reply_to=None, sent=False):
    m = email.message.EmailMessage(policy=email.policy.SMTP)
    m["From"] = "%s <%s>" % (name, addr)
    m["To"] = to
    m["Subject"] = subject
    m["Message-ID"] = "<fr-%d@whoosh-check.invalid>" % n
    m["Date"] = "Mon, 09 Nov 2026 %02d:%02d:00 +0000" % (9 + n // 60, n % 60)   # later UID, later date: the specials (added last) sit on the list's first page
    if in_reply_to:
        m["In-Reply-To"] = in_reply_to
        m["References"] = in_reply_to
    for k, v in headers:
        m[k] = v
    m.set_content(body, charset="utf-8", cte="quoted-printable")
    if html:
        m.add_alternative(html, subtype="html", charset="utf-8", cte="quoted-printable")
    if attach:
        m.add_attachment(("Contenu du fichier %s.\n" % attach).encode(), maintype="application", subtype="octet-stream", filename=attach)
    return m.as_bytes(policy=email.policy.SMTP)


def firstrun_inbox():
    """(inbox raws, sent raws): 60 INBOX messages, 3 Sent. Oldest first (UID 1 is the oldest; the list shows the newest on top)."""
    inbox = []
    def add(*a, **k):
        inbox.append(_fr_msg(*a, n=len(inbox) + 1, **k))
    for k in range(1, FR_FILLERS + 1):   # the filler first: the list shows the newest on top, so the named messages below sit on its first page
        add("Message de routine %d" % k, "Expéditeur %d" % k, "sender%d@example.test" % k, "%s Rien d’urgent (%d)." % (FR_FIRST_SENTENCE, k))
    add(FR_ANSWERED, "Camille Roy", "camille@example.test", "%s Peut-on se voir jeudi à dix heures ?" % FR_FIRST_SENTENCE)
    for sub, name, addr in FR_NAMED:
        add(sub, name, addr, "%s Voici ce qu’il faut savoir." % FR_FIRST_SENTENCE)
    add(FR_CADEAU_SUBJECT, "Marie Lefèvre", "marie@example.test", "%s Je t’envoie une idée de %s pour Marie." % (FR_FIRST_SENTENCE, FR_CADEAU_WORD))
    add(FR_REUNION_SUBJECT, "Éric Martin", "eric@example.test", "%s On se retrouve pour la %s de lundi à neuf heures." % (FR_FIRST_SENTENCE, FR_REUNION_WORD))
    for sub in FR_HTML_SUBJECTS:
        add(sub, "Café Bernard", "cafe@example.test", "%s Texte simple." % FR_FIRST_SENTENCE,
            html="<html><body><p>%s</p><p>Caf&eacute; pr&ecirc;t, l&#8217;&eacute;quipe vous attend &amp; vous remercie.</p></body></html>" % FR_FIRST_SENTENCE)
    for sub, f in FR_FILES:
        add(sub, "Service Facturation", "facturation@example.test", "%s Le fichier est joint." % FR_FIRST_SENTENCE, attach=f)
    add(FR_NEWSLETTER, "La Boutique", "news@shop.example.test", "%s Cette semaine : des offres." % FR_FIRST_SENTENCE,
        headers=[("List-Unsubscribe", "<mailto:unsubscribe@shop.example.test>")])
    for q in FR_QUESTIONS:
        add(q, "Client Dupont", QUESTION_SENDER, "%s Pouvez-vous me répondre avant vendredi ?" % FR_FIRST_SENTENCE)
    sent = [_fr_msg("Re: " + FR_ANSWERED, "Owner", ADDRESS, "Jeudi dix heures, c’est bon pour moi.", 101, to="camille@example.test", in_reply_to="<fr-%d@whoosh-check.invalid>" % (FR_FILLERS + 1), sent=True)]
    for i in (102, 103):
        sent.append(_fr_msg("Envoyé %d" % i, "Owner", ADDRESS, "Un message envoyé.", i, to="friend%d@example.test" % i, sent=True))
    return inbox, sent


def second_account(mail):
    """The second account of the first-run walk: a server with NO SPECIAL-USE attribute, its folders named as such servers name them."""
    two = Mail.__new__(Mail)
    two.lock, two.log = mail.lock, mail.log
    two.latency_s = mail.latency_s
    two.boxes = {"INBOX": [(1, _fr_msg("Bienvenue", "Équipe", "team@example.test", "%s Bienvenue." % FR_FIRST_SENTENCE, 201, to=SECOND_ADDRESS))],
                 "Sent Items": [], "Receipts": []}
    two.special = {"INBOX": "", "Sent Items": "", "Receipts": ""}
    two.act, two.flags = False, {}
    two.stores, two.moves, two.appends = [], [], []
    two.second = None
    two.plain, two.change_lists, two.away, two.expunged, two.generation, two.announced = False, [], [], [], 0, 0
    two.state = mail.state
    return two


def deliver(mail, raw):
    """New mail on INBOX: the next UID; an IDLE connection announces it at its next poll. Returns the UID."""
    with mail.lock:
        uid = max([u for u, _ in mail.boxes["INBOX"]] or [0]) + 1
        mail.boxes["INBOX"].append((uid, raw))
    return uid


def move_away(mail, uid, to_folder="Archive", quiet=False):
    """Slice moved (A144): the SERVER moves INBOX mail `uid` to `to_folder` by itself (another client, a rule), so the app's copy goes stale.
    Not a write of the core: `mail.moves` is untouched; the move is in `mail.away` with the log length at that moment. An IDLE connection
    announces the removal (its position, then the count) at its next poll, unless `quiet`: then nothing is said until `announce(mail)`.
    Needs a server started with `act=True` (it has the folder). Returns the UID the mail has in `to_folder`."""
    with mail.lock:
        if to_folder not in mail.boxes:
            raise KeyError("the fake has no folder %r (start it with act=True)" % to_folder)
        inbox = mail.boxes["INBOX"]
        seq = next((i for i, (u, _) in enumerate(inbox, 1) if u == uid), None)
        if seq is None:
            raise KeyError("INBOX holds no UID %d" % uid)
        raw = inbox[seq - 1][1]
        top = max([u for u, _ in mail.boxes[to_folder]] or [0]) + 1
        mail.boxes[to_folder].append((top, raw))
        del inbox[seq - 1]
        mail.generation += 1
        mail.expunged.append((mail.generation, seq))
        mail.away.append((uid, to_folder, seq, len(mail.log)))
        mail.state["quiet"] = bool(quiet)
    return top


def announce(mail):
    """The server stops keeping its news back: the next IDLE poll tells every idling connection what moved."""
    mail.state["quiet"] = False


def command_kind(cmd, args):
    """A command as a kind with no mail text and no secret: `UID FETCH body` / `UID FETCH headers`, `SELECT <folder>`, else the command word."""
    if cmd == "UID":
        words = args.split(" ")
        what = " body" if "BODY.PEEK[]" in args.upper() else (" headers" if "HEADER" in args.upper() else "")
        return "UID %s%s" % (words[0].upper() if words else "", what)
    if cmd in ("SELECT", "EXAMINE"):
        return "%s %s" % (cmd, args.split(" ")[0])
    return cmd


def peak_sessions(spans):
    """The moment the most connections were open, from `Mail.conn_spans`: (count, [(connection, [kinds])] open then). A connection still
    open at the end counts as open to the end."""
    events = sorted([(o, 1, c) for c, (o, _, _) in spans.items()] + [(e, -1, c) for c, (_, e, _) in spans.items() if e is not None], key=lambda x: (x[0], x[1]))
    open_now, best, best_set = set(), 0, []
    for _, step, c in events:
        if step == 1:
            open_now.add(c)
        else:
            open_now.discard(c)
        if len(open_now) > best:
            best, best_set = len(open_now), sorted(open_now)
    return best, [(c, list(spans[c][2])) for c in best_set]


class Mail:
    """The mailboxes: INBOX with `inbox` messages, Sent with one. Shared by every connection. With `act` (slice 5) the server also
    has an Archive and a Junk folder and records every write the core sends: `stores` (folder, uids, +/-FLAGS, flags), `moves`
    (from, uids, to), `appends` (folder, flags, raw). With `corpus` (slice 6, needs `act`) INBOX also holds CORPUS_PER_CLASS
    ordinary messages and Junk CORPUS_PER_CLASS junk ones: what the spam filter's first-run training learns from."""
    # tail-realcore: what the sessions and the full-body fetches looked like on the wire (class defaults: `second_account` builds a Mail with `__new__`)
    sessions = 0           # IMAP connections open now
    max_sessions = 0       # the most that were ever open at once
    conn_seq = 0           # connections accepted so far; the next one is numbered conn_seq
    body_fetches = None    # [(connection number, folder selected on it, [uids asked])] for every UID FETCH that asked BODY.PEEK[] (the whole message), in order
    conn_spans = None      # fix-realcore: {connection number: [opened (monotonic s), closed or None, [command kinds in first-use order]]}, to name the sessions open at the peak

    def __init__(self, inbox=3, act=False, corpus=False, latency_ms=0, rich=False, firstrun=False, refs=False, plain=False):
        self.lock = threading.Lock()
        self.body_fetches = []
        self.conn_spans = {}
        self.latency_s = latency_ms / 1000.0   # perf-realcore-r1: the wait before each SELECT and FETCH answer (0 = none)
        self.boxes = {"INBOX": [], "Sent": []}
        self.special = {"INBOX": "", "Sent": " \\Sent"}
        if act:
            self.boxes.update({"Archive": [], "Junk": []})
            self.special.update({"Archive": " \\Archive", "Junk": " \\Junk"})
        self.act = act
        for n in range(1, inbox + 1):
            self.boxes["INBOX"].append((n, raw_message(n, "INBOX", rich, refs)))
        self.boxes["Sent"].append((1, raw_message(1, "Sent")))
        if corpus and act:
            top = len(self.boxes["INBOX"])
            for i in range(1, CORPUS_PER_CLASS + 1):
                self.boxes["INBOX"].append((top + i, good_corpus_message(i)))
                self.boxes["Junk"].append((i, junk_corpus_message(i)))
        self.flags = {}
        self.stores, self.moves, self.appends = [], [], []
        self.log = []
        self.second = None
        self.plain = plain             # slice moved: a server with no change list (A144); a CONDSTORE or QRESYNC command is recorded in `change_lists` and refused
        self.change_lists = []
        self.away = []                 # slice moved: every mail the SERVER moved out of INBOX by itself (move_away), as (uid, folder, seq, log length at the move)
        self.expunged = []             # (generation, seq): what an IDLE connection announces about those moves, once the server is not quiet
        self.generation = self.announced = 0
        self.state = {"down": False, "quiet": False}   # firstrun: `down` ends every open connection at its next line and refuses new ones (the server stopped)
        if firstrun:   # checks/firstrun-check.py: 60 real-looking messages, Sent of 3, a folder with a non-ASCII name, and a second account
            inbox_raws, sent_raws = firstrun_inbox()
            self.boxes = {"INBOX": list(enumerate(inbox_raws, 1)), FIRSTRUN_SENT: list(enumerate(sent_raws, 1)), "Projects": []}
            self.special = {"INBOX": "", FIRSTRUN_SENT: " \\Sent", "Projects": ""}
            self.second = second_account(self)


def uid_set(spec, uids):
    """The UIDs of an IMAP set ("1:3,7", "4:*") present in `uids`."""
    top = max(uids) if uids else 0
    out = set()
    for part in spec.split(","):
        if ":" in part:
            a, b = part.split(":", 1)
            lo = top if a == "*" else int(a)
            hi = top if b == "*" else int(b)
            lo, hi = min(lo, hi), max(lo, hi)
            out.update(u for u in uids if lo <= u <= hi)
        else:
            u = top if part == "*" else int(part)
            if u in uids:
                out.add(u)
    return sorted(out)


def fetch_line(seq, uid, raw, items, flags=()):
    up = items.upper()
    head, _, text = raw.partition(b"\r\n\r\n")
    head += b"\r\n\r\n"
    parts = [b"UID %d" % uid]
    if "FLAGS" in up:
        parts.append(b"FLAGS (" + " ".join(sorted(flags)).encode() + b")")
    if "INTERNALDATE" in up:
        # the hour moves on past UID 59, as the Date headers do: the engine dates a mail by this arrival date, and with the minute alone a mail delivered
        # during a run (UID 61) was dated 10:01, the oldest of the box, off the list's first page (firstrun-check T-12 on main143 and main147)
        parts.append(b'INTERNALDATE "09-Nov-2026 %02d:%02d:00 +0000"' % (10 + uid // 60, uid % 60))
    if "RFC822.SIZE" in up:
        parts.append(b"RFC822.SIZE %d" % len(raw))
    if "BODY.PEEK[HEADER]" in up:
        parts.append(b"BODY[HEADER] {%d}\r\n" % len(head) + head)
    m = re.search(r"BODY\.PEEK\[TEXT\]<0\.(\d+)>", up)
    if m:
        t = text[: int(m.group(1))]
        parts.append(b"BODY[TEXT]<0> {%d}\r\n" % len(t) + t)
    if "BODY.PEEK[]" in up:
        parts.append(b"BODY[] {%d}\r\n" % len(raw) + raw)
    return b"* %d FETCH (" % seq + b" ".join(parts) + b")\r\n"


def tokens(s):
    """IMAP arguments: atoms and quoted strings (with \\ escapes)."""
    out, i = [], 0
    while i < len(s):
        if s[i] == " ":
            i += 1
        elif s[i] == '"':
            j, t = i + 1, ""
            while j < len(s) and s[j] != '"':
                if s[j] == "\\" and j + 1 < len(s):
                    j += 1
                t += s[j]
                j += 1
            out.append(t)
            i = j + 1
        else:
            j = s.find(" ", i)
            j = len(s) if j < 0 else j
            out.append(s[i:j])
            i = j
    return out


class Imap(socketserver.StreamRequestHandler):
    mail = None

    def send(self, b):
        self.wfile.write(b if isinstance(b, bytes) else b.encode())
        self.wfile.flush()

    def handle(self):
        mail = self.mail
        with mail.lock:
            mail.conn_seq += 1
            conn = mail.conn_seq
            mail.sessions += 1
            mail.max_sessions = max(mail.max_sessions, mail.sessions)
            if mail.conn_spans is not None:
                mail.conn_spans[conn] = [time.monotonic(), None, []]
        try:
            self.serve_session(conn)
        finally:
            with mail.lock:
                mail.sessions -= 1
                if mail.conn_spans is not None and conn in mail.conn_spans:
                    mail.conn_spans[conn][1] = time.monotonic()

    def serve_session(self, conn):
        mail, selected = self.mail, None
        caps = getattr(self, "caps", CAPS)
        self.send("* OK [CAPABILITY %s] whoosh check fake ready\r\n" % caps)
        while True:
            line = self.rfile.readline()
            if not line or mail.state["down"]:
                return
            line = line.decode(errors="replace").rstrip("\r\n")
            tag, _, rest = line.partition(" ")
            cmd, _, args = rest.partition(" ")
            cmd = cmd.upper()
            with mail.lock:
                mail.log.append("%s %s" % (cmd, args if cmd not in ("LOGIN", "AUTHENTICATE") else (args.split(" ")[0] + " (hidden)")))
                if mail.conn_spans is not None and conn in mail.conn_spans:
                    kinds = mail.conn_spans[conn][2]
                    kind = command_kind(cmd, args)
                    if kind not in kinds:
                        kinds.append(kind)
            if mail.plain and cmd != "ENABLE" and re.search(r"CHANGEDSINCE|QRESYNC|CONDSTORE", args, re.I):
                # a server with no change list: the command is recorded (a check reads `change_lists`) and refused, as such a server does
                with mail.lock:
                    mail.change_lists.append("%s %s" % (cmd, args))
                self.send("%s BAD this server keeps no change list\r\n" % tag)
            elif cmd == "CAPABILITY":
                self.send("* CAPABILITY %s\r\n%s OK done\r\n" % (caps, tag))
            elif cmd in ("NOOP", "CHECK", "ENABLE"):
                self.send("%s OK done\r\n" % tag)
            elif cmd == "LOGOUT":
                self.send("* BYE bye\r\n%s OK done\r\n" % tag)
                return
            elif cmd == "LOGIN":
                t = tokens(args)
                if len(t) == 2 and t[1] == PASSWORD:
                    if t[0].lower() == SECOND_ADDRESS and mail.second is not None:
                        mail = mail.second   # the second account's own folders, no SPECIAL-USE
                    self.send("%s OK [CAPABILITY %s] logged in\r\n" % (tag, caps))
                else:
                    self.send("%s NO [AUTHENTICATIONFAILED] The check server refused this password\r\n" % tag)
            elif cmd == "AUTHENTICATE":
                mech = (tokens(args) or [""])[0].upper()
                if mech != "XOAUTH2":
                    self.send("%s NO [AUTHENTICATIONFAILED] only XOAUTH2 is served by AUTHENTICATE here\r\n" % tag)
                    continue
                parts = tokens(args)
                if len(parts) > 1:
                    resp = parts[1]
                else:
                    self.send("+ \r\n")
                    resp = self.rfile.readline().decode(errors="replace")
                if xoauth2_ok(resp):
                    self.send("%s OK [CAPABILITY %s] authenticated\r\n" % (tag, caps))
                else:
                    self.send("%s NO [AUTHENTICATIONFAILED] The check server refused this sign-in token\r\n" % tag)
            elif cmd == "LIST":
                for name in mail.boxes:
                    self.send('* LIST (\\HasNoChildren%s) "/" "%s"\r\n' % (mail.special[name], mutf7(name)))
                self.send("%s OK done\r\n" % tag)
            elif cmd in ("SELECT", "EXAMINE"):
                if mail.latency_s:
                    time.sleep(mail.latency_s)
                name = mutf7_decode(tokens(args)[0]) if args else ""
                if name.upper() == "INBOX":
                    name = "INBOX"
                if name not in mail.boxes:
                    self.send("%s NO no such folder\r\n" % tag)
                    continue
                selected = name
                with mail.lock:
                    msgs = list(mail.boxes[name])
                nxt = (max(u for u, _ in msgs) + 1) if msgs else 1
                self.send("* FLAGS (\\Answered \\Flagged \\Deleted \\Seen \\Draft)\r\n* %d EXISTS\r\n* 0 RECENT\r\n"
                          "* OK [UIDVALIDITY %d] ok\r\n* OK [UIDNEXT %d] ok\r\n%s OK [READ-WRITE] selected\r\n" % (len(msgs), UIDVALIDITY, nxt, tag))
            elif cmd == "UID" or cmd in ("FETCH", "SEARCH"):
                by_uid = cmd == "UID"
                sub, _, sargs = (args.partition(" ") if by_uid else (cmd, " ", args))
                sub = sub.upper()
                with mail.lock:
                    msgs = list(mail.boxes.get(selected, []))
                uids = [u for u, _ in msgs]
                if sub == "SEARCH":
                    m = re.search(r"UID\s+(\S+)", sargs, re.I)
                    found = uid_set(m.group(1), uids) if m else uids
                    self.send("* SEARCH%s\r\n%s OK done\r\n" % ("".join(" %d" % u for u in found), tag))
                elif sub == "FETCH":
                    if mail.latency_s:
                        time.sleep(mail.latency_s)
                    spec, _, items = sargs.partition(" ")
                    if by_uid:
                        want = uid_set(spec, uids)
                    else:
                        seqs = uid_set(spec, list(range(1, len(msgs) + 1)))
                        want = [msgs[s - 1][0] for s in seqs]
                    if "BODY.PEEK[]" in items.upper() and mail.body_fetches is not None:
                        with mail.lock:
                            mail.body_fetches.append((conn, selected, list(want)))
                    for seq, (u, raw) in enumerate(msgs, 1):
                        if u in want:
                            self.send(fetch_line(seq, u, raw, items, mail.flags.get((selected, u), ())))
                    self.send("%s OK done\r\n" % tag)
                elif sub == "STORE" and mail.act:
                    spec, _, rest = sargs.partition(" ")
                    op, _, fl = rest.partition(" ")
                    names = fl.strip().strip("()").split()
                    hit = uid_set(spec, uids)
                    with mail.lock:
                        for u in hit:
                            cur = set(mail.flags.get((selected, u), ()))
                            cur = cur | set(names) if op.startswith("+") else (cur - set(names) if op.startswith("-") else set(names))
                            mail.flags[(selected, u)] = tuple(sorted(cur))
                        mail.stores.append((selected, hit, op.split(".")[0], names))
                    self.send("%s OK stored\r\n" % tag)
                elif sub in ("MOVE", "COPY") and mail.act:
                    spec, _, dest = sargs.partition(" ")
                    dest = tokens(dest)[0] if dest.strip() else ""
                    if dest not in mail.boxes:
                        self.send("%s NO [TRYCREATE] no such folder\r\n" % tag)
                        continue
                    hit = uid_set(spec, uids)
                    with mail.lock:
                        moving = [(u, r) for u, r in mail.boxes[selected] if u in hit]
                        top = max([u for u, _ in mail.boxes[dest]] or [0])
                        for i, (_, r) in enumerate(moving, 1):
                            mail.boxes[dest].append((top + i, r))
                        if sub == "MOVE":
                            mail.boxes[selected] = [(u, r) for u, r in mail.boxes[selected] if u not in hit]
                        mail.moves.append((selected, hit, dest, sub))
                    self.send("%s OK [COPYUID %d %s %s] done\r\n" % (tag, UIDVALIDITY, ",".join(map(str, hit)) or "0", ",".join(str(top + i) for i in range(1, len(hit) + 1)) or "0"))
                elif sub == "EXPUNGE" and mail.act:
                    with mail.lock:
                        gone = [u for u in uid_set(sargs.strip(), uids) if "\\Deleted" in mail.flags.get((selected, u), ())]
                        mail.boxes[selected] = [(u, r) for u, r in mail.boxes[selected] if u not in gone]
                    self.send("%s OK expunged\r\n" % tag)
                else:
                    self.send("%s BAD not served by the check fake\r\n" % tag)
            elif cmd == "APPEND" and mail.act:
                m = re.match(r'("?)([^" ]+)\1\s*(\([^)]*\))?\s*("[^"]*")?\s*\{(\d+)\+?\}\s*$', args)
                if not m:
                    self.send("%s BAD APPEND not understood\r\n" % tag)
                    continue
                box, fl, n = m.group(2), (m.group(3) or "()").strip("()").split(), int(m.group(5))
                if "+}" not in args:
                    self.send("+ ready\r\n")
                raw = self.rfile.read(n)
                self.rfile.readline()
                if box not in mail.boxes:
                    self.send("%s NO [TRYCREATE] no such folder\r\n" % tag)
                    continue
                with mail.lock:
                    uid = max([u for u, _ in mail.boxes[box]] or [0]) + 1
                    mail.boxes[box].append((uid, raw))
                    mail.flags[(box, uid)] = tuple(sorted(fl))
                    mail.appends.append((box, fl, raw))
                self.send("%s OK [APPENDUID %d %d] appended\r\n" % (tag, UIDVALIDITY, uid))
            elif cmd == "IDLE":
                self.send("+ idling\r\n")
                with mail.lock:
                    seen = len(mail.boxes.get(selected, []))
                    seen_gen = mail.announced   # what is left to say since the last announcement (a connection that starts mid-quiet still says it)
                while True:
                    # Poll the socket, never time out the reader: a socket file refuses every read after its first
                    # timeout ("cannot read from timed out object"), which closed every IDLE connection a second in.
                    ready, _, _ = select.select([self.connection], [], [], IDLE_POLL_S)
                    if mail.state["down"]:
                        return
                    if not ready:
                        if mail.state.get("quiet"):
                            continue    # slice moved: the server keeps its news back until `announce` (the app has not heard of a move yet)
                        with mail.lock:
                            now = len(mail.boxes.get(selected, []))
                            gone = [q for g, q in mail.expunged if g > seen_gen] if selected == "INBOX" else []
                            seen_gen = mail.announced = mail.generation
                        for q in gone:
                            self.send("* %d EXPUNGE\r\n" % q)     # a server with no change list announces a removal by its position, then the count
                        if now != seen or gone:
                            self.send("* %d EXISTS\r\n" % now)
                            seen = now
                        continue
                    l2 = self.rfile.readline()
                    if not l2 or l2.strip().upper() == b"DONE":
                        break
                if not l2:
                    return
                self.send("%s OK IDLE done\r\n" % tag)
            else:
                self.send("%s BAD not served by the check fake\r\n" % tag)


class Smtp(socketserver.StreamRequestHandler):
    def send(self, s):
        self.wfile.write(s.encode())
        self.wfile.flush()

    def ok(self, user, pw):
        return pw == PASSWORD

    def handle(self):
        self.send("220 whoosh-check ESMTP fake\r\n")
        while True:
            line = self.rfile.readline()
            if not line:
                return
            words = line.decode(errors="replace").strip().split(" ")
            verb = words[0].upper()
            if verb in ("EHLO", "HELO"):
                self.send("250-whoosh-check\r\n250-AUTH PLAIN LOGIN XOAUTH2\r\n250 OK\r\n")
            elif verb == "AUTH" and len(words) > 1 and words[1].upper() == "PLAIN":
                b64 = words[2] if len(words) > 2 else None
                if b64 is None:
                    self.send("334 \r\n")
                    b64 = self.rfile.readline().decode().strip()
                parts = base64.b64decode(b64).split(b"\0")
                good = len(parts) == 3 and self.ok(parts[1].decode(), parts[2].decode())
                self.send("235 2.7.0 accepted\r\n" if good else "535 5.7.8 The check server refused this password\r\n")
            elif verb == "AUTH" and len(words) > 1 and words[1].upper() == "XOAUTH2":
                b64 = words[2] if len(words) > 2 else None
                if b64 is None:
                    self.send("334 \r\n")
                    b64 = self.rfile.readline().decode().strip()
                self.server.auths.append("XOAUTH2")
                self.send("235 2.7.0 accepted\r\n" if xoauth2_ok(b64) else "535 5.7.8 The check server refused this sign-in token\r\n")
            elif verb == "AUTH" and len(words) > 1 and words[1].upper() == "LOGIN":
                self.send("334 VXNlcm5hbWU6\r\n")
                user = base64.b64decode(self.rfile.readline().strip()).decode()
                self.send("334 UGFzc3dvcmQ6\r\n")
                pw = base64.b64decode(self.rfile.readline().strip()).decode()
                self.send("235 2.7.0 accepted\r\n" if self.ok(user, pw) else "535 5.7.8 The check server refused this password\r\n")
            elif verb == "MAIL" and self.server.refuse_mail > 0:
                # lead, 13 Nov (owner pictures 208 and 209): the server ANSWERS and refuses, `refuse_mail` times: 550 (the card "Not sent", no
                # automatic retry) or, with `refuse_code` 451, a try-later answer (the engine retries by itself)
                self.server.refuse_mail -= 1
                self.server.mail_froms += 1
                self.server.refusals.append(self.server.refuse_code)
                self.send("451 4.7.0 Temporary server error. Please try again later. CPRX1\r\n" if self.server.refuse_code == 451 else "550 5.7.1 The check server refused this message\r\n")
            elif verb == "MAIL" and self.server.reset_mail > 0:
                # fix-realcore round 15 (--slice-sendfail): the connection is reset at MAIL FROM (SO_LINGER 0, then close: the client reads
                # "Connection reset by peer", the owner's words on main115), `reset_mail` times; later sessions are served
                self.server.reset_mail -= 1
                self.server.mail_froms += 1
                self.server.resets.append("mail_from")
                self.request.setsockopt(socket.SOL_SOCKET, socket.SO_LINGER, struct.pack("ii", 1, 0))
                self.rfile.close()
                self.request.close()
                return
            elif verb == "MAIL" or verb == "RCPT":
                self.server.mail_froms += verb == "MAIL"
                self.send("250 2.1.0 OK\r\n")
            elif verb == "DATA":
                self.send("354 end with a dot\r\n")
                lines = []
                while True:
                    l2 = self.rfile.readline()
                    if not l2 or l2 in (b".\r\n", b".\n"):
                        break
                    lines.append(l2[1:] if l2.startswith(b"..") else l2)
                self.server.messages.append(b"".join(lines))
                self.send("250 2.0.0 queued\r\n")
            elif verb == "QUIT":
                self.send("221 bye\r\n")
                return
            elif verb in ("NOOP", "RSET"):
                self.send("250 OK\r\n")
            else:
                self.send("502 not served by the check fake\r\n")


class Server(socketserver.ThreadingTCPServer):
    allow_reuse_address = True
    daemon_threads = True

    def __init__(self, *a, **k):
        super().__init__(*a, **k)
        self.auths = []   # the SMTP mechanisms the sign-in check used (no credential is kept)
        self.messages = []   # slice 5: every message the SMTP side accepted (DATA), as bytes
        self.refuse_mail = 0   # how many MAIL FROM commands are answered by a refusal (see the handler)
        self.refuse_code = 550
        self.refusals = []
        self.reset_mail = 0   # fix-realcore round 15: how many MAIL FROM commands are answered by a reset (the SMTP side only)
        self.mail_froms = 0   # every MAIL FROM received, the reset ones included: the tries to send
        self.resets = []      # the step of each reset
        self.errors = []  # every handler that died, in its own words: a check reads a dead fake as its own failure

    def handle_error(self, request, client_address):
        self.errors.append("%s: %s" % sys.exc_info()[:2])
        super().handle_error(request, client_address)


def start(imap_port, smtp_port, inbox=3, act=False, corpus=False, latency_ms=0, rich=False, firstrun=False, refs=False, plain=False):
    """Both servers on 127.0.0.1 in background threads. Returns (mail, imap_server, smtp_server); raises OSError when a
    port cannot be bound. `act` (slice 5): Archive and Junk folders, MOVE, and every write recorded. `corpus` (slice 6): the
    spam filter's first-run mail in INBOX and Junk. `latency_ms` and `rich`: see the head of this file. `firstrun`: the first-run walk's
    corpus (firstrun_inbox) and its second account (SECOND_ADDRESS, no SPECIAL-USE); the SMTP side keeps every DATA in `smtp.messages`.
    `refs`: each INBOX mail carries a References chain (`ref_chain`). `plain` (slice moved, SPEC A144): a server with no change list, as Microsoft 365:
    CONDSTORE and QRESYNC are not advertised and a command asking for one is recorded in `mail.change_lists` and refused. `move_away` and
    `announce` move mail by the server's own hand (needs `act`)."""
    mail = Mail(inbox, act, corpus, latency_ms, rich, firstrun, refs, plain)
    handler = type("ImapFor", (Imap,), {"mail": mail, "caps": CAPS + (" MOVE" if act else "")})
    imap = Server(("127.0.0.1", imap_port), handler)
    smtp = Server(("127.0.0.1", smtp_port), Smtp)
    for s in (imap, smtp):
        threading.Thread(target=s.serve_forever, daemon=True).start()
    return mail, imap, smtp


def idle_holds(mail, port, hold_s=1.6):
    """IDLE as the core holds it: longer than the fake's own poll, a message arriving meanwhile, then DONE; the
    connection still serves after it (sync-bye-r2: the handler died on its second poll, the reader refusing every read
    after its first timeout, so every IDLE connection closed about a second in)."""
    import time
    c = socket.create_connection(("127.0.0.1", port), timeout=5)
    f = c.makefile("rb")
    def say(line):
        c.sendall(line.encode() + b"\r\n")
    def until(tag):
        got = []
        while True:
            line = f.readline()
            assert line, "the fake closed the connection; it read so far: %r" % got
            got.append(line)
            if line.startswith(tag.encode() + b" "):
                return got
    f.readline()
    say('a1 LOGIN "%s" "%s"' % (ADDRESS, PASSWORD)); until("a1")
    say("a2 SELECT INBOX"); until("a2")
    say("a3 IDLE")
    assert f.readline().startswith(b"+"), "no continuation for IDLE"
    time.sleep(hold_s / 2)
    with mail.lock:
        n = len(mail.boxes["INBOX"]) + 1
        mail.boxes["INBOX"].append((n, raw_message(n, "INBOX")))
    time.sleep(hold_s / 2)
    say("DONE")
    got = until("a3")
    assert b"* %d EXISTS\r\n" % n in got and got[-1].startswith(b"a3 OK"), got
    say("a4 NOOP")
    assert until("a4")[-1].startswith(b"a4 OK")
    c.close()
    with mail.lock:
        mail.boxes["INBOX"].pop()


def selftest():
    import imaplib
    import smtplib
    mail, imap, smtp = start(0, 0)
    ip, sp = imap.server_address[1], smtp.server_address[1]
    c = imaplib.IMAP4("127.0.0.1", ip)
    c.login(ADDRESS, PASSWORD)
    typ, boxes = c.list()
    assert typ == "OK" and any(b"\\Sent" in b for b in boxes), boxes
    c.select("INBOX")
    typ, data = c.uid("SEARCH", "UID 1:4294967294")
    assert data[0].split() == [b"1", b"2", b"3"], data
    typ, data = c.uid("FETCH", "1:3", "(UID FLAGS INTERNALDATE RFC822.SIZE BODY.PEEK[HEADER] BODY.PEEK[TEXT]<0.2048>)")
    assert typ == "OK" and b"Check message 3" in b"".join(x[1] for x in data if isinstance(x, tuple)), data
    c.logout()
    bad = imaplib.IMAP4("127.0.0.1", ip)
    try:
        bad.login(ADDRESS, "wrong")
        raise AssertionError("a wrong password signed in")
    except imaplib.IMAP4.error as e:
        assert "refused" in str(e), e
    s = smtplib.SMTP("127.0.0.1", sp)
    s.ehlo()
    s.login(ADDRESS, PASSWORD)
    s.quit()
    # slice 2b: XOAUTH2 with the issued token signs in on both; a wrong token is refused
    sasl = base64.b64encode(("user=%s\x01auth=Bearer %s\x01\x01" % (ADDRESS, ACCESS_TOKEN)).encode()).decode()
    c = imaplib.IMAP4("127.0.0.1", ip)
    typ, _ = c.authenticate("XOAUTH2", lambda _: base64.b64decode(sasl))
    assert typ == "OK", typ
    c.logout()
    c = imaplib.IMAP4("127.0.0.1", ip)
    try:
        c.authenticate("XOAUTH2", lambda _: b"user=x\x01auth=Bearer wrong\x01\x01")
        raise AssertionError("a wrong token signed in")
    except imaplib.IMAP4.error as e:
        assert "refused" in str(e), e
    s = smtplib.SMTP("127.0.0.1", sp)
    s.ehlo()
    code, _ = s.docmd("AUTH", "XOAUTH2 " + sasl)
    assert code == 235, code
    s.quit()
    assert smtp.auths == ["XOAUTH2"], smtp.auths
    idle_holds(mail, ip)
    assert imap.errors == [] and smtp.errors == [], (imap.errors, smtp.errors)
    imap.shutdown(); smtp.shutdown()
    print("selftest: IMAP and SMTP fakes answer as the core expects (password and XOAUTH2)")
    reset_selftest()
    firstrun_selftest()
    moved_selftest()
    body_selftest()
    crawl_selftest()
    return 0


def reset_selftest():
    """fix-realcore round 15: with `reset_mail` 1 the first MAIL FROM is answered by a reset (the client's own words: reset by peer), the next session
    sends; one message kept, two MAIL FROM counted."""
    import smtplib
    mail, imap, smtp = start(0, 0)
    smtp.reset_mail = 1
    sp = smtp.server_address[1]
    s = smtplib.SMTP("127.0.0.1", sp)
    s.ehlo()
    s.login(ADDRESS, PASSWORD)
    words = None
    try:
        s.sendmail(ADDRESS, ["friend@example.test"], b"Subject: x\r\n\r\nhello\r\n")
    except (ConnectionResetError, smtplib.SMTPServerDisconnected) as e:
        words = "%s: %s" % (type(e).__name__, e)
    assert words is not None, "the first MAIL FROM was not broken"
    s = smtplib.SMTP("127.0.0.1", sp)
    s.ehlo()
    s.login(ADDRESS, PASSWORD)
    s.sendmail(ADDRESS, ["friend@example.test"], b"Subject: x\r\n\r\nhello\r\n")
    s.quit()
    assert len(smtp.messages) == 1 and smtp.mail_froms == 2 and smtp.resets == ["mail_from"], (len(smtp.messages), smtp.mail_froms, smtp.resets)
    assert smtp.errors == [], smtp.errors
    imap.shutdown(); smtp.shutdown()
    print("selftest: a reset at MAIL FROM (the client read %s), then a session that sends: 1 message, 2 MAIL FROM" % words)


def firstrun_selftest():
    """The first-run corpus and its second account, read back through Python's own imaplib and email parser (nothing assumed)."""
    import email as em
    import imaplib
    mail, imap, smtp = start(0, 0, firstrun=True)
    ip = imap.server_address[1]
    try:
        c = imaplib.IMAP4("127.0.0.1", ip)
        c.login(ADDRESS, PASSWORD)
        typ, boxes = c.list()
        names = [b.decode() for b in boxes]
        assert any("\\Sent" in b and "&AMk-l&AOk-ments envoy&AOk-s" in b for b in names), names
        assert any(b.endswith('"Projects"') and "\\Sent" not in b for b in names), names
        typ, data = c.select("INBOX")
        assert int(data[0]) == 60, data
        typ, d = c.uid("FETCH", "1:*", "(BODY.PEEK[])")
        raws = [x[1] for x in d if isinstance(x, tuple)]
        assert len(raws) == 60, len(raws)
        msgs = [em.message_from_bytes(r, policy=em.policy.default) for r in raws]
        subjects = [str(m["Subject"]) for m in msgs]
        for want in ("L’offre d’Oranta", "Réunion de l’équipe", "RE: Devis n° 4471", "Question sur le devis n° 4471"):
            assert want in subjects, (want, subjects[:8])
        assert str(msgs[FR_FILLERS + 1]["From"]).startswith("Hélène Dupré"), msgs[FR_FILLERS + 1]["From"]
        bodies = [m.get_body(("plain",)).get_content() for m in msgs]
        assert [i for i, b in enumerate(bodies) if "cadeau" in b] == [FR_FILLERS + 4] and not any("cadeau" in s for s in subjects)
        assert len([m for m in msgs if m.get_body(("html",)) is not None and "&eacute;" in m.get_body(("html",)).get_content()]) == 5
        assert sorted(f for m in msgs for f in [a.get_filename() for a in m.iter_attachments()]) == ["facture-4471.pdf", "photos.txt"]
        assert sum(1 for m in msgs if m["List-Unsubscribe"]) == 1
        typ, data = c.select('"&AMk-l&AOk-ments envoy&AOk-s"')
        assert typ == "OK" and int(data[0]) == 3, (typ, data)
        c.logout()
        c = imaplib.IMAP4("127.0.0.1", ip)
        c.login(SECOND_ADDRESS, PASSWORD)
        typ, boxes = c.list()
        names = [b.decode() for b in boxes]
        assert len(names) == 3 and not any("\\" in b.split(")")[0].replace("\\HasNoChildren", "") for b in names), names
        assert any(b.endswith('"Sent Items"') for b in names) and any(b.endswith('"Receipts"') for b in names), names
        typ, data = c.select("INBOX")
        assert int(data[0]) == 1, data
        c.logout()
        assert mutf7("Éléments envoyés") == "&AMk-l&AOk-ments envoy&AOk-s" and mutf7_decode(mutf7("A&B é")) == "A&B é"
        mail.state["down"] = True    # the server stopped: an open connection is ended at its next line and a new one is not served
        try:
            d = imaplib.IMAP4("127.0.0.1", ip)
            d.login(ADDRESS, PASSWORD)
            raise AssertionError("a stopped server signed a client in")
        except (imaplib.IMAP4.abort, imaplib.IMAP4.error, OSError):
            pass
        mail.state["down"] = False
        d = imaplib.IMAP4("127.0.0.1", ip)
        d.login(ADDRESS, PASSWORD)    # the control: the same call works once the server is back
        d.logout()
    finally:
        imap.shutdown(); smtp.shutdown()
    print("selftest: firstrun corpus: 60 INBOX messages (accents, curly apostrophes, 5 HTML with entities, 2 files, a body-only word), Sent of 3 under a non-ASCII SPECIAL-USE name, a second account with no SPECIAL-USE")


def body_selftest():
    """The two body shapes, read back through Python's own imaplib and email parser: multipart quoted-printable and base64 parts that decode to the words the
    hidden `--slice-body` looks for, boundary and part headers present in the RAW message (so a body that still shows them is the raw one), and no soft break left once decoded."""
    import email as em
    import imaplib
    mail, imap, smtp = start(0, 0, inbox=0)
    try:
        load_body_mail(mail)
        c = imaplib.IMAP4("127.0.0.1", imap.server_address[1])
        c.login(ADDRESS, PASSWORD)
        c.select("INBOX")
        typ, d = c.uid("FETCH", "1:2", "(BODY.PEEK[])")
        raws = [x[1] for x in d if isinstance(x, tuple)]
        assert len(raws) == 2, len(raws)
        c.logout()
        for raw, subject, words, bound in ((raws[0], SWEET_SUBJECT, SWEET_WORDS, BODY_BOUNDARY), (raws[1], SOYO_SUBJECT, SOYO_WORDS, SOYO_BOUNDARY)):
            assert ("--" + bound).encode() in raw and b"Content-Type" in raw, subject
            assert b"=\r\n" in raw or subject == SOYO_SUBJECT, "the raw sweetgreen text holds soft line breaks"
            m = em.message_from_bytes(raw, policy=em.policy.default)
            assert str(m["Subject"]) == subject and m.is_multipart(), subject
            text = m.get_body(("plain",)).get_content()
            assert all(w in " ".join(text.split()) for w in words), (subject, text)
            assert "=\n" not in text and "=\r\n" not in text and bound not in text and "Content-" not in text, (subject, text)
            html = m.get_body(("html",)).get_content()
            assert "Content-" not in html and bound not in html and "=3D" not in html, (subject, html)
            # fix-realcore: the reader shows the html alternative of a multipart/alternative mail (Sources/Features/MessageBody/MessageBodyContent.swift
            # `document(for:)`), so the words the hidden `body-holds-the-decoded-words` asks for are in the html part too, as in a real alternative mail.
            assert all(w in " ".join(re.sub(r"<[^>]*>", " ", html).split()) for w in words), ("the html alternative holds the same words", subject)
        assert b"almo=\r\nnds" in raws[0] and b"cor=\r\nner" in raws[1], "soft line breaks inside words"
        assert imap.errors == [], imap.errors
    finally:
        imap.shutdown(); smtp.shutdown()
    print("selftest: body shapes: a sweetgreen-shaped multipart quoted-printable mail and a Soyo-World-shaped one (base64 text, quoted-printable html) decode to their words and carry boundary, part headers and soft breaks raw")


def em_date(raw):
    """The Date header of a raw message as a datetime."""
    import email.utils
    head = raw.decode(errors="replace").split("\r\n\r\n")[0]
    line = next(l for l in head.split("\r\n") if l.lower().startswith("date:"))
    return email.utils.parsedate_to_datetime(line.split(":", 1)[1].strip())


def crawl_selftest():
    """The crawl's mailbox and the wire record: every folder holds its count, the word and the meaning each sit in one Archive mail, full-body fetches are recorded
    per connection with the folder selected on it, and the most sessions ever open at once is counted."""
    import imaplib
    mail, imap, smtp = start(0, 0, inbox=0, act=True)
    try:
        total = load_crawl_mail(mail)
        assert total == 100 and [len(mail.boxes[f]) for f, _ in CRAWL_PLAN] == [25, 65, 10], total
        # fix-realcore: the word and the meaning mail are below the newest BODY_PREFETCH_COUNT (50) of Archive by date: only the crawl stores them
        dated = sorted(mail.boxes["Archive"], key=lambda m: em_date(m[1]), reverse=True)
        assert [u for u, _ in dated] == list(range(65, 0, -1)), "Archive's dates fall with the UID"
        newest = {u for u, _ in dated[:50]}
        assert CRAWL_WORD_UID not in newest and CRAWL_MEANING_UID not in newest, "the word and the meaning mail are the crawl's alone"
        preview = ledger_number("IMAP_PREVIEW_BYTES")
        assert preview, "the ledger names IMAP_PREVIEW_BYTES"
        for uid, words in ((CRAWL_WORD_UID, CRAWL_WORD), (CRAWL_MEANING_UID, "harbour")):
            raw = dict(mail.boxes["Archive"])[uid]
            text = raw.split(b"\r\n\r\n", 1)[1]
            assert text.find(words.encode()) > preview, ("the sentence is past the preview bytes the sync fetches", uid, text.find(words.encode()), preview)
        every = [raw.decode().lower() for f, _ in CRAWL_PLAN for _, raw in mail.boxes[f]]
        assert sum(1 for t in every if CRAWL_WORD in t) == 1 and not any(CRAWL_WORD in t.split("\r\n\r\n")[0] for t in every)
        assert not any(w in t for t in every for w in CRAWL_MEANING_QUERY.split()), "the meaning query shares no word with any mail"
        a = imaplib.IMAP4("127.0.0.1", imap.server_address[1]); a.login(ADDRESS, PASSWORD)
        b = imaplib.IMAP4("127.0.0.1", imap.server_address[1]); b.login(ADDRESS, PASSWORD)
        assert mail.max_sessions == 2 and mail.sessions == 2, (mail.max_sessions, mail.sessions)
        a.select("INBOX"); b.select("Archive")
        a.uid("FETCH", "16:25", "(UID BODY.PEEK[])")
        b.uid("FETCH", "1:3", "(UID BODY.PEEK[])")
        a.uid("FETCH", "1:25", "(UID FLAGS BODY.PEEK[TEXT]<0.512>)")      # a preview fetch is not a full-body fetch
        a.logout(); b.logout()
        deadline = time.time() + 5
        while mail.sessions and time.time() < deadline:
            time.sleep(0.05)
        assert mail.sessions == 0 and mail.max_sessions == 2, (mail.sessions, mail.max_sessions)
        most, at_peak = peak_sessions(mail.conn_spans)
        assert most == 2 and len(at_peak) == 2 and all("UID FETCH body" in k and "LOGIN" in k for _, k in at_peak), (most, at_peak)
        assert not any("hidden" in x or "@" in x for _, k in at_peak for x in k), "the kinds hold no login and no address"
        rec = [(f, u[0], u[-1], len(u)) for _, f, u in mail.body_fetches]
        assert rec == [("INBOX", 16, 25, 10), ("Archive", 1, 3, 3)] and mail.body_fetches[0][0] != mail.body_fetches[1][0], mail.body_fetches
        assert imap.errors == [], imap.errors
    finally:
        imap.shutdown(); smtp.shutdown()
    print("selftest: crawl mailbox: INBOX 25, Archive 65 (its oldest 15 past the prefetch's 50), Sent 10; one word and one meaning each in one Archive mail; full-body fetches recorded per connection and folder; sessions counted")


def moved_selftest():
    """Slice moved: the plain mode (no change list) and the server's own move, read back through Python's own imaplib and a raw IDLE socket."""
    import email as em
    import imaplib
    mail, imap, smtp = start(0, 0, inbox=3, act=True, refs=True, plain=True)
    ip = imap.server_address[1]
    try:
        c = imaplib.IMAP4("127.0.0.1", ip)
        assert not any(x in " ".join(c.capabilities).upper() for x in ("CONDSTORE", "QRESYNC")), c.capabilities
        c.login(ADDRESS, PASSWORD)
        c.select("INBOX")
        typ, d = c.uid("FETCH", "2", "(BODY.PEEK[HEADER])")
        head = em.message_from_bytes(d[0][1])
        assert head["Message-ID"] == message_id(2) and head["References"].split() == ref_chain(2), (head["Message-ID"], head["References"])
        try:
            c.uid("FETCH", "1:*", "(FLAGS)", "(CHANGEDSINCE 1)")
            raise AssertionError("a plain server answered a CHANGEDSINCE fetch")
        except imaplib.IMAP4.error as e:
            assert "no change list" in str(e), e
        assert len(mail.change_lists) == 1 and "CHANGEDSINCE" in mail.change_lists[0], mail.change_lists
        c.logout()
        # an idling connection hears a loud move at once, a quiet one only after announce()
        s = socket.create_connection(("127.0.0.1", ip), timeout=5)
        f = s.makefile("rb")
        def say(line):
            s.sendall(line.encode() + b"\r\n")
        def until(tag):
            got = []
            while True:
                line = f.readline()
                assert line, "the fake closed the connection; it read so far: %r" % got
                got.append(line)
                if line.startswith(tag.encode() + b" "):
                    return got
        f.readline()
        say('a1 LOGIN "%s" "%s"' % (ADDRESS, PASSWORD)); until("a1")
        say("a2 SELECT INBOX"); until("a2")
        say("a3 IDLE"); assert f.readline().startswith(b"+")
        got_uid = move_away(mail, 2, "Archive")
        assert got_uid == 1 and [u for u, _ in mail.boxes["INBOX"]] == [1, 3] and [u for u, _ in mail.boxes["Archive"]] == [1], (got_uid, mail.boxes["INBOX"])
        heard = f.readline() + f.readline()
        assert heard == b"* 2 EXPUNGE\r\n* 2 EXISTS\r\n", heard
        move_away(mail, 1, "Archive", quiet=True)
        assert not select.select([s], [], [], 1.6)[0], "a quiet server spoke"
        announce(mail)
        heard = f.readline() + f.readline()
        assert heard == b"* 1 EXPUNGE\r\n* 1 EXISTS\r\n", heard
        say("DONE"); until("a3")
        s.close()
        assert mail.moves == [] and [(u, to, q) for u, to, q, _ in mail.away] == [(2, "Archive", 2), (1, "Archive", 1)], (mail.moves, mail.away)
        assert [u for u, _ in mail.boxes["INBOX"]] == [3], mail.boxes["INBOX"]
        try:
            move_away(mail, 99, "Archive")
            raise AssertionError("a move of a UID INBOX does not hold was accepted")
        except KeyError:
            pass
        assert imap.errors == [], imap.errors
    finally:
        imap.shutdown(); smtp.shutdown()
    print("selftest: moved-away mode: a plain server (no CONDSTORE, a change-list command recorded and refused), References chains served, move_away heard by an IDLE connection as EXPUNGE and EXISTS, held back while quiet until announce")


def serve(argv):
    """`--serve [--latency-ms N] [--messages N] [--act] [--ports-file PATH]`: until interrupted. `--act` is the real-core slices' mode (MOVE, APPEND, every write
    recorded), which a Mac copy and an iPhone copy sharing this one server need (scripts/iphone-rig.sh --with-mac --shared-server). `--ports-file` gets `<imap>:<smtp>`,
    written whole once both servers listen. Both servers are bound to 127.0.0.1 only."""
    def opt(name, default):
        return int(argv[argv.index(name) + 1]) if name in argv else default
    n = opt("--messages", 3)
    mail, imap, smtp = start(0, 0, inbox=n, latency_ms=opt("--latency-ms", 0), rich="--messages" in argv, act="--act" in argv)
    print("imap %d smtp %d" % (imap.server_address[1], smtp.server_address[1]), flush=True)
    if "--ports-file" in argv:
        path = argv[argv.index("--ports-file") + 1]
        with open(path + ".tmp", "w") as f:
            f.write("%d:%d\n" % (imap.server_address[1], smtp.server_address[1]))
        os.replace(path + ".tmp", path)
    try:
        threading.Event().wait()
    except KeyboardInterrupt:
        pass
    return 0


if __name__ == "__main__":
    sys.exit(selftest() if "--selftest" in sys.argv else serve(sys.argv) if "--serve" in sys.argv else 2)

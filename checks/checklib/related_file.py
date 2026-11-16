"""A corpus where a related mail found by its attachment is certain (item bad881fa; hid-search round 23).

main111 and main112 never reached the state `search-related-from-attachment` (checks/shots-sheet.py) nor par-searchmac's case
J2.4/par-a-related-mail-from-an-attachment-opens-and-the-app-keeps-answering: in fixtures/tests-j2.json the one readable file (msg-s02's warehouse photo)
belongs to an invoice whose own text is about prices, so no query made the photo the reason msg-s02 is related. Guessing queries against the real model
failed twice. This corpus is the J2 corpus plus ONE mail whose text says almost nothing ("See attached.") and whose one readable attachment holds a clear
paragraph on a subject no other mail touches (bee keeping). The queries are about that subject in words that are in no field of that mail, of its file, or
of any other mail (`letters_absent` proves it), so the mail is never a match, its file is the piece closest to them, and the body ("See attached.") is not.

Written at run time beside the run's results (never under fixtures/: the J2 corpus and every count read from it stay as they are), loaded with
`fixture corpus <path>`; the caller loads the J2 corpus again after.
"""
import json
import os

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
BASE = os.path.join(ROOT, "fixtures", "tests-j2.json")
MAIL_ID = "msg-r01"
FILE = "roof-notes.pdf"
FILE_TEXT = ("Beehive notes: the hives, their queens, the honey and the swarms. "   # a first line that names the subject (main148: 0.52 against 0.54 without it)
             "On the roof the three beehives came through the summer well: the queen of the second colony was replaced in June, the colonies stored "
             "enough honey for the winter, and the swarm stayed calm when the frames were lifted out for the inspection.")
# The first two read 0.48 and 0.50 against cut-offs of 0.53 and 0.52 on the real model (main144, main148: the file's name is not part of the piece the model
# reads). The next ones name the subject itself in words the file does not hold ("beehives" holds neither "bees" nor "beekeeping").
QUERIES = ["apiary pollination nectar", "apiarist smoker veil", "beekeeping", "rooftop bees honeycomb"]
NAME = "tests-j2-related-file.json"


def mail():
    return {"id": MAIL_ID, "thread": "thr-r01", "account": "acc-04", "from_name": "Ines Varga", "from": "ines@garden-club-example.org",
            "date": "2026-09-20T09:15:00", "subject": "From Saturday", "body": "See attached.", "attachments": [FILE],
            "role": "related by its attachment only (hid-search round 23, item bad881fa): a text that says nothing, a file on a subject no other mail touches"}


def corpus(base=None):
    """The J2 corpus plus the one mail and its file's text, as a dict."""
    base = base or json.load(open(BASE, encoding="utf-8"))
    out = dict(base)
    out["messages"] = list(base["messages"]) + [mail()]
    ac = dict(base.get("attachment_content") or {})
    ac[FILE] = {"message": MAIL_ID, "kind": "pdf", "state": "indexed", "text": FILE_TEXT, "mime": "application/pdf", "size_bytes": 48210}
    out["attachment_content"] = ac
    return out


def write(dest_dir):
    """Writes the corpus into `dest_dir` (made if absent); returns its path."""
    os.makedirs(dest_dir, exist_ok=True)
    path = os.path.join(dest_dir, NAME)
    with open(path, "w", encoding="utf-8") as f:
        json.dump(corpus(), f, ensure_ascii=False, indent=1)
    return path


def letters_absent(c=None):
    """[(query word, where it was found)] for every query word that some field of some mail, or a file's text, holds (inside a longer word too, as the search
    matches, SPEC A149). Empty when no query can make any mail a match."""
    c = c or corpus()
    hay = [(m["id"], " ".join([m.get("subject", ""), m.get("body", ""), m.get("from_name", ""), m.get("from", "")] + list(m.get("attachments") or [])).lower())
           for m in c["messages"]]
    hay += [(name, (e.get("text") or "").lower()) for name, e in (c.get("attachment_content") or {}).items() if isinstance(e, dict)]
    found = []
    for q in QUERIES:
        for w in q.lower().split():
            found += [(w, where) for where, text in hay if w in text]
    return found

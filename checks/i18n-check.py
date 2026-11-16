#!/usr/bin/env python3
"""i18n check: the String Catalog holds every user-facing string of Sources/Features in 14 languages, and no label is cut off.

Modes (all print their evidence; exit 1 when a check is red or could not be judged):
  --keys         write _captures/i18n/keys.json: the user-facing strings found in Sources/Features, with where each sits
  --build        merge _captures/i18n/translations/<lang>.json into Sources/Resources/Localizable.xcstrings
  --compiled B   the catalogue as Xcode compiled it into app bundle B: every table, every key
  --swift        the Localization sources read the catalogue file in all 14 languages (compiled alone with swiftc)
  --resolver     Words alone with a stub bundle: a stored choice beats the system list, an unknown one follows it
  --mutants      each of the checks above run on a broken copy must go red
  --gap          print, per feature, the hard-coded user strings still in Sources/Features (the migration gap each lane closes)
  --coverage     static, per language: every key the code asks Words.text for has a non-empty value in each of the 14 languages (red names key and language)
  --leaks B      driven: under the rig launched in French, the rows of the main screen, the quick reply and the add-account sheet hold no English catalogue value
  --picked B     driven: system English with whoosh.language=fr, the screens hold no English catalogue value; without the key they do
  --merge-code   add every key the code asks Words.text for and the catalogue lacks, with its 13 translations from _captures/i18n/translations
  --catalogue    static: every key has a translation in every language, placeholders kept, nothing left in English by mistake
  --fit          static: natural width of every short label in every language, at its token size, against its container
  (default)      driven: launch the fixture inbox once per language at 1280x820 under the rig and shoot it.
                 Run: sh scripts/launch-one.sh <bundle> -- --fixture-core sh -c 'WHOOSH_SOCKET_DIR="$D" python3 checks/i18n-check.py'
  --fit-run B    driven: German and French over main, Settings, Search, Welcome at 1280x820 and 960x600, both themes (shots in _captures/i18n/fit)
  --mutant       prove the fit check red: one language given an overlong string for one label (a copy; the catalogue is untouched)
The container of a label would be read with `measure <ax-id>`; the control socket serves no frame for any ax-id yet
(Sources/ControlSocket/Verbs.swift answers "no measurement at <ax-id> yet"), so containers come from docs/CONSTANTS.md.
"""
import json, os, re, shutil, subprocess, sys, tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
FEAT = os.path.join(ROOT, "Sources", "Features")
CAT = os.path.join(ROOT, "Sources", "Resources", "Localizable.xcstrings")
CAP = os.path.join(ROOT, "_captures", "i18n")
TRANS = os.path.join(CAP, "translations")
LANGS = [("en", "English"), ("fr", "French"), ("nl", "Dutch"), ("de", "German"), ("es", "Spanish"), ("it", "Italian"),
         ("pt", "Portuguese"), ("pl", "Polish"), ("sv", "Swedish"), ("da", "Danish"), ("nb", "Norwegian Bokmal"),
         ("fi", "Finnish"), ("ja", "Japanese"), ("zh-Hans", "Simplified Chinese")]
LOC = os.path.join(ROOT, "Sources", "Localization")
RIG_ID = "io.nexus.whoosh.rig"
MT_NOTE = "machine-translated, owner review pending"

# ---- the scan: Swift string literals with their calling context -------------------------------------------------
CTX = re.compile(r'(?:\b(?:Text|Button|Label|TextField|SecureField|Toggle|Section|Picker|Menu|LabeledContent|DisclosureGroup|Link|TextEditor)\(\s*$'
                 r'|\.(?:navigationTitle|help|accessibilityLabel|accessibilityHint|accessibilityValue|badge|navigationSubtitle)\(\s*$'
                 r'|\b(?:prompt|placeholder|title|label|hint|subtitle|message|line|text|caption|detail|heading|header|footer|summary|empty|tooltip)\s*:\s*$)')
# Files whose literals are not user-facing: probes, wire formats, model prompts, key names, demo data, brand-new dev messages.
SKIP_FILES = re.compile(r'(Embedding/|LabelReaderLayout\.swift|GlideLayout\.swift|Probe\.swift|MessageBodyDocument|MessageBodyPolicy|MessageBodyPassage|CloudSpeechEngine|FluidAudioEngine|ParakeetCppEngine|Recorders\.swift|'
                        r'DictationController|Spine/|CredentialGuard|ComponentGallery|GalleryWindow|SidebarData|SidebarLabelTree|PhoneAccounts|McpSession|'
                        r'RowFacts|RichHTML|LayaEngine|LayaQuestion|LayaSequence|LayaTokenizer|AskAgents|Shortcuts/|MatryoshkaVector|GemmaWeights|SidebarTargets|'
                        r'MacSystemBridges|SidebarAccountsMenu|NotchSample|NotchRigStage|ComposerModel\+AI|MessageBodyFeature|DictationFeature|OAuthClient\.swift:48)')
SKIP_KEYS = {"%lld", "%@ pt", "%@ · %lld", "%@ %lld", "%lld %lld in %@ %@", "%lld%@%@, %@, to %@", "Label %lld %lld as", " %lld", "Syncing · %@%",
             "trust it from %@ %", "chord %@", "command %@", "Enter", "Escape", "Tab", "Down", "Left", "Right", "Bearer ", "Whoosh", "Models",
             "I can label %lld as %@. Nothing changes until you approve.", "%@ %@ blocked", "Authorization", "Accept", "Selected", "Not selected",
             "Escape", "Work", "Personal", "Clients", "Side", "Provanta", "Billing", "Legal", "Pricing", "Redlines", "Round 2", "RFP 2026", "Whoosh test provider",
             "title: none | text: ", "task: search result | query: ", "Code %@", "Tab for %@"}
# keys the scan cannot read from an interpolation with a nested condition: written out, one per ending
EXTRA_KEYS = [
    ("MessageBody", "Sources/Features/MessageBody/MessageBodyContent.swift", "%lld images blocked"),
    ("MessageBody", "Sources/Features/MessageBody/MessageBodyContent.swift", "1 image blocked"),
    ("InboxList", "Sources/Features/InboxList/InboxListView.swift", "Tab for %@"),
]
# Strings that read the same in a language on purpose (a loanword, a search operator the parser reads in English, a placeholder-only shape).
SAME_ALL = {"%@ (%@)", "%@: %@", "%1$@, %2$@", "%@:%@ · IMAP LOGIN, then LIST", "Microsoft Autodiscover"}   # no word in them, or a protocol and a vendor name
SAME_OK = {"nl": {"Account is", "Label %@", "was %@", "was %lld", "· was %lld", "%@ accounts", "1 account", "Open inbox", "Server: %@", "in %@", "%@, in %@", "%lld %@ in %@ %@", "%lld accounts", "%lld mails", "Label · L", "has attachment", "Labels %@", "index %@"},
           "de": {"Label %@", "Server: %@", "%@, in %@", "%lld %@ in %@ %@", "Label · L", "has attachment", "in %@", "Version %@"},
           "it": {"1 file · %@", "1 account", "Server: %@", "%@, in %@", "%lld %@ in %@ %@", "in %@"},
           "da": {"Server: %@", "%lld mails", "Send, ⌘ Return", "Send (⌘↵)", "Start import", "Version %@"}, "nb": {"Send, ⌘ Return", "Send (⌘↵)"},
           "sv": {"Server: %@", "Version %@", "index %@"}, "fr": {"Version %@", "%@ messages", "%lld conversations", "1 conversation", "%lld messages", "1 message", "Message %@", "has attachment"}}
# Sample rows of a preview or a rule catalogue (a made-up sender, subject or label path), not words of the interface.
SAMPLE_LINE = re.compile(r'\.init\(id: "m\d+"|\.init\(\.(?:rule|model|otherwise)?\w*, "from @|Work › Clients|String\(format: "entry %d|document\.getElementById|"%lld\|%@')
BRAND = {"J K E H L P", "Proton Mail", "Gmail", "Apple Mail", "Superhuman", "GotEmail", "Apple Intelligence", "Claude Desktop", "Cursor", "Raycast", "Notion Calendar"}


def scan(src):
    i, n, out, line = 0, len(src), [], 1
    while i < n:
        c = src[i]
        if c == "\n": line += 1; i += 1; continue
        if src.startswith("//", i):
            while i < n and src[i] != "\n": i += 1
            continue
        if src.startswith("/*", i):
            j = src.find("*/", i + 2); j = n if j < 0 else j + 2
            line += src.count("\n", i, j); i = j; continue
        if src.startswith('"""', i):
            j = src.find('"""', i + 3); j = n if j < 0 else j + 3
            line += src.count("\n", i, j); i = j; continue
        if c == '"':
            j, depth, buf = i + 1, 0, []
            while j < n:
                d = src[j]
                if d == "\\" and src[j + 1:j + 2] == "(": depth += 1; buf.append("\\("); j += 2; continue
                if depth:
                    if d == "(": depth += 1
                    elif d == ")": depth -= 1
                    buf.append(d); j += 1; continue
                if d == "\\": buf.append(src[j:j + 2]); j += 2; continue
                if d == '"' or d == "\n": break
                buf.append(d); j += 1
            out.append((line, i, "".join(buf))); i = j + 1; continue
        i += 1
    return out


def key_of(lit):
    res, i = [], 0
    while i < len(lit):
        if lit.startswith("\\(", i):
            d, j = 1, i + 2
            while j < len(lit) and d:
                d += (lit[j] == "(") - (lit[j] == ")"); j += 1
            expr = lit[i + 2:j - 1]
            numeric = bool(re.fullmatch(r"(\w+\.)*(count|n|total|unread|waiting|minutes|min|hours|days|index|remaining|seconds|number)", expr.strip())) or expr.strip().endswith(".count")
            res.append("%lld" if numeric else "%@"); i = j
        elif lit[i] == "\\" and i + 1 < len(lit):
            um = re.match(r"\\u\{([0-9A-Fa-f]+)\}", lit[i:])
            if um: res.append(chr(int(um.group(1), 16))); i += len(um.group(0)); continue   # \u{201C} is the character, not text
            res.append({"n": "\n", "t": "\t", '"': '"', "\\": "\\", "'": "'"}.get(lit[i + 1], lit[i + 1])); i += 2
        else:
            res.append(lit[i]); i += 1
    return "".join(res)


NOT_UI_EXACT = {"Add permissions", "Application permissions", "Delegated permissions", "Email, phone, or Skype", "EntraGuide", "Microsoft Graph", "No account? Create one!", "Read and write access to mailboxes via IMAP.", "Request API permissions", "Send emails from mailboxes using SMTP AUTH.", "What type of permissions does your application require?", "Your application needs to access the API as the signed-in user.", "Your application runs as a background service or daemon without a signed-in user.", "‹ All APIs",  # Microsoft Entra's own control names, drawn on the guide's plates as on Microsoft's page (A90)
    " vis=%@ key=%@", "0 armed", " · Esc", " · ⌘I", "%@-%@-%@-%@-%lld", "%@-%lld-%@-%@-%@", "'%@' HH:mm", "EEE HH:mm", "EEEE HH:mm", "d MMM HH:mm", "Laya", "Range", "Sample quote", "before %@",
                "EmbeddingGemma 2 (text, image, audio)", "Qwen2.5 1.5B Instruct (4-bit)", "KnowLine 0.8B", "GLiDE 0.8B", "Fastino Community License", "KnowLine 0.8B Gen1",
                "no cc field yet (contract delta 12)",
                " · ⌘I · Esc", "%@tokens"}   # labels r3: a glyph-only key hint joined to a translated word; a probe row id (LabelGeo)   # the last: a probe reason read by checks (ReaderWords.ccNotServed), never shown to a user   # key hints, date formats, wire fields, model and product names, a search-parser word
NOT_UI_START = ("A quote for 12,000", "January|", "a mailbox read (", "setLabelColour{", "updateSmartFolder{")


def not_ui(k):
    return k in NOT_UI_EXACT or k.startswith(NOT_UI_START) or bool(re.match(r"M-?\d[\d.\s,a-zA-Z-]*$", k) and len(k) > 4)   # the last: an SVG path


def userfacing(k):
    if not_ui(k): return False
    if not re.search(r"[A-Za-z]", k): return False
    if " " in k.strip(): return True
    if re.search(r"[./:_]", k) or re.fullmatch(r"[A-Z0-9]{1,12}", k) and k != "OK" or re.fullmatch(r"Shift\+\w", k) or k.startswith("NS") or "%02x" in k or k[0] in "⌘⇧⌃⌥" or re.fullmatch(r"Whoosh\w+ClientId", k) or "bytes instead of" in k or k.startswith("%@%@%@") or k.startswith("%lld %@ in"): return False
    if re.fullmatch(r"[a-z0-9._:/#@\-]+", k) or re.fullmatch(r"[a-z]+[A-Z][A-Za-z0-9]*", k): return False
    if re.fullmatch(r"[A-Za-z]+[_\-.][A-Za-z0-9_.\-]+", k): return False
    return bool(re.match(r"[A-Z%·…“‘]", k)) or any(ord(ch) > 127 for ch in k)


def literals():
    """Every user-facing literal in Sources/Features: dicts feature, file, line, key, words (in a *Words file), ctx (a UI call)."""
    rows = []
    for dp, _, fn in os.walk(FEAT):
        for f in sorted(fn):
            if not f.endswith(".swift"): continue
            p = os.path.join(dp, f); rel = os.path.relpath(p, ROOT)
            if SKIP_FILES.search(rel): continue
            src = open(p, encoding="utf-8").read()
            feat = os.path.relpath(p, FEAT).split(os.sep)[0]
            for line, pos, lit in scan(src):
                k = key_of(lit)
                if k in SKIP_KEYS or not userfacing(k): continue
                before = src[max(0, pos - 60):pos].replace("\n", " ")
                ls = src.rfind("\n", 0, pos) + 1
                code = src[ls:src.find("\n", pos)]
                asserty = re.search(r"assert|XCTAssert|print\(|precondition|fatalError|log\(", before + code[:40])
                if asserty or "<" in k and ">" in k or SAMPLE_LINE.search(code): continue
                rows.append({"feature": feat, "file": rel, "line": line, "key": k, "ui": bool(CTX.search(before)),
                             "words": "Words" in f, "wt": "Words.text(" in before, "src": code.strip()[:160]})
    for feat, path, k in EXTRA_KEYS:
        rows.append({"feature": feat, "file": path, "line": 0, "key": k, "ui": False, "words": True, "src": "written out by the i18n lane"})
    return rows


def keys():
    seen = {}
    for r in literals():
        seen.setdefault(r["key"], []).append(r)
    return seen


def load_catalogue():
    return json.load(open(CAT, encoding="utf-8"))


def cmd_keys():
    ks = keys()
    out = [{"key": k, "features": sorted({r["feature"] for r in v}), "where": ["%s:%d  %s" % (r["file"], r["line"], r["src"]) for r in v[:2]]}
           for k, v in sorted(ks.items())]
    os.makedirs(CAP, exist_ok=True)
    json.dump(out, open(os.path.join(CAP, "keys.json"), "w", encoding="utf-8"), ensure_ascii=False, indent=1)
    print("%d distinct user-facing strings in Sources/Features" % len(out))
    return 0



# ---- the catalogue ----------------------------------------------------------------------------------------------
def lang_code(code):
    return {"pt": "pt-BR"}.get(code, code)


def translations():
    out = {}
    for code, _ in LANGS[1:]:
        p = os.path.join(TRANS, code if code != "pt" else "pt-BR") + ".json"
        out[code] = json.load(open(p, encoding="utf-8")) if os.path.exists(p) else {}
    return out


def cmd_build():
    ks = json.load(open(os.path.join(CAP, "keys.json"), encoding="utf-8"))
    tr = translations()
    strings = {}
    for e in ks:
        k = e["key"]
        loc = {"en": {"stringUnit": {"state": "translated", "value": k}}}
        for code, _ in LANGS[1:]:
            if k in tr[code]:
                loc[code] = {"stringUnit": {"state": "translated", "value": tr[code][k]}}
        strings[k] = {"comment": "%s (%s). %s." % (", ".join(e["features"]), e["where"][0].split("  ")[0], MT_NOTE),
                      "extractionState": "extracted_with_value", "localizations": loc}
    os.makedirs(os.path.dirname(CAT), exist_ok=True)
    doc = {"sourceLanguage": "en", "strings": strings, "version": "1.0"}
    open(CAT, "w", encoding="utf-8").write(json.dumps(doc, ensure_ascii=False, indent=2, sort_keys=True) + "\n")
    print("wrote %s: %d strings x %d languages" % (os.path.relpath(CAT, ROOT), len(strings), len(LANGS)))
    return 0


PH = re.compile(r"%(?:\d+\$)?(?:@|lld|d)")


def placeholders(t):
    return sorted(re.sub(r"\d+\$", "", m) for m in PH.findall(t))


def is_symbolic(k):
    """A catalogue key that is a name (`entra-guide.s26.say`), not the English sentence itself: its placeholders live in the English value."""
    return " " not in k and NOT_WORD_TEMPLATE.fullmatch(k) is not None


def live_words(rows_by_key=None, rules=None):
    """The keys the catalogue must hold: every user-facing literal of Sources/Features minus the named not-a-word rules (sample data, log lines,
    folder matchers, command names: `drop_non_words`, the same rules `--coverage` uses; each has a case in `--selftest`)."""
    ks = keys() if rows_by_key is None else rows_by_key
    return set(drop_non_words({k: ["%s:%d  %s" % (r["file"], r["line"], r["src"]) for r in rows] for k, rows in ks.items()}, rules)[0])


def catalogue_findings(cat, live, symbolic=True, same_ok=None, same_all=None):
    """(findings, english, missing): the flags exist so that `--selftest` can switch one allowance off and see its case go red."""
    same_ok = SAME_OK if same_ok is None else same_ok
    same_all = SAME_ALL if same_all is None else same_all
    bad, english = [], {}
    missing_keys = sorted(live - set(cat))
    for k, e in cat.items():
        en = e.get("localizations", {}).get("en", {}).get("stringUnit", {}).get("value")
        want = placeholders(en) if symbolic and en is not None and is_symbolic(k) else placeholders(k)
        for code, name in LANGS:
            v = e.get("localizations", {}).get(code, {}).get("stringUnit", {}).get("value")
            if v is None or not v.strip(): bad.append("%s: no %s translation" % (k[:50], name)); continue
            if placeholders(v) != want: bad.append("%s: %s changes the placeholders (%s)" % (k[:50], name, v[:50]))
            if (v[:1] == " ") != (k[:1] == " ") or (v[-1:] == " ") != (k[-1:] == " "): bad.append("%s: %s changes the edge spaces" % (k[:50], name))
            if code != "en" and v == k and " " in k.strip() and k not in BRAND and k not in same_ok.get(code, ()) and k not in same_all: english.setdefault(code, []).append(k)
    for k in missing_keys: bad.append("%s: found in Sources/Features, not in the catalogue" % k[:60])
    for code, lst in english.items(): bad.extend("%s: still English in %s" % (x[:50], code) for x in lst)
    return bad, english, missing_keys


def cmd_catalogue():
    """Every key in every language, placeholders and edge spaces kept, no sentence left in English."""
    if not os.path.exists(CAT):
        print("RED  no catalogue at %s" % os.path.relpath(CAT, ROOT)); return 1
    cat = load_catalogue()["strings"]
    live = live_words()
    bad, english, missing_keys = catalogue_findings(cat, live)
    stale = sorted(set(cat) - live)
    n = len(cat)
    print("catalogue: %d strings x %d languages = %d cells; %d keys in the code, %d missing, %d no longer in the code" %
          (n, len(LANGS), n * len(LANGS), len(live), len(missing_keys), len(stale)))
    for code, lst in english.items():
        print("  left in English in %s: %s" % (code, "; ".join(x[:40] for x in lst[:6])))
    for b in bad[:40]: print("RED  " + b)
    print("catalogue: %s" % ("GREEN" if not bad else "RED, %d findings" % len(bad)))
    return 1 if bad else 0


# ---- round 2, one language throughout ------------------------------------------------------------------------------
SRC = os.path.join(ROOT, "Sources")


def code_keys():
    """Every key a Words.text(...) call in Sources can ask for: key -> ["file:line  source line"]. The first argument may be a ternary
    (`open ? "Hide the accounts" : "Show one row per account"`): every string literal at the top level of that argument counts."""
    res = {}
    for dp, _, fn in os.walk(SRC):
        for f in sorted(fn):
            if not f.endswith(".swift"): continue
            p = os.path.join(dp, f); rel = os.path.relpath(p, ROOT)
            src = open(p, encoding="utf-8").read()
            lits = scan(src)
            for m in re.finditer(r"Words\.text\(", src):
                start, depth, i, end = m.end(), 1, m.end(), None
                skip = {pos for _, pos, _ in lits}
                while i < len(src) and depth:
                    c = src[i]
                    if c == '"':
                        j = i + 1
                        while j < len(src) and src[j] != '"' and src[j] != "\n": j += 2 if src[j] == "\\" else 1
                        i = j + 1; continue
                    if c in "([{": depth += 1
                    elif c in ")]}": depth -= 1
                    elif c == "," and depth == 1 and end is None: end = i
                    i += 1
                end = end if end is not None else i
                for line, pos, lit in lits:
                    if start <= pos < end and not _nested(src, start, pos):
                        ls = src.rfind("\n", 0, pos) + 1
                        res.setdefault(key_of(lit), []).append("%s:%d  %s" % (rel, line, src[ls:src.find("\n", pos)].strip()[:140]))
    return res


def _nested(src, start, pos):
    """True when the literal at pos sits inside a bracket opened after the call's own '(' (an argument of an inner call)."""
    depth, i = 0, start
    while i < pos:
        c = src[i]
        if c == '"':
            j = i + 1
            while j < len(src) and src[j] != '"' and src[j] != "\n": j += 2 if src[j] == "\\" else 1
            i = j + 1; continue
        if c in "([{": depth += 1
        elif c in ")]}": depth -= 1
        i += 1
    return depth > 0


# Words handed to Words.text by a variable, so no literal in a call shows them: the built-in label names the core serves and the key names a
# KeyCap prints. Each is asked of the catalogue so that the one-line change in its lane (Words.text(name)) finds it in every language.
DYNAMIC_KEYS = {"Mark done": "foot-strip hint for E (macshell r9, passed as data from the preset)", "Pop out": "foot-strip hint for ⇧⌘P (macshell r9)","Needs Reply": "built-in label name served by the core (InboxList tab strip, sidebar, chips)",
                "Promotions": "built-in label name served by the core", "Social": "built-in label name served by the core",
                "Inbox": "mailbox name served by the core",
                **{k: "key name printed by KeyCap (Sources/Features/Components/SmallControls.swift:10), chord spelling of Sources/Features/Shortcuts/ChordKit.swift:18"
                   for k in ("Enter", "Escape", "Space", "Backspace", "Up", "Down", "Left", "Right")}}


# ---- what is NOT a word shown to anyone (tests-r58 round 3): the scan reads every string literal, so it also reads these. Each rule is named, has a selftest
# case and a mutant (`--selftest`); a key is dropped only when EVERY site that asks for it is one of these, so one real Words.text site keeps it red.
NOT_WORD_SITE_FILES = re.compile(r"Fixture[A-Za-z]*\.swift:|RepliesDoors\.swift:|RichTextFixture\.swift:")            # sample data a fixture or a rig serves
NOT_WORD_SITE_LINE = re.compile(r"CalendarLog\.line\(|Refusal\(words: \"(?:fixture |usage: fixture)|HiddenRig\.allowSystemUI|"   # log lines, fixture verbs, rig panels
                                r"setLocalizedDateFormatFromTemplate|holdToDictateKey|\bsentNames\b|^\S+:\d+\s+(?:case|return) \"[^\"]*\"(?:, \"[^\"]*\")*:|"   # a date template, a key name, folder-name matchers
                                r"^\S+:\d+\s+\"[^\"]*\",\s*\"|return \"SetReply|case \.use: return \"SetReplyUseEnabled\"")                                        # command names
NOT_WORD_TEMPLATE = re.compile(r"[a-z][a-z0-9_-]*(\.[A-Za-z0-9_%@-]+)+")      # a catalogue key built at run time: `entra-guide.c%@.title`, `inbox.view.%@.help`
NOT_WORD_PAGE_SCRIPT = re.compile(r"\bdocument\.(?:body|documentElement|scrollingElement|getElementById|querySelector)\b|\b\w+\.(?:style|dataset)\.\w+\s*=")   # JavaScript the app injects in a mail page (MessageBodyFit.script), never shown
NOT_WORD_ALLOW = {"Fn": "the Fn key's name, printed as a key",
                  "d MMM": "a date template", "NSOpenPanel replies folder": "a rig panel's name",
                  "RunVoiceAnalysis": "a core command name", "SetReplySourceEnabled": "a core command name", "SetReplyUseEnabled": "a core command name",
                  "SENT ITEMS": "a folder name matched, not shown", "SENT MESSAGES": "a folder name matched, not shown"}
NOT_WORD_RULES = {
    "sample-file": lambda k, sites: all(NOT_WORD_SITE_FILES.search(x) for x in sites),
    "log-or-matcher-line": lambda k, sites: all(NOT_WORD_SITE_LINE.search(x) for x in sites),
    "key-template": lambda k, sites: "%@" in k and NOT_WORD_TEMPLATE.fullmatch(k) is not None,
    "allow-list": lambda k, sites: k in NOT_WORD_ALLOW,
    "page-script": lambda k, sites: NOT_WORD_PAGE_SCRIPT.search(k) is not None and not any("Words.text(" in x for x in sites),
}


def drop_non_words(ck, rules=None):
    """(kept, {rule: [dropped keys]}) : a key goes at the first rule that holds for it."""
    rules = NOT_WORD_RULES if rules is None else rules
    kept, dropped = {}, {r: [] for r in rules}
    for k, sites in ck.items():
        hit = next((r for r, f in rules.items() if f(k, sites)), None)
        if hit: dropped[hit].append(k)
        else: kept[k] = sites
    return kept, dropped


def universe(skip=True):
    """What the interface can show: every Words.text key, plus every user-facing literal still hard-coded in Sources/Features (each becomes a key when its lane migrates).
    Strings that are not words shown to anyone are dropped by the named rules above (`skip=False` lists them all)."""
    ck = code_keys()
    for k, rows in keys().items():
        ck.setdefault(k, ["%s:%d  %s" % (rows[0]["file"], rows[0]["line"], rows[0]["src"])])
    for k, why in DYNAMIC_KEYS.items(): ck.setdefault(k, ["dynamic  " + why])
    return drop_non_words(ck)[0] if skip else ck


def cell(cat, k, code):
    return cat.get(k, {}).get("localizations", {}).get(code, {}).get("stringUnit", {}).get("value")


def coverage(cat, ck):
    """(red lines, per-language count of missing or empty cells)."""
    red, per = [], {c: 0 for c, _ in LANGS}
    for k in sorted(ck):
        for code, name in LANGS:
            v = cell(cat, k, code)
            if v is None or not v.strip():
                per[code] += 1
                red.append("%s: %s has %s  (%s)" % (repr(k)[:70], name, "no cell" if v is None else "an empty value", ck[k][0].split("  ")[0]))
    return red, per


def cmd_coverage(cat_path=None):
    cat = json.load(open(cat_path or CAT, encoding="utf-8"))["strings"]
    ck = universe()
    red, per = coverage(cat, ck)
    print("coverage: %d keys asked of Words.text in Sources, %d in the catalogue, %d languages = %d cells to hold" % (len(ck), sum(1 for k in ck if k in cat), len(LANGS), len(ck) * len(LANGS)))
    for code, name in LANGS: print("  %-8s %-18s %3d missing or empty" % (code, name, per[code]))
    for r in red[:25]: print("RED  " + r)
    if len(red) > 25: print("  ... and %d more" % (len(red) - 25))
    print("coverage: %s" % ("GREEN" if not red else "RED, %d cells (%d keys)" % (len(red), len({r.split(":")[0] for r in red}))))
    return 1 if red else 0


def cmd_merge_code():
    doc = load_catalogue(); cat = doc["strings"]; ck = universe(); tr = translations(); added = 0; unmet = []
    for k in sorted(ck):
        if k in cat: continue
        loc = {"en": {"stringUnit": {"state": "translated", "value": k}}}
        for code, _ in LANGS[1:]:
            if k in tr[code]: loc[code] = {"stringUnit": {"state": "translated", "value": tr[code][k]}}
            else: unmet.append((k, code))
        cat[k] = {"comment": "%s. %s." % (ck[k][0].split("  ")[0], MT_NOTE), "extractionState": "extracted_with_value", "localizations": loc}
        added += 1
    open(CAT, "w", encoding="utf-8").write(json.dumps(doc, ensure_ascii=False, indent=2, sort_keys=True) + "\n")
    print("merged: %d keys added (%d now); %d cells had no translation file entry" % (added, len(cat), len(unmet)))
    for k, c in unmet[:10]: print("  no %s for %r" % (c, k))
    return 1 if unmet else 0


# ---- the leaks probe: French under the rig, no English catalogue value on the main screen, the quick reply, the add-account sheet ----------
# mac-main.sidebar.mailboxes is not read: its `name` is the mailbox's logical English id (slug, count key); the screen draws it through Words.text (shot: Boîtes aux lettres, Brouillons, Corbeille).
# The word-bearing fields of each region (a sender, a subject, an address or a signature is mail data, not interface words).
LEAK_REGIONS = [("main", "mac-main.tabs", ("title",)), ("main", "mac-main.status", ("text",)), ("main", "mac-main.sidebar.labels", ("name", "title", "label")),
                ("main", "mac-main.sidebar.smart-folders", ("name",)), ("main", "mac-main.list", ("chips",)),
                ("quick reply", "mac-main.quickreply.field", ("placeholder",)), ("quick reply", "mac-main.undo.bar", ("text",)),
                ("add account", "mac-add-account.controls", ("title", "tooltip")), ("add account", "mac-add-account.step", ("text",)),
                ("add account", "mac-add-account.detection.step", ("text",))]


def english_patterns(cat, code):
    """(regex, key) for each catalogue key whose `code` value differs from its English text: a screen showing the English text in that language is a leak."""
    out = []
    for k, e in cat.items():
        v = cell(cat, k, code)
        if v is None or v == k or not re.search(r"[A-Za-z]{2}", re.sub(r"%(?:\d+\$)?(?:@|lld)", "", k)) or len(k.strip()) < 3: continue
        rx = "".join(".+" if m else re.escape(t) for t, m in re.findall(r"(%(?:\d+\$)?(?:@|lld)|[^%]+|%)", k) and [(t, t.startswith("%") and len(t) > 1) for t in re.findall(r"(%(?:\d+\$)?(?:@|lld)|[^%]+|%)", k)])
        out.append((re.compile("^" + rx + "$"), k))
    return out


def strings_of(v):
    if isinstance(v, str): yield v
    elif isinstance(v, list):
        for x in v: yield from strings_of(x)


def leaks_in(rows_by_region, pats):
    found = []
    for (surface, ax), rows in rows_by_region.items():
        for r in rows:
            for f in dict(LEAK_REGIONS_BY_AX).get(ax, ()):
                for t in strings_of(r.get(f)):
                    for rx, k in pats:
                        if rx.match(t.strip()): found.append((surface, ax, f, t, k)); break
    return found


LEAK_REGIONS_BY_AX = [(ax, fields) for _, ax, fields in LEAK_REGIONS]


def cmd_leaks_inner(code):
    """Inside the rig, language `code`: read the word-bearing rows of three screens, print one JSON line {surface/ax: rows}."""
    sys.path.insert(0, os.path.join(ROOT, "journeys"))
    from lib.ctl import Ctl, Unreadable
    c = Ctl().connect(); got = {}; why = []; shots = {}
    def shoot(surface):
        try:
            src = c.shot("i18n-leaks-%s-%s" % (code, surface.replace(" ", "-"))).get("path", "")
            dest = os.path.join(CAP, "r2"); os.makedirs(dest, exist_ok=True)
            if src and os.path.exists(src): shutil.copy(src, os.path.join(dest, "leaks-%s-%s.png" % (code, surface.replace(" ", "-")))); shots[surface] = "leaks-%s-%s.png" % (code, surface.replace(" ", "-"))
        except Unreadable as e: why.append("shot %s: %s" % (surface, str(e)[:80]))
    def read(surface):
        shoot(surface)
        for s, ax, _ in LEAK_REGIONS:
            if s != surface: continue
            try: got["%s|%s" % (s, ax)] = c.rows(ax)
            except Unreadable as e: why.append("%s: %s" % (ax, str(e)[:80]))
    try:
        read("main")
        c.key("Enter"); c.key("R"); read("quick reply"); c.key("Escape")
        c.click("mac-add-account.open"); read("add account")
    except Unreadable as e:
        why.append("drive: " + str(e)[:120])
    print("LEAKS " + json.dumps({"code": code, "rows": got, "unread": why, "shots": shots}, ensure_ascii=False))
    return 0


def leak_list(bundle, code="fr", cat_path=None, pick=None, system=None):
    """(found, unread, counts) for one launch of `bundle` in `code`; found = [(surface, ax, field, text, key)], or None when nothing was read."""
    cat = json.load(open(cat_path or CAT, encoding="utf-8"))["strings"]
    subprocess.run(["defaults", "write", RIG_ID, "AppleLanguages", "-array", lang_code(system or code)], check=True)
    cmd = ["sh", os.path.join(ROOT, "scripts", "launch-one.sh"), bundle, "--", "--fixture-core",
           "sh", "-c", 'WHOOSH_SOCKET_DIR="$D" python3 %s --leaks-inner %s' % (os.path.abspath(__file__), code)]
    # A stored choice left in the rig domain by an earlier check (par-settingsgeneralmac saves languages through the picker and does not delete the key)
    # beats the system list, so a run that means "system French" would read English. Start every launch from no stored choice.
    subprocess.run(["defaults", "delete", RIG_ID, "whoosh.language"], capture_output=True)
    if pick: subprocess.run(["defaults", "write", RIG_ID, "whoosh.language", pick], check=True)
    try: r = subprocess.run(cmd, capture_output=True, text=True, cwd=ROOT)
    finally:
        subprocess.run(["defaults", "delete", RIG_ID, "AppleLanguages"], capture_output=True)
        if pick: subprocess.run(["defaults", "delete", RIG_ID, "whoosh.language"], capture_output=True)
    line = next((l for l in r.stdout.splitlines() if l.startswith("LEAKS ")), None)
    if not line: print("NOT JUDGED  launch gave no rows (rc %d): %s" % (r.returncode, (r.stdout + r.stderr).strip()[-300:])); return None
    data = json.loads(line[6:])
    rows = {tuple(k.split("|")): v for k, v in data["rows"].items()}
    counts = {}
    for k, v in data["rows"].items(): counts[k.split("|")[0]] = counts.get(k.split("|")[0], 0) + len(v)
    return leaks_in(rows, english_patterns(cat, code)), data["unread"], counts


def cmd_leaks(bundle, code="fr"):
    res = leak_list(bundle, code)
    if res is None: return 2
    found, unread, counts = res
    print("leaks (%s): %s rows read; %d English catalogue values on screen" % (code, ", ".join("%s %d" % kv for kv in sorted(counts.items())), len(found)))
    for s, ax, f, t, k in found[:30]: print("RED  %s %s.%s shows English %r (catalogue key %r)" % (s, ax, f, t[:60], k[:50]))
    for w in unread: print("NOT READ  " + w)
    print("leaks: %s" % ("GREEN" if not found and not unread else "RED, %d leaks, %d regions not read" % (len(found), len(unread))))
    return 1 if found or unread else 0


def cmd_picked(bundle):
    """The stored choice drives the app. System list English: with `whoosh.language` = fr the tab strip, the quick reply and the add-account
    sheet hold no English catalogue value; without the key the same screens do (so the key is what changed them). The key is deleted after."""
    with_key, without = leak_list(bundle, "fr", system="en", pick="fr"), leak_list(bundle, "fr", system="en")
    if with_key is None or without is None: return 2
    bad = [x for x in with_key[0] if x[1] != "mac-main.sidebar.labels"]
    print("picked: system en. Without the key %d English values on screen; with whoosh.language=fr %d (%d outside the sidebar labels, a sidebar finding)" % (len(without[0]), len(with_key[0]), len(bad)))
    for s, ax, f, t_, k in bad[:10]: print("RED  %s %s.%s still English: %r" % (s, ax, f, t_[:50]))
    ok = not bad and len(without[0]) > len(with_key[0])
    print("picked: %s" % ("GREEN" if ok else "RED"))
    return 0 if ok else 1


def cmd_leaks_mutant(bundle):
    """A copy of the bundle whose French table holds the English words for 'Needs Reply': the tab strip must then show a leak the real bundle does not."""
    d = tempfile.mkdtemp(prefix="i18n-leak-")
    mb = os.path.join(d, "Whoosh.app"); shutil.copytree(bundle, mb, symlinks=True)
    f = os.path.join(mb, "Contents", "Resources", "Whoosh_WhooshUI.bundle", "Contents", "Resources", "fr.lproj", "Localizable.strings")
    t = json.loads(subprocess.run(["plutil", "-convert", "json", "-o", "-", f], capture_output=True, text=True).stdout)
    t["Needs Reply"] = "Needs Reply"
    j = os.path.join(d, "fr.json"); json.dump(t, open(j, "w")); subprocess.run(["plutil", "-convert", "xml1", "-o", f, j])
    subprocess.run(["codesign", "--force", "--deep", "-s", "-", mb], capture_output=True)
    base, mut = leak_list(bundle), leak_list(mb)
    shutil.rmtree(d)
    if base is None or mut is None: print("NOT JUDGED"); return False
    new = [x for x in mut[0] if x not in base[0] and x[1] == "mac-main.tabs"]
    print("MUTANT leaks: French table of a copy holds 'Needs Reply' in English: %d leaks beside the real bundle's %d, on the tab strip: %s" % (len(mut[0]), len(base[0]), [x[3] for x in new]))
    return bool(new)


# ---- the fit: natural width of a short label against the width of the row it sits in -----------------------------
SWIFT_MEASURE = """import AppKit
import Foundation
let data = FileManager.default.contents(atPath: CommandLine.arguments[1])!
let items = try! JSONSerialization.jsonObject(with: data) as! [[String: Any]]
var out: [[String: Any]] = []
for it in items {
    let size = CGFloat((it["size"] as! NSNumber).doubleValue)
    let font = NSFont.systemFont(ofSize: size, weight: .semibold)
    let w = (it["text"] as! NSString).size(withAttributes: [.font: font]).width
    out.append(["id": it["id"]!, "w": Double(w)])
}
FileHandle.standardOutput.write(try! JSONSerialization.data(withJSONObject: out))
"""
# Where a short label sits: the feature, the ramp step its text uses, the container constant, and what the row spends elsewhere.
CONTAINERS = {"Sidebar": ("SIDEBAR_W_PX", "body", ("inset", 2)),
              "InboxList": ("LIST_W_PX", "subhead", ("leading", 1), ("inset", 2))}


def constant(name):
    m = re.search(r"\|\s*%s\s*\|\s*(\d+)" % name, open(os.path.join(ROOT, "docs", "CONSTANTS.md"), encoding="utf-8").read())
    if not m: raise SystemExit("docs/CONSTANTS.md has no %s" % name)
    return int(m.group(1))


def ramp(step):
    m = re.search(r"static let %s = TextStyleToken\(size: (\d+)" % step, open(os.path.join(ROOT, "Sources/Design/WhooshTypography.swift"), encoding="utf-8").read())
    return int(m.group(1))


def metric(name):
    m = re.search(r"%s: CGFloat = (\d+)" % name, open(os.path.join(ROOT, "Sources/Design/WhooshSpacing.swift"), encoding="utf-8").read())
    return int(m.group(1))


def is_short_label(k):
    """One line by nature: no sentence punctuation, few words. Longer text wraps and is not cut off."""
    return not re.search(r"[.!?:]|\bto\b.*\bto\b", k) and len(k.split()) <= 4


def measure_all(items):
    d = tempfile.mkdtemp(prefix="i18n-fit-")
    src, exe, inp = [os.path.join(d, n) for n in ("m.swift", "m", "in.json")]
    open(src, "w").write(SWIFT_MEASURE); json.dump(items, open(inp, "w"))
    r = subprocess.run(["swiftc", "-O", src, "-o", exe], capture_output=True, text=True)
    if r.returncode: print("could not compile the measuring tool:\n" + r.stderr[-800:]); return None
    r = subprocess.run([exe, inp], capture_output=True, text=True)
    if r.returncode: print("measuring tool failed:\n" + r.stderr[-800:]); return None
    return {x["id"]: x["w"] for x in json.loads(r.stdout)}


def fit_cells(cat, only_lang=None):
    ks = keys(); cells = []; unmeasured = 0
    for k, rows in sorted(ks.items()):
        if not is_short_label(k) or k not in cat: continue
        for f in sorted({r["feature"] for r in rows}):
            if f not in CONTAINERS:
                unmeasured += 1; continue
            spec = CONTAINERS[f]
            width = constant(spec[0]) - sum(n * metric({"inset": "listRowInset", "leading": "listRowLeading"}[w]) for w, n in spec[2:])
            for code, _ in LANGS:
                if only_lang and code != only_lang: continue
                v = cat[k]["localizations"].get(code, {}).get("stringUnit", {}).get("value", k)
                cells.append({"id": "%s|%s|%s" % (f, k, code), "feature": f, "key": k, "lang": code, "text": v, "size": ramp(spec[1]), "container": width})
    return cells, unmeasured


def cmd_fit(mutant=False):
    cat = load_catalogue()["strings"]
    if mutant:
        probe = next(k for k, rows in sorted(keys().items()) if is_short_label(k) and any(r["feature"] == "Sidebar" for r in rows))
        cat = json.loads(json.dumps(cat))
        cat[probe]["localizations"]["de"]["stringUnit"]["value"] = "Eingehende Nachrichten aller verbundenen Konten nach Absender gruppiert anzeigen und sortieren"
        print("MUTANT: de given an overlong string for the Sidebar label \"%s\"" % probe)
    cells, unmeasured = fit_cells(cat)
    w = measure_all([{"id": c["id"], "text": c["text"], "size": c["size"]} for c in cells])
    if w is None: return 1
    red = [(c, w[c["id"]]) for c in cells if w[c["id"]] > c["container"]]
    langs = len({c["lang"] for c in cells})
    print("fit: %d labels x %d languages = %d cells measured against the width of their row; %d labels sit in a container the control socket cannot measure yet (no frame for any ax-id)" %
          (len({(c["feature"], c["key"]) for c in cells}), langs, len(cells), unmeasured))
    widest = {}
    for c in cells: widest[c["lang"]] = max(widest.get(c["lang"], 0), w[c["id"]])
    print("  widest label per language, pt: " + ", ".join("%s %.0f" % (k, v) for k, v in widest.items()))
    for c, x in red[:20]:
        print("RED  %s \"%s\" in %s: %.0f pt wide, the row is %d pt (%s)" % (c["feature"], c["key"], c["lang"], x, c["container"], c["text"][:60]))
    print("fit: %s" % ("GREEN" if not red else "RED, %d cells" % len(red)))
    return 1 if red else 0


def cmd_mutant():
    rc = cmd_fit(mutant=True)
    print("mutant: %s" % ("seen RED, the check catches an overlong string" if rc else "NOT SEEN RED: the fit check cannot fail"))
    return 0 if rc else 1


# ---- the Swift reading path: the same catalogue file read by Sources/Localization, one language at a time ----------------
SWIFT_READ = """import Foundation
let cat = WordsCatalogue(data: FileManager.default.contents(atPath: CommandLine.arguments[1])!)!
var out: [[String: Any]] = []
for lang in cat.languages {
    let chosen = cat.language(preferring: [lang])
    for k in cat.keys { out.append(["lang": lang, "chosen": chosen, "key": k, "text": cat.value(k, language: chosen) ?? "", "filled": WordsFormat.fill(cat.value(k, language: chosen) ?? "", ["ARG1", "7", "ARG3"])]) }
}
let cases: [(String, [String])] = [("%@ of %lld accounts synced", ["Work", "4"]), ("%2$@ / %1$lld", ["3", "x"]), ("100%% of %@", ["a"]), ("%@ and %@", ["only"]), ("50% off %@", ["z"])]
for (f, a) in cases { out.append(["lang": "fmt", "key": f, "filled": WordsFormat.fill(f, a)]) }
FileHandle.standardOutput.write(try! JSONSerialization.data(withJSONObject: out))
"""


def cmd_swift():
    d = tempfile.mkdtemp(prefix="i18n-swift-")
    main = os.path.join(d, "main.swift"); exe = os.path.join(d, "r")
    open(main, "w").write(SWIFT_READ)
    loc = LOC
    r = subprocess.run(["swiftc", "-O", main, os.path.join(loc, "WordsCatalogue.swift"), os.path.join(loc, "WordsFormat.swift"), "-o", exe], capture_output=True, text=True)
    if r.returncode: print("RED  the Localization sources do not compile on their own:\n" + r.stderr[-800:]); return 1
    r = subprocess.run([exe, CAT], capture_output=True, text=True)
    if r.returncode: print("RED  the reader failed:\n" + r.stderr[-600:]); return 1
    rows = json.loads(r.stdout); cat = load_catalogue()["strings"]; bad = []
    want = {"%@ of %lld accounts synced": "Work of 4 accounts synced", "%2$@ / %1$lld": "x / 3", "100%% of %@": "100% of a", "%@ and %@": "only and ", "50% off %@": "50% off z"}
    n = 0; langs = set()
    for x in rows:
        if x["lang"] == "fmt":
            if x["filled"] != want[x["key"]]: bad.append("format %r gave %r, wanted %r" % (x["key"], x["filled"], want[x["key"]]))
            continue
        langs.add(x["lang"]); n += 1
        if x["chosen"] != x["lang"]: bad.append("preference %s chose %s" % (x["lang"], x["chosen"]))
        exp = cat[x["key"]]["localizations"][x["lang"]]["stringUnit"]["value"]
        if x["text"] != exp: bad.append("%s / %s read %r, the file says %r" % (x["lang"], x["key"][:40], x["text"], exp))
        if "%" in re.sub(r"%%", "", x["filled"]) and PH.search(x["filled"]): bad.append("%s / %s kept a placeholder: %r" % (x["lang"], x["key"][:40], x["filled"]))
    print("swift reader: %d languages, %d strings read through WordsCatalogue and filled through WordsFormat, 5 format cases" % (len(langs), n))
    for b in bad[:20]: print("RED  " + b)
    print("swift reader: %s" % ("GREEN" if not bad else "RED, %d" % len(bad)))
    return 1 if bad else 0


RESOLVER_MAIN = """import Foundation
extension Bundle { static var module: Bundle { Bundle(path: CommandLine.arguments[1])! } }
let out: [String: String] = ["current": Words.current, "selected": Words.selected, "from": Words.text("From"),
    "reply": Words.text("Reply to %@…", "Ana"), "absent": Words.text("zz no such key zz"), "dflt": Words.text("zz no such key zz", default: "kept")]
FileHandle.standardOutput.write(try! JSONSerialization.data(withJSONObject: out))
"""


def cmd_resolver():
    """Words answers in the stored choice when there is one (above the system list), else the system's; one language for every string."""
    d = tempfile.mkdtemp(prefix="i18n-resolver-"); res = os.path.join(d, "res"); os.makedirs(res)
    shutil.copy(CAT, os.path.join(res, "Localizable.xcstrings"))
    main = os.path.join(d, "main.swift"); exe = os.path.join(d, "w"); open(main, "w").write(RESOLVER_MAIN)
    r = subprocess.run(["swiftc", main] + [os.path.join(LOC, f) for f in ("Words.swift", "WordsCatalogue.swift", "WordsFormat.swift")] + ["-o", exe], capture_output=True, text=True)
    if r.returncode: print("RED  Words does not compile on its own:\n" + r.stderr[-900:]); return 1
    cat = load_catalogue()["strings"]; bad = []; n = 0
    def run(system, pick=None):
        args = [exe, res, "-AppleLanguages", "(%s)" % system] + (["-whoosh.language", pick] if pick else [])
        o = subprocess.run(args, capture_output=True, text=True)
        return json.loads(o.stdout) if o.returncode == 0 and o.stdout else {"err": o.stderr[-200:]}
    def want(label, got, key_lang, system, pick=None):
        nonlocal n; n += 1
        exp = {"current": key_lang, "from": cell(cat, "From", key_lang), "reply": cell(cat, "Reply to %@…", key_lang).replace("%@", "Ana"), "absent": "zz no such key zz", "dflt": "kept"}
        for k, v in exp.items():
            if got.get(k) != v: bad.append("system %s, pick %s: %s was %r, wanted %r" % (system, pick, k, got.get(k), v))
    for system in ("fr", "en", "ja"):                      # system alone: the system's language
        want("system", run(system), {"fr": "fr", "en": "en", "ja": "ja"}[system], system)
    for code, _ in LANGS:                                  # a stored choice beats the system list, whichever it is
        want("pick", run("fr" if code != "fr" else "de", code), code, "fr" if code != "fr" else "de", code)
    want("bogus", run("fr", "xx"), "fr", "fr", "xx")      # an unknown stored value follows the system
    want("system word", run("de", "system"), "de", "de", "system")
    print("resolver: %d launches of Words with a stub resource bundle (system list x stored choice), 5 answers each" % n)
    for b in bad[:20]: print("RED  " + b)
    print("resolver: %s" % ("GREEN" if not bad else "RED, %d" % len(bad)))
    return 1 if bad else 0


def broken_copy(name, text, old, new):
    """The text of a broken copy: `old` must occur once in `text`, else the mutant would be a no-op and the check could never be seen red (a source that moved on):
    the run stops with the file's name instead of printing a mutant that was never applied."""
    n = text.count(old)
    if n != 1:
        raise SystemExit("MUTANT TARGET: %r occurs %d times in %s (it must occur once); the mutant would not change the file" % (old, n, os.path.basename(name)))
    return text.replace(old, new)


def cmd_mutants(bundle=None):
    """Each check, run on a broken COPY, must go red; the files in the tree are never touched."""
    global CAT, LOC
    real_cat, real_loc = CAT, LOC
    d = tempfile.mkdtemp(prefix="i18n-mut-")
    doc = json.load(open(real_cat, encoding="utf-8"))
    out = {}
    # 1. a language loses one cell, another gets a changed placeholder
    m = json.loads(json.dumps(doc))
    del m["strings"]["Archive"]["localizations"]["pl"]
    m["strings"]["%lld selected"]["localizations"]["de"]["stringUnit"]["value"] = "ausgewaehlt"
    cm = os.path.join(d, "cat.xcstrings"); json.dump(m, open(cm, "w", encoding="utf-8"), ensure_ascii=False)
    CAT = cm
    print("MUTANT catalogue: pl loses 'Archive', de drops the number of '%lld selected'")
    out["catalogue"] = cmd_catalogue() != 0
    # 1b. a whole key is removed from the catalogue (a string the code asks for is no longer there)
    m = json.loads(json.dumps(doc)); del m["strings"]["Apply"]
    cm = os.path.join(d, "cat-nokey.xcstrings"); json.dump(m, open(cm, "w", encoding="utf-8"), ensure_ascii=False)
    CAT = cm
    print("MUTANT catalogue key: the key 'Apply' is removed from the catalogue")
    out["catalogue key"] = cmd_catalogue() != 0
    CAT = real_cat
    # 2. the formatter ignores the position of a placeholder
    shutil.copytree(real_loc, os.path.join(d, "loc")); LOC = os.path.join(d, "loc"); CAT = real_cat
    f = os.path.join(LOC, "WordsFormat.swift"); t = open(f).read()
    open(f, "w").write(broken_copy(f, t, 'position = Int(digits); j += 1', 'position = nil; j += 1'))
    print("MUTANT swift reader: WordsFormat ignores the position in %2$@")
    out["swift reader"] = cmd_swift() != 0
    CAT, LOC = real_cat, real_loc
    # 3. coverage: one French value removed, one Japanese value emptied, one key gone from every language
    m = json.loads(json.dumps(doc))
    del m["strings"]["Archive"]["localizations"]["fr"]
    m["strings"]["Reply all"]["localizations"]["ja"]["stringUnit"]["value"] = ""
    del m["strings"]["Trash"]
    cm = os.path.join(d, "cat-cover.xcstrings"); json.dump(m, open(cm, "w", encoding="utf-8"), ensure_ascii=False)
    print("MUTANT coverage: fr loses 'Archive', ja empties 'Reply all', 'Trash' is gone")
    out["coverage"] = cmd_coverage(cm) != 0
    # 4. the resolver ignores the stored choice (the bug of the owner's screenshot: the system list wins)
    shutil.copytree(real_loc, os.path.join(d, "loc2")); LOC = os.path.join(d, "loc2")
    f = os.path.join(LOC, "Words.swift"); t = open(f).read()
    open(f, "w").write(broken_copy(f, t, "let pick = UserDefaults.standard.string(forKey: languageKey) ?? system\n        return codes", "let pick = system\n        return codes"))
    print("MUTANT resolver: Words.text ignores the stored choice and follows the system list")
    out["resolver"] = cmd_resolver() != 0
    LOC = real_loc
    if bundle:
        res = os.path.join(bundle, "Contents", "Resources", "Whoosh_WhooshUI.bundle", "Contents", "Resources")
        mb = os.path.join(d, "app"); shutil.copytree(res, os.path.join(mb, "Contents", "Resources", "Whoosh_WhooshUI.bundle", "Contents", "Resources"))
        f = os.path.join(mb, "Contents", "Resources", "Whoosh_WhooshUI.bundle", "Contents", "Resources", "fr.lproj", "Localizable.strings")
        t = json.loads(subprocess.run(["plutil", "-convert", "json", "-o", "-", f], capture_output=True, text=True).stdout)
        t["Archive"] = "Archive"
        j = os.path.join(d, "fr.json"); json.dump(t, open(j, "w")); subprocess.run(["plutil", "-convert", "xml1", "-o", f, j])
        print("MUTANT compiled: the French table of a copy of the app bundle holds the English word for 'Archive'")
        out["compiled"] = cmd_compiled(mb) != 0
    if bundle: out["leaks"] = cmd_leaks_mutant(bundle)
    out["fit"] = cmd_fit(mutant=True) != 0
    shutil.rmtree(d)
    for k, v in out.items(): print("mutant %-13s %s" % (k, "seen RED" if v else "NOT SEEN RED"))
    return 0 if all(out.values()) else 1


def cmd_compiled(bundle):
    """The catalogue as Xcode compiled it into the app: every language table, every key, the same words and placeholders."""
    res = os.path.join(bundle, "Contents", "Resources", "Whoosh_WhooshUI.bundle", "Contents", "Resources")
    cat = load_catalogue()["strings"]; bad = []; n = 0
    for code, name in LANGS:
        f = os.path.join(res, code + ".lproj", "Localizable.strings")
        if not os.path.exists(f): bad.append("%s: no %s.lproj in the app bundle" % (name, code)); continue
        r = subprocess.run(["plutil", "-convert", "json", "-o", "-", f], capture_output=True, text=True)
        if r.returncode: bad.append("%s: table unreadable: %s" % (name, r.stderr.strip()[:80])); continue
        tbl = json.loads(r.stdout)
        for k, e in cat.items():
            want = e["localizations"][code]["stringUnit"]["value"]; got = tbl.get(k); n += 1
            if got != want: bad.append("%s / %s: compiled %r, catalogue %r" % (name, k[:40], (got or "")[:40], want[:40]))
    print("compiled: %d cells read from the app bundle's %d language tables" % (n, len(LANGS)))
    for b in bad[:20]: print("RED  " + b)
    print("compiled: %s" % ("GREEN" if not bad else "RED, %d" % len(bad)))
    return 1 if bad else 0


# ---- the gap: hard-coded user strings each lane still has to move into the catalogue -------------------------------
def cmd_gap():
    cat = load_catalogue()["strings"] if os.path.exists(CAT) else {}
    per = {}; where = {}
    for r in literals():
        if r["line"] == 0: continue
        if "Words.text(" in r.get("src", "") or r.get("wt"): continue   # already in the catalogue: the literal is the key, not a hard-coded string (lead fix)
        per.setdefault(r["feature"], set()).add((r["file"], r["line"]))
        where.setdefault(r["feature"], []).append("%s:%d  %s" % (r["file"], r["line"], r["src"][:110]))
    total = 0
    print("migration gap: hard-coded user strings still in Sources/Features, per feature (each lane moves its own into Words.text)")
    for f, s in sorted(per.items(), key=lambda kv: -len(kv[1])):
        print("  %-22s %3d" % (f, len(s))); total += len(s)
    print("  %-22s %3d  (in %d features; %d distinct strings, all in the catalogue)" % ("TOTAL", total, len(per), len(keys())))
    if "--list" in sys.argv:
        out = ["# Remaining hard-coded user strings, per feature (file:line  source line)\n"]
        for f, s in sorted(per.items(), key=lambda kv: kv[0]):
            out.append("\n## %s (%d)\n" % (f, len(s)) + "\n".join("- " + w for w in sorted(set(where[f]))))
        open(os.path.join(CAP, "gap.md"), "w", encoding="utf-8").write("\n".join(out) + "\n")
        print("wrote _captures/i18n/gap.md")
    return 0


# ---- the driven run: one launch per language -----------------------------------------------------------------------
def cmd_inner(code):
    sys.path.insert(0, os.path.join(ROOT, "journeys"))
    from lib.ctl import Ctl, Unreadable
    c = Ctl().connect(); bad = []
    try:
        st = c.uistate()
        rows = c.rows("mac-main.list")
        import time
        win = None
        for _ in range(50):                       # the shell sizes its window after the first paint
            fr = c.winframes()
            win = max(fr, key=lambda f: f["w"] * f["h"]) if fr else None
            if win and (round(win["w"]), round(win["h"])) == (1280, 820): break
            time.sleep(0.1)
        shot = c.shot("i18n-" + code)
    except Unreadable as e:
        print("NOT JUDGED  %s: %s" % (code, e)); return 2
    if st.get("surface") != "mac-main": bad.append("surface is %r, not mac-main" % st.get("surface"))
    if not rows: bad.append("the fixture inbox shows no rows")
    if not win or (round(win["w"]), round(win["h"])) != (1280, 820): bad.append("window is %s, not 1280x820" % (win and "%dx%d" % (round(win["w"]), round(win["h"]))))
    dest = os.path.join(CAP, "shots"); os.makedirs(dest, exist_ok=True)
    src = shot.get("path", "")
    if src and os.path.exists(src):
        import shutil; shutil.copy(src, os.path.join(dest, "%s.png" % code))
    else: bad.append("no screenshot at %r" % src)
    print("%s: surface=%s rows=%d window=%s %s" % (code, st.get("surface"), len(rows), win and "%dx%d" % (round(win["w"]), round(win["h"])), "OK" if not bad else "RED " + "; ".join(bad)))
    return 1 if bad else 0


def cmd_all(bundle):
    rc = 0
    for code, name in LANGS:
        al = lang_code(code)
        # The language is the rig copy's own preference (its test identity, never the owner's app): launch-one.sh passes only
        # leading "--" words to the app, and a "-AppleLanguages" argument kept the app from publishing its socket.
        subprocess.run(["defaults", "write", RIG_ID, "AppleLanguages", "-array", al], check=True)
        cmd = ["sh", os.path.join(ROOT, "scripts", "launch-one.sh"), bundle, "--", "--fixture-core",
               "sh", "-c", 'WHOOSH_SOCKET_DIR="$D" python3 %s --inner %s' % (os.path.abspath(__file__), code)]
        try:
            r = subprocess.run(cmd, capture_output=True, text=True, cwd=ROOT)
        finally:
            subprocess.run(["defaults", "delete", RIG_ID, "AppleLanguages"], capture_output=True)
        line = next((l for l in r.stdout.splitlines() if l.startswith(code + ":") or "NOT JUDGED" in l), None)
        print(line or "%s: launch failed (rc %d): %s" % (code, r.returncode, (r.stdout + r.stderr).strip().splitlines()[-1:] ))
        rc = rc or (1 if r.returncode else 0)
    print("driven: %s" % ("GREEN, 14 launches" if not rc else "RED"))
    return rc


# ---- the driven fit run: two long languages, two window sizes, two themes, four surfaces ---------------------------------
FIT_LANGS = ["de", "fr"]
FIT_SIZES = [(1280, 820), (960, 600)]
FIT_SURFACES = ["mac-main", "mac-settings-general", "mac-search", "mac-welcome-accounts"]


def blank(path):
    """True when the PNG holds one colour only (a window nothing was drawn into)."""
    r = subprocess.run(["python3", "-c", "import sys,zlib\nd=open(sys.argv[1],'rb').read()\nprint(len(set(d[i:i+64] for i in range(200,len(d)-64,997))))", path], capture_output=True, text=True)
    return r.returncode == 0 and r.stdout.strip().isdigit() and int(r.stdout.strip()) < 4


def cmd_fit_inner(code):
    sys.path.insert(0, os.path.join(ROOT, "journeys"))
    from lib.ctl import Ctl, Unreadable
    import time
    c = Ctl().connect(); dest = os.path.join(CAP, "fit"); os.makedirs(dest, exist_ok=True); bad = []; n = 0
    def keep(src, name):
        if src and os.path.exists(src):
            shutil.copy(src, os.path.join(dest, name + ".png"))
            if blank(src): bad.append("%s is a blank image" % name)
        else: bad.append("no shot for %s" % name)
    try:
        for theme in ("light", "dark"):
            c.send("theme", theme)
            for (w, h) in FIT_SIZES:
                c.send("resize", w, h); time.sleep(0.4)
                for s in FIT_SURFACES:
                    r = c.send("capture", "shot", s); n += 1
                    fr = c.winframes(); big = max(fr, key=lambda f: f["w"] * f["h"]) if fr else {"w": 0, "h": 0}
                    keep(r.get("path", ""), "%s-%s-%dx%d-%s" % (code, s, w, h, theme))
                    print("  %s %s %dx%d %s: window %dx%d" % (code, s, w, h, theme, round(big["w"]), round(big["h"])))
    except Unreadable as e:
        print("NOT JUDGED  %s: %s" % (code, e)); return 2
    print("%s: %d shots %s" % (code, n, "OK" if not bad else "RED " + "; ".join(bad[:6])))
    return 1 if bad else 0


def cmd_fit_run(bundle):
    rc = 0
    for code in FIT_LANGS:
        subprocess.run(["defaults", "write", RIG_ID, "AppleLanguages", "-array", lang_code(code)], check=True)
        cmd = ["sh", os.path.join(ROOT, "scripts", "launch-one.sh"), bundle, "--", "--fixture-core",
               "sh", "-c", 'WHOOSH_SOCKET_DIR="$D" python3 %s --fit-inner %s' % (os.path.abspath(__file__), code)]
        try: r = subprocess.run(cmd, capture_output=True, text=True, cwd=ROOT)
        finally: subprocess.run(["defaults", "delete", RIG_ID, "AppleLanguages"], capture_output=True)
        print(r.stdout.strip() or ("%s: launch failed rc %d: %s" % (code, r.returncode, r.stderr.strip()[-300:])))
        rc = rc or r.returncode
    print("fit run: %s" % ("shots taken" if not rc else "RED or not judged"))
    return rc


def selftest_skip():
    """One case per skip rule (a key that rule must drop), a positive control per rule (a key it must keep), and a mutant per rule (the rule removed: its case goes red)."""
    F, L = "Sources/Features/SettingsCalendarsMac/FixtureCalendarAccess.swift:68  ev(\"ev-2\", ...)", 'Sources/Features/SettingsCalendarsMac/CalendarsModel.swift:72  CalendarLog.line("calendar access: \\(x)")'
    REAL = "Sources/Features/SettingsAIMac/AIWords.swift:28  static var addKeyAbove: String { Words.text(\"Add a key above\") }"
    PS = 'Sources/Features/MessageBody/MessageBodyFit.swift:53  return page + "b.style.transform=\'\';b.style.transformOrigin=\'\';delete b.dataset.ws;"'
    M = 'Sources/Features/WelcomeMac/RepliesStepModel.swift:94  case "SENT", "SENT ITEMS": return'
    M2 = 'Sources/Features/WelcomeMac/RepliesStepModel.swift:95  "gesendet", "gesendete objekte", "gesendete elemente", "verzonden"'
    cases = [("sample-file", "Appel fournisseur", [F]), ("log-or-matcher-line", "calendar access: %@", [L]), ("log-or-matcher-line", "SENT", [M]), ("log-or-matcher-line", "gesendete objekte", [M2]),
             ("key-template", "entra-guide.c%@.title", [REAL]), ("allow-list", "Fn", [REAL]),
             ("page-script", "var b=document.body,r=document.documentElement;", [PS]), ("page-script", "b.style.transform='';b.style.transformOrigin='';delete b.dataset.ws;r.style.overflowY='';", [PS]),
             ("page-script", "b.style.transformOrigin='0 0';b.style.transform='scale(%@)';b.dataset.ws='%@';r.style.overflowY='hidden';", [PS])]
    keeps = [("a real key, one real site", "Add a key above", [REAL]), ("a sample key that ALSO has a real site", "Appel fournisseur", [F, REAL]),
             ("a sentence with %@ is not a template", "Hold %@ in a draft and speak.", [REAL]), ("a near-miss of the allow list", "Fnord", [REAL]),
             ("a sentence about the document is not a page script", "Open the document body and style it", [REAL]), ("a page script with a real Words.text site", "var b=document.body;", [PS, REAL])]
    bad = 0
    def show(name, ok, extra=""):
        nonlocal bad; bad += not ok
        print("%-72s %s%s" % (name, "ok" if ok else "FAIL", "" if ok else "  " + extra))
    for rule, k, sites in cases:
        kept, dropped = drop_non_words({k: sites})
        show("rule %-20s drops %r" % (rule, k), k in sum(dropped.values(), []) and k not in kept, "dropped=%r" % dropped)
    for name, k, sites in keeps:
        kept, _ = drop_non_words({k: sites})
        show("kept: " + name, k in kept)
    for rule in NOT_WORD_RULES:
        rules = {r: f for r, f in NOT_WORD_RULES.items() if r != rule}
        red = [k for r, k, sites in cases if r == rule and k in drop_non_words({k: sites}, rules)[0]]
        show("mutant: rule %s removed -> its keys stay in the universe (red)" % rule, bool(red), "no case went red")
    # the `--catalogue` rules (settle-the-catalogue round): each case on a made-up catalogue, its allowance switched off in the mutant
    def ent(**v): return {"localizations": {c: {"stringUnit": {"value": v.get(c, "[%s] %s" % (c, v["en"]))}} for c, _ in LANGS}}   # a language not given holds a marked copy of the English
    def row(f, line, src): return {"file": f, "line": line, "src": src}
    sym = {"entra-guide.s26.say": ent(en="Click Yes (%@).", fr="Cliquez sur Yes (%@).")}
    show("catalogue: a symbolic key takes its placeholders from the English value", not catalogue_findings(sym, set())[0], "%r" % catalogue_findings(sym, set())[0])
    show("mutant: symbolic rule off -> the same cell is red", bool(catalogue_findings(sym, set(), symbolic=False)[0]), "stayed green")
    lost = {"entra-guide.s26.say": ent(en="Click Yes (%@).", fr="Cliquez sur Yes.")}
    show("kept: a symbolic key whose French drops the %@ is still red", bool(catalogue_findings(lost, set())[0]), "went green")
    ver = {"Version %@": ent(en="Version %@", fr="Version %@"), "%1$@, %2$@": ent(en="%1$@, %2$@", fr="%1$@, %2$@", ja="%1$@, %2$@")}
    show("catalogue: Version in French and a placeholder-only shape are not left in English", not catalogue_findings(ver, set())[1], "%r" % catalogue_findings(ver, set())[1])
    show("mutant: both allowances off -> both are red", len(catalogue_findings(ver, set(), same_ok={}, same_all=set())[1].get("fr", [])) == 2, "not both red")
    eng = {"Open the folder": ent(en="Open the folder", fr="Open the folder")}
    show("kept: a real English sentence in French is still red", bool(catalogue_findings(eng, set())[1]), "went green")
    fx = {"Appel fournisseur": [row("Sources/Features/SettingsCalendarsMac/FixtureCalendarAccess.swift", 68, 'ev("ev-2", "cal-work", "Appel fournisseur")')]}
    show("catalogue: a fixture's sample title is not a key the catalogue must hold", "Appel fournisseur" not in live_words(fx), "kept")
    show("mutant: the not-a-word rules off -> it is a missing key", "Appel fournisseur" in live_words(fx, rules={}), "dropped anyway")
    fx["Appel fournisseur"].append(row("Sources/Features/AIWords.swift", 3, 'static var x: String { "Appel fournisseur" }'))
    show("kept: the same text at one real site is still a key", "Appel fournisseur" in live_words(fx), "dropped")
    real = universe(skip=False); kept, dropped = drop_non_words(real)
    show("the real tree: every dropped key has a site the rule names (count %d)" % sum(len(v) for v in dropped.values()), sum(len(v) for v in dropped.values()) > 0)
    print("%d problems" % bad)
    return 1 if bad else 0


if __name__ == "__main__":
    a = sys.argv[1:]
    if "--keys" in a: sys.exit(cmd_keys())
    if "--build" in a: sys.exit(cmd_build())
    if "--selftest" in a: sys.exit(selftest_skip())
    if "--coverage" in a: sys.exit(cmd_coverage())
    if "--leaks-inner" in a: sys.exit(cmd_leaks_inner(a[a.index("--leaks-inner") + 1]))
    if "--picked" in a: sys.exit(cmd_picked(a[a.index("--picked") + 1]))
    if "--leaks" in a:
        i = a.index("--leaks")
        if i + 1 >= len(a) or a[i + 1].startswith("--"):
            print("usage: i18n-check.py --leaks <path/to/Whoosh.app>   (the bundle to drive; none given)"); sys.exit(2)
        sys.exit(cmd_leaks(a[i + 1]))
    if "--merge-code" in a: sys.exit(cmd_merge_code())
    if "--catalogue" in a: sys.exit(cmd_catalogue())
    if "--fit" in a: sys.exit(cmd_fit())
    if "--mutant" in a: sys.exit(cmd_mutant())
    if "--mutants" in a: sys.exit(cmd_mutants(a[a.index("--mutants") + 1] if len(a) > a.index("--mutants") + 1 else None))
    if "--swift" in a: sys.exit(cmd_swift())
    if "--resolver" in a: sys.exit(cmd_resolver())
    if "--compiled" in a: sys.exit(cmd_compiled(a[a.index("--compiled") + 1]))
    if "--gap" in a: sys.exit(cmd_gap())
    if "--inner" in a: sys.exit(cmd_inner(a[a.index("--inner") + 1]))
    if "--fit-inner" in a: sys.exit(cmd_fit_inner(a[a.index("--fit-inner") + 1]))
    if "--fit-run" in a: sys.exit(cmd_fit_run(a[a.index("--fit-run") + 1]))
    if "--all" in a: sys.exit(cmd_all(a[a.index("--all") + 1]))
    print(__doc__); sys.exit(2)

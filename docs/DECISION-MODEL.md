# How Whoosh decides (version 2: organised by what it decides; rules and the model in ONE list)

Owner ruling (2026-11-07 18:30, verbatim) on version 1, which kept "Rules" and "Decision model" as two settings pages: "This is bad. You're separating too much the fixed rule and the AI. This is not it. You should have a settings specific to, for example, email classification. And in there having BOTH the fixed rules AND the potential extra decision model and deciding on priority. This is an example of course but you need to propeerly merge the AI and the rules. AI on it's own is pointless, what matter is what it does". Version 1 is kept below as "## Appendix: version 1 (rejected)" for its grounded facts; everything above the appendix supersedes it.

## 1. Settings are organised by WHAT IS DECIDED, never by how
Settings › Decisions holds one page per thing Whoosh decides about a message. Each page is ONE ordered list of entries, top to bottom, first match wins, and the model is just one kind of entry in that list, placed where you want it:
| Decision page | What it settles | Shipped list (you reorder, edit, add) |
|---|---|---|
| **Which tab** (email classification) | the split tab a message sits in | 1 · if from a VIP → Primary · 2 · if has a list header → Updates · 3 · **ask the model** "Which tab?" (trust it from 80 %) · 4 · otherwise Primary |
| **Needs my reply** | the Needs Reply flag and the waiting count | 1 · if from me → no · 2 · if has a list header → no · 3 · **ask the model** "Does this email ask me, personally, to answer or to do something?" (trust it from 75 %) · 4 · otherwise no |
| **Sign-in codes** | the notch's code pill and "Fill into the field in front" | 1 · if subject contains "code", "verification", "sign in" → yes · 2 · **ask the model** "Does this email carry a one-time code, a login link or a verification number?" (trust it from 90 %) · 3 · otherwise no |
| **VIPs** | the VIPs tab and the "waiting on you" count | 1 · your VIP list (addresses and domains) · 2 · otherwise no (no model entry shipped; you may add one) |
| **Notify me** | which arrivals ring the Mac and the iPhone | 1 · if VIP → notify · 2 · if Needs my reply is yes → notify · 3 · if has a list header → silent · 4 · otherwise silent |
| **Labels** | labels added on arrival | your rules only (no model entry shipped) |
Each entry is a sentence. A RULE entry = conditions (from, to or cc, subject contains, body contains, has a list header, account, has an attachment, size, older than, from a VIP, from me) → the page's outcome. A MODEL entry = "ask the model: <the question in words> · trust it from <level>"; below the trust level the list continues to the next entry. A DEFAULT closes every list. "Create a rule from this message" opens the right page with a rule entry prefilled; "Why?" on any row opens the entry that decided it ("Which tab · entry 3 · the model said Updates (0.91)").

## 2. The model is one engine for every "ask the model" entry
Settings › Decisions › **Engine** (one short section at the top of the Decisions page, not a page of its own): Local (Laya, downloaded, runs on this Mac, nothing leaves) · Jev (TypeSafe, your API key in the Keychain, the privacy sentence with a switch per field) · Off (every "ask the model" entry is skipped and its list falls through). The engine answers typed questions the way Laya and Jev both take them (a message plus questions of three kinds: pick one tab, yes or no, a score); a model entry's question text and its answer options are exactly what is sent, so editing the sentence edits the prompt.

## 3. Teaching lives on each decision page
When you move a message to another tab or toggle Needs reply, the page records it: "7 corrections since Monday". The page then offers the two fixes in order: "Make a rule from these" (when the corrections share a sender, domain or list) or "Refit the model" (Laya: after 35 examples, on this Mac; Jev: no refit, so only rules). A **Preview** button on each page runs the whole list on the last 20 messages and shows, per message, which entry decided and whether it differs from today.

SIGN-OFF (v2): Assem Chammah, 2026-11-07 19:16, owner: "Yes, build it this way" (card on the redrawn screens docs/design/boards/settings-decision/v2).

## 4. What this replaces
Two pages (Rules; Decision model) become one Decisions section organised by outcome; the provider settings shrink to the Engine block; the rule grammar, the question wording, the floors, the teaching loop and the providers keep the grounded facts of version 1 (appendix).

## Appendix: version 1 (rejected 2026-11-07 18:30), kept for its sources
# How Whoosh decides where a message goes (rules first, then the decision model)

Owner asks, verbatim: "have ability to connect or configure a local decision model like laya or Jev with the API key and be able to properly configure it" and "you need to look much better into decision models to know how they will ACTUALLY need to be 'prompted' or configured and be able to add fixed rules for categorization (e.g., email recipients, domains, content of subject or body, etc.). This needs to go one step further."
Grounding: research/decision-config.md (Laya card and README, TypeSafe Jev docs, the org's own Jev client and question builders, Gmail help, RFC 5228 Sieve), research/decision-model.md. Every field below names its source there; a field with no source is marked proposed.

## 1. Three layers, in this order, and every message says why
1. **Your rules** (fixed, top to bottom, first match wins; proposed order rule, as Apple Mail and Sieve do).
2. **The decision model** (Laya on this Mac, or Jev with your key): asked only what the rules did not settle.
3. **The shipped defaults**: list headers and sender patterns put newsletters and receipts in Updates and Promotions when neither a rule nor the model decided.
Every row shows its reason on hover and in the reader: "Rule 2: from @linea-supply-example.com → Clients", "Model: Needs reply (0.91)", "Default: list header → Updates". A click on the reason opens the rule or the question that produced it.

## 2. Rules (Settings › General › Rules; "Create a rule from this message" on any row)
A rule = conditions + actions, written in words, no code.
- Conditions (all must match; one rule per combination): **from** address or domain · **to or cc** address or domain · **subject contains** words · **body contains** words · **has a list header** (a newsletter or mailing list) · **account** (which of your 30) · **has an attachment** · **size** over or under · **older than** N days. Sources: Gmail from/to/subject/has the words (recipient fields unverified at the official page), Sieve address/header/size tests, Apple Mail (unverified page); list header and account are proposed.
- Actions (any number): **put in tab** (Primary, Needs Reply, Updates, Promotions, Social, VIPs, or a tab you added) · **mark Needs reply** · **mark VIP** · **add label** · **skip the inbox** (archive on arrival) · **pin** · **notify** (even when the account is quiet) · **mute** (never notify). Sources: Gmail actions (label, archive, delete, star, forward), Sieve actions; the tab and VIP actions are Whoosh's own.
- "Create a rule from this message" opens the editor prefilled with the message's sender, domain, account and list header; you keep the conditions you want.
- "Test this rule" counts the matches in the last 30 days before you save it (proposed).
- Rules sync to the iPhone; the iPhone applies the same rules.

## 3. The decision model: what it is actually asked
Laya and Jev take the same thing: a **state** (the message) and **typed questions**; they return **answers with probabilities** (Laya README; Jev api/system-one.md). Whoosh ships three questions, shown in Settings in words, each editable:

| Question (its type) | Instructions sent | Criteria sent | Used for |
|---|---|---|---|
| **Which tab?** (choice) | "Which inbox tab should this email sit in?" | Primary: "a person writing to me directly about something I take part in" · Updates: "an automatic notice from a service I use: a receipt, a shipment, an alert, a report" · Promotions: "marketing, an offer, a newsletter I did not ask a person for" · Social: "a social network, a forum or a community notification" | the row's tab when no rule decided |
| **Needs my reply?** (noul, yes/no) | "Does this email ask me, personally, to answer or to do something?" | yes: "a question, a request or a deadline aimed at me" · no: "information only, or aimed at someone else, or automated" | the Needs Reply tab and the waiting count |
| **Has a sign-in code?** (noul) | "Does this email carry a one-time code, a login link or a verification number?" | yes: "a code or link meant to be typed or clicked once" · no: "anything else" | the notch's code pill and "Fill into the field in front" |

Why this shape: Jev's own guidance: "work best when each question asks one specific, well-scoped thing"; a choice "always picks one, even when nothing fits", so there is no "Other" option and the shipped defaults take the uncertain ones; "a choice without a description is interpreted by its name alone", so every tab carries a description. A tab you add in the tab editor gets its own description line, which is what the model reads.

- **What leaves the device for Jev** (never for Laya): from, to, cc, subject, the first 2,000 characters of the body (DECISION_BODY_CHARS), whether a list header is present, the account's display name. Never attachments, never the whole thread. Shown in Settings as one sentence with a switch per field (proposed).
- **Confidence floor** per question (Laya `answer_confidence`; Jev `confidence` for choice, the yes-probability for noul): below the floor the shipped default applies and the row is marked "unsure" with a one-tap correction. Starting floors are measured on your mail in the preview run, not typed (Laya: "a policy you choose from measured accuracy at that coverage on your data").
- **Teaching it**: every time you move a message to another tab, toggle Needs reply, or confirm a code, Whoosh records the message and your answer. Local Laya: after 35 examples (its minimum) Whoosh refits the calibration on this Mac, weekly, and keeps it (`fit`, `save_calibration`); Jev: there is no fine-tune, so Whoosh offers the recurring corrections as rules ("Mail from @hostcloud-example.net went to Updates 4 times: make it a rule?").
- **Preview run** (Settings): "Run on the last 20 messages" lists each message, its verdicts with confidence, and marks the ones that differ from where the message sits today; one tap moves the message or writes a rule.
- **Providers**: **Local (Laya)**: downloaded once (size shown), runs on this Mac, nothing leaves; **Jev (TypeSafe)**: the API key in the Keychain, endpoint `https://api.typesafe.ai/v1/systemone`, model `jev-latest`, a Test button that sends one fixed sample, the privacy sentence above, at most 2 requests in flight (the org saw errors above about 16), answers cached 24 h per message; **None**: rules and defaults only.
- Cost line for Jev, from the org's measurement: about $0.042 per million input tokens with about 260 hidden tokens per call; the preview run and each new message cost one call.

## 4. Open facts
- Apple Mail's exact rule names and Gmail's filter field names were not read from an official page; Whoosh does not copy them, it names its own conditions as above.
- Laya on device: the Python runtime is not for the app; the on-device route is ONNX (the card lists an ONNX extra) or a Core ML conversion, proven by a probe in the plan before the local provider ships; until then Jev and rules work.
- Laya's zero-shot accuracy on its own benchmark is low (0.362); the calibration on your corrections is what makes the local route useful, and the preview run shows whether it is.

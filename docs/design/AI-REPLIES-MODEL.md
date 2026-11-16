# AI replies: the model before any drawing

State: written by Cue on 2026-11-10 from the owner's messages of that day (PRD A109 and its notes). Confirmed by the owner so far: the account wall by default, clashes kept and flagged ("Yes make sense"). Not yet confirmed: the piece list as a whole, and how closely the editor follows Nexus (default below). No screen is drawn from this file until the owner has read the drawings' premise.

Owner's limits, verbatim: "We shouldn't try to fix too much but give existing sections and rules of thumbs and let people decide what they want to do"; "don't overengineer … still have access to full prompt of reply and be able to basically specify prompt, context"; "these numbering and style of heading are shit, too much AI generated style"; "You didn't make 10 genuinely different full designs"; "Think before you design. Think of all the compoenents of it (e.g., context, knowledge, tone of voice, instructions, etc.) how they can assemble, how they can be understood and when do we cal lthem"; every design "Consistent with the rest of the UI · Award winning clarity and UX · Beautiful · feel like easy to handle".

## 1. The pieces of a reply

| Piece | What it is | Made by | Per reply? |
|---|---|---|---|
| The mail | The message and its thread | Given | Yes |
| The ask | What the owner types for this one reply | The owner, in the moment | Yes |
| Instructions | How replies are to be written, always | The owner, once | No |
| Voice profile | A written description of how the owner writes, per kind of recipient | An analysis of sent mail (size chosen: 10, 50, 100, 500, all), then corrected by the owner | No |
| Voice examples | The owner's real earlier replies in this thread or to this person | Fetched | Yes |
| Knowledge found | Excerpts from mail and attachments that bear on this message | Searched | Yes |
| Knowledge fixed | Facts the owner pins (prices, terms, a bio) | The owner, once | No |
| Calendar | Free and busy times, events with this correspondent, from the connected calendar(s) | Fetched | Yes |
| The situation | Now, me, them, this thread, what is still open | Read from the store | Yes |
| The model's room | How much fits, and what is dropped first | A limit of the chosen model | No |

Two properties sort them: written by the owner or fetched by the app; fixed or resolved per reply. What the owner edits is therefore text plus references to fetched things.

## 2. When each piece is called

- Ahead, in the background: mail and attachments indexed for words and meaning (PRD A104); the voice analysis, once, on the chosen number of messages; a new run proposes changes and never overwrites the owner's corrections.
- When a mail arrives, auto-draft only: fixed filters first (label, facts such as VIP or unread; no model), then one model question, "does this need a reply from me?" (a yes or no, the decision model's kind of question).
- When a draft is written: (1) look up, (2) choose the voice (my replies in this thread, else my replies to this person, else the profile for that kind of recipient), (3) write, one call with everything assembled.
- After sending: the owner's edits may propose one change to the instructions or the voice profile; never applied alone.

Up to three model calls, each with its own short prompt the owner can read: should I draft, what to look up, write.

## 3. Retrieval: seven decisions

| Step | Question | Options |
|---|---|---|
| Where | Which mail and documents may be searched | Folders and accounts; attachments or not; how far back. DEFAULT, confirmed: this account only; crossing accounts is a deliberate choice |
| What to look for | How queries are made | The mail itself as the query (no model call); the model writes a few questions from the mail, the thread and the ask; names, dates, amounts, file names searched as words |
| Search | How a query runs | Words, meaning, or both; widening rings: this thread, this person, their company, everything |
| Filter | What is discarded | Quoted repeats, duplicates, weak matches, excluded folders, too old, never-sent drafts |
| Clashes | Two results disagree | Newest wins; mine wins; same thread wins; DEFAULT, confirmed: keep both, dated, and flag it in the draft |
| Shape | The form given to the writer | Raw excerpts; whole messages; one summary per thread; a fact sheet condensed by the model, each fact with its source |
| Subset | Which found items are used | Top few by score; the decision model judges each yes or no; the owner ticks them beside the draft |

Depth is the real choice: Quick (the mail is the query, 5 raw excerpts, no extra call) for a reply the owner waits on; Careful (model-written queries, each result judged, a fact sheet) for auto-drafts written in the background. With small local models, the chunk count and the summary choice matter most.

## 4. What the writer is handed

One label line per excerpt: a number to cite; the date and its age; who wrote it and to whom ("Me → Claire"); where it comes from (folder, thread subject, message 2 of 5); its kind (mail, attachment with file name and page); whether a later message in the same thread covers the same point.

The situation, once: today (date, weekday, time zone); me (name, account, signature for this recipient kind); them (name, company, kind, tu or vous, last exchange); this thread (To, Cc, anyone new, attachment names, language of the last message); the questions still unanswered in the thread, which also serve as queries.

Not given: signatures, legal footers, tracking lines, quoted history repeated under each reply, anything behind the account wall, the match score.

Calendars as a source (owner 2026-11-10: "Yes so potentially you can connect calendar(s) into whoosh and also use that as a source"): one or more calendars connect to Whoosh (the Mac's own calendars through EventKit first, read-only, on this Mac; an account's calendar through its own protocol later) and become a source like a folder: a reference "@calendar" gives the writer the free and busy times of the days in question and the events with this correspondent, each labelled with its date and calendar. It obeys the same wall as mail (a calendar belongs to an account or is allowed explicitly). Until a calendar is connected the writer is told it does not know the owner's availability; it asks or leaves a marked blank and never invents a slot.

## 5. Assembly

Order kept when room is short: the ask, the mail, the thread (newest first), instructions, voice, knowledge, examples. Each fetched piece has a size the owner sets. What was dropped is shown on the draft.

Levels: a default for all mail; an override for a label or a kind of recipient; the ask on one reply. Each level adds to the one above.

Invariant: a draft, never a send. The draft names what it used.

## 6. What the Nexus prompt editor teaches

- The prompt is one document with real headings, lists, quotes and code blocks; "/" inserts a block (Section, Heading, Comment, Quote, Bullet list, Ordered list, Task list, Code block, Knowledge, Snippet, Variables, Skills); "@" inserts a reference (Variables, Customer attributes, Skills, Knowledge); "%%" inserts a knowledge marker.
- A knowledge marker says in its tooltip what happens at run time (the most relevant chunks are retrieved and injected there, not the whole collection) and warns when it would inject nothing (detached, off, not found).
- Comments ("//") stay in the document and never reach the model.
- A section can hold one variant per channel and can be read-only.
- A drawer shows the compiled prompt, what the model receives; broken references are listed as issues with a jump; versions with drafts and publishing.

Consequences for Whoosh: every piece of section 1 is a reference placed in one document; a reference explains itself and shows its health ("voice not analysed yet", "folder not indexed"); rules of thumb ship as comments in the starting document; the compiled view on one real mail is the "full prompt" the owner asked for; a section may vary by kind of recipient. The knowledge reference's definition is one editable sentence carrying the seven choices of section 3, and the label line of section 4 is a small template inside it.

Default taken, the owner may reverse it: Whoosh follows Nexus's habits closely ("/" for blocks, "@" for references, "//" for comments, a compiled view), in Whoosh's own look, so the two products feel related and nothing is invented twice ("NEVER start things just from scratch").

## 7. What stays open for the drawings

The document does not answer three things, and the alternatives must differ on these, end to end (set up, draft, inspect, correct), never as one box in several wrappers:
1. Where "when" lives (on request, auto with filters and the yes or no question): on the label, in a rule list, or as its own section of the document.
2. How the three calls show: one document with three sections, three small documents, or a visible chain of steps.
3. Where the owner meets it: in Settings, beside a real draft, or both; and what the primary object is (a document with references, a library of pieces combined per situation, a visible pipeline, the draft as the place, teaching by correction, a conversation).

No numbered coloured badges as headings; no decoration that reads as generated.

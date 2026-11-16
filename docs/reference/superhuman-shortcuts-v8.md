# Superhuman keys v8 against Whoosh (read 2026-11-08)

Source: Superhuman Mail Keyboard Shortcuts, v8, Mac Edition (https://superhuman.com/shortcuts), read 2026-11-08. Compared to the Superhuman preset, the composer and the reader at commit 3fcf2bd.

Total 113: same 25, differs 6, missing 75, unverified (native text behaviour, not driven) 7.

| Section | Action | Superhuman | Whoosh today | Note |
|---|---|---|---|---|
| Actions | Superhuman Command | `⌘K` | same | ⌘K search: type an action or a search (owner's one-search design) |
| Actions | Search | `/` | same | / also opens the search |
| Actions | Undo | `Z` | same | Z undo |
| Actions | Ask AI | `?` | differs | ⇧? opens the key list |
| Actions | Select Conversation | `X` | missing |  |
| Actions | Add to Selection | `⇧J / ⇧K` | missing |  |
| Actions | Clear Selection | `esc` | missing | Esc is back only |
| Actions | Select All From Here | `⌘A` | differs | ⌘A selects all (J5.6) |
| Actions | Select All | `⌘⇧A` | missing |  |
| Conversations | Mark Done (Archive) | `E` | same | E archive |
| Conversations | Mark Not Done | `⇧E` | missing |  |
| Conversations | Remind Me (Snooze) | `H` | same | H snooze |
| Conversations | Star | `S` | same | S star |
| Conversations | Mark Read or Unread | `U` | same | U unread |
| Conversations | Trash | `#` | same | ⇧# delete |
| Conversations | Mark Spam | `!` | same | ⇧! junk |
| Conversations | Mute | `⇧M` | missing |  |
| Conversations | Unsubscribe | `⌘U` | missing |  |
| Messages | Compose | `C` | same | C compose |
| Messages | Reply All | `enter` | differs | A reply all; Enter opens only |
| Messages | Reply | `R` | same | R reply |
| Messages | Forward | `F` | same | F forward |
| Messages | Open Links & Attachments | `⌘O` | missing |  |
| Messages | Cycle through Links & Dates | `tab` | missing | Tab is next split only |
| Messages | Expand Message | `O` | missing |  |
| Messages | Expand All Messages | `⇧O` | missing |  |
| Labels | Add or Remove Label | `L` | same | L label |
| Labels | Remove Label | `Y` | missing |  |
| Labels | Remove Label, Next | `[` | missing |  |
| Labels | Remove Label, Previous | `]` | missing |  |
| Labels | Remove All Labels | `⇧Y` | missing |  |
| Labels | Move | `V` | differs | M move |
| Filters | Unread | `⇧U` | missing |  |
| Filters | Starred | `⇧S` | missing |  |
| Filters | Important | `⇧I` | missing |  |
| Filters | No Reply | `⇧R` | missing |  |
| Folders | Go to Inbox / Important | `G then I` | missing |  |
| Folders | Go to Other | `G then O` | missing | no Important/Other split |
| Folders | Go to Starred | `G then S` | missing | no Starred mailbox yet |
| Folders | Go to Drafts | `G then D` | missing |  |
| Folders | Go to Sent Mail | `G then T` | missing |  |
| Folders | Go to Done | `G then E` | missing | Archive |
| Folders | Go to Reminders | `G then H` | missing | Snoozed |
| Folders | Go to Muted | `G then M` | missing | no Muted view yet |
| Folders | Go to Snippets | `G then ;` | missing | snippets exist (composer panel); key missing |
| Folders | Go to Spam | `G then !` | missing | Junk |
| Folders | Go to Trash | `G then #` | missing |  |
| Folders | Go to All Mail | `G then A` | missing | no All mail view yet |
| Folders | Go to Label | `G then L` | missing |  |
| Calendar | Open Day | `0` | missing | no calendar |
| Calendar | Open Week | `0 then 0` | missing | no calendar |
| Calendar | Today | `T` | missing | no calendar |
| Calendar | Previous Day / Week | `P / -` | missing | no calendar |
| Calendar | Next Day / Week | `N / =` | missing | no calendar |
| Calendar | Create Event | `B` | missing | no calendar |
| Format | Bold | `⌘B` | unverified | may come from the native text view; not driven |
| Format | Italics | `⌘I` | unverified | may come from the native text view; not driven |
| Format | Underline | `⌘U` | unverified | may come from the native text view; not driven |
| Format | Hyperlink | `⌘K` | missing | no handler in the composer or rich text |
| Format | Color | `⌘O` | missing | no handler in the composer or rich text |
| Format | Strikethrough | `⌘⇧X` | missing | no handler in the composer or rich text |
| Format | Numbers | `⌘⇧7` | missing | no handler in the composer or rich text |
| Format | Bullets | `⌘⇧8` | missing | no handler in the composer or rich text |
| Format | Quote | `⌘⇧9` | missing | no handler in the composer or rich text |
| Format | Indent List | `tab` | missing | no handler in the composer or rich text |
| Format | Outdent List | `⇧tab` | missing | no handler in the composer or rich text |
| Format | Increase Indent | `⌘]` | missing | no handler in the composer or rich text |
| Format | Decrease Indent | `⌘[` | missing | no handler in the composer or rich text |
| Compose | To | `⌘⇧O` | missing | no handler in the composer or rich text |
| Compose | Cc | `⌘⇧C` | same | ⌘⇧C |
| Compose | Bcc | `⌘⇧B` | same | ⌘⇧B |
| Compose | From | `⌘⇧F` | missing | no handler in the composer or rich text |
| Compose | Edit Subject | `⌘⇧S` | missing | no handler in the composer or rich text |
| Compose | Edit Message | `⌘⇧M` | missing | no handler in the composer or rich text |
| Compose | Attach | `⌘⇧A` | missing | no handler in the composer or rich text |
| Compose | Discard Draft | `⌘⇧,` | missing | no handler in the composer or rich text |
| Compose | Move Contacts to Bcc | `⌘⇧I` | missing | no handler in the composer or rich text |
| Compose | Remind Me | `⌘⇧H` | missing | no reminder-if-no-reply yet |
| Compose | Send Later | `⌘⇧L` | same | ⌘⇧L |
| Compose | Use Snippet inline | `;` | missing | snippets exist (composer panel); key missing |
| Compose | Use Snippet | `⌘;` | same | ⌘; opens the snippets panel |
| Compose | Insert Emoji | `:smile` | unverified | may come from the native text view; not driven |
| Compose | Send | `⌘enter` | same | ⌘Enter send |
| Compose | Send & Mark Done | `⌘⇧enter` | same | ⌘⇧Enter |
| Compose | Instant Send | `⌘⇧Z` | same | ⌘⇧Z |
| Navigation | Superhuman Focus | `→ ← ↑ ↓` | missing | arrows move focus between panes |
| Navigation | Next Split | `tab` | same | Tab next split |
| Navigation | Previous Split | `⇧tab` | same | Shift+Tab previous split |
| Navigation | Open Conversation | `enter` | same | Enter open |
| Navigation | Next / Previous Conversation | `J / K` | same | J next, K previous |
| Navigation | Next / Previous Message | `N / P` | differs | P is pin; N unbound |
| Navigation | Scroll Down | `space` | unverified | may come from the native text view; not driven |
| Navigation | Scroll Up | `⇧space` | unverified | may come from the native text view; not driven |
| Navigation | Jump to Top / Bottom | `⌘↑ / ⌘↓` | unverified | may come from the native text view; not driven |
| Navigation | New Window | `⌘N` | missing |  |
| Navigation | New Tab | `⌘T` | missing | no tabs feature |
| Navigation | Next Tab | `⌘⇧]` | missing | no tabs feature |
| Navigation | Previous Tab | `⌘⇧[` | missing | no tabs feature |
| Navigation | Switch Tabs | `⌘1-9` | missing | no tabs feature |
| Navigation | Close Tab | `⌘W` | missing | no tabs feature |
| Navigation | Switch Accounts | `ctrl 1-9` | differs | ⌘J opens the accounts menu |
| Navigation | Increase Font Size | `⌘=` | missing |  |
| Navigation | Decrease Font Size | `⌘-` | missing |  |
| Navigation | Reset Font Size | `⌘0` | missing |  |
| Navigation | Copy Page Link | `ctrl /` | missing |  |
| Navigation | Back | `esc` | same | Esc back |
| Pop Out Compose | Pop Draft In or Out | `⌘⇧P` | missing | no pop-out compose yet |
| Pop Out Compose | Pop Out Reply | `⇧R` | missing |  |
| Pop Out Compose | Pop Out Reply-all | `⇧enter` | missing |  |
| Pop Out Compose | Pop Out New Message | `⇧C` | missing |  |
| Pop Out Compose | Pop Out Forward | `⇧F` | missing |  |
| Pop Out Compose | Pop Out a Draft & Search | `⌘/` | missing |  |
| Pop Out Compose | Switch to or from a Draft | `⌘D` | missing |  |

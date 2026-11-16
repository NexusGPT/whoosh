# Surface: mac-entra-guide (the in-app guide "Get your organisation's app ID")

Objective: an admin who has never seen Entra registers a Whoosh app in their own tenant and pastes its client ID into the add-account sheet, without leaving the app's words (PRD A90, 15:28; J9 does not include it: the guide opens from "How to get one" on the add-account sheet and returns to it with the ID filled).

picked-layout: E1 Screenshot stage — owner: "Screenshot stage is fine but you should have a bit more step (e.g., micosoroft graph clicking)" (card 2026-11-09 16:14; boards docs/design/boards/entra-guide-r1/, informed lane; blind E3 and informed E2 declined)
picked-style: E1 Screenshot stage (the same board carries layout and look, as the round was run) — owner: same words, 16:14

## The owner's amendment (binding)
More steps than seven: every click the admin makes is its own step with its own screenshot and ring, "e.g., micosoroft graph clicking". The seven stages of the board stay as chapter heads; inside each, every target (Name, Who can use it, Platform, Redirect address, Register; Overview, Copy the ID; Authentication, the toggle, Save; Add a permission, Microsoft Graph, Delegated, search IMAP, tick, search SMTP, tick, offline_access, Add permissions; Grant admin consent, the ticks) is a numbered step of its own in the progress bar and the step counter ("Step 9 of 21"), not a sub-dot.

## Inputs
- Screenshots: docs/guides/entra-shots/flow/step-01..09.png with the blur/highlight/remove rule (README there); the cleaned copies the board lanes made under docs/design/boards/entra-guide-r1/_assets-informed/; the account-type shot is 2026-11-09-144854-AF7C6ED6.png (the flow README's 152445 pointer is wrong). Missing shots: the sign-in page; IMAP.AccessAsUser.All and SMTP.Send ticked (drawn stand-ins marked "real shot to come" until the owner captures them). Three files in docs/guides/entra-shots/ are not Entra (144025, 144035, 144048): not used.
- The register-form shot shows Entra's red address error; the guide shows the clean state (painted) with the caption to paste the address again.
- Account type caption for an admin registering their OWN app: "Accounts in this organizational directory only" is enough; the owner's own registration (the shipped default) is "Any Entra ID Tenant + Personal Microsoft accounts".

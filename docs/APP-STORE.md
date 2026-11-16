# Publishing Whoosh on the App Store under Nexus (on top of the open-source project)

Owner, 2026-11-09 17:52 (verbatim): "keep all of these elements as list of things we need to add as we want to publish the app on the app store under nexus (on top of the opensource project)".

The open-source repository builds and runs with no Apple account and no Nexus secret. The App Store build is the SAME code plus the items below, each supplied by Nexus outside the repository (a signing identity, entitlements, ids in a config file the repo ignores). One row per item; "Found" says where the need surfaced.

| # | Item | What it enables | Owner | Found |
|---|---|---|---|---|
| 1 | Apple Developer Program membership for Nexus (team id) | signing, notarisation, App Store Connect, TestFlight, push, iCloud | Nexus (the owner's "Register outside accounts") | plan, 2026-11-08 |
| 2 | Developer ID + App Store distribution certificates and provisioning profiles for `io.nexus.whoosh` (Mac) and the iPhone app | a signed, notarised build; no "unidentified developer" warning; App Store upload | Nexus | pipeline.md "Signing and shipping" |
| 3 | Hardened runtime + App Sandbox entitlements (the Mac App Store requires the sandbox) | network client (IMAP/SMTP), user-selected files (the import step picks ~/Library/Mail through an open panel or a security-scoped bookmark instead of Full Disk Access), microphone (dictation), Keychain | Cue (entitlements file) | import crate (A83), dictation |
| 4 | Keychain access group + iCloud Keychain entitlement | credentials and the sign-in ID synced across the person's Macs (today stored locally: errSecMissingEntitlement -34018 on the ad-hoc build, owner's screenshot 17:30) | Nexus (entitlement) + Cue (SyncMode::ICloud back on) | main47 sign-in |
| 5 | Push Notifications entitlement + an APNs key for the relay | new-mail push on iPhone and Mac through the hosted relay (today "health degraded (no APNs)") | Nexus | relay v6 |
| 6 | iCloud container (CloudKit) | sync of settings, labels and rules between devices (devicesync) | Nexus | devicesync lane |
| 7 | App Group | the notch panel and any helper sharing state with the app | Cue | notch |
| 8 | Privacy: usage strings (microphone for dictation, Contacts if used, Automation if used) and the App Privacy label "Data not collected" (nothing leaves the Mac; the relay carries tokens only) | review passes; honest privacy card | Cue | PRD "nothing leaves this Mac" |
| 9 | The default Microsoft sign-in ID (2d200179-…) stays Nexus's registration; a Google OAuth client for the store build needs Google's verification (the open-source build keeps app passwords) | sign-in without pasting an ID, in the store build too | Nexus + Google review | A88 |
| 10 | App Store Connect record: bundle ids, version and build numbers from the stamp, 1024 px icon and the icon set (the glass envelope), screenshots (light and dark, 14 languages or the store's subset), description, keywords, support URL, privacy policy URL | the listing | Nexus marketing + Cue (assets from the pipeline) | A86 icon |
| 11 | Review notes with a test mailbox (a demo IMAP account the reviewer can open) and the "why we need" lines for microphone and files | review passes first time | Nexus | |
| 12 | Open-source licence compliance in the About box (docs/LICENCES.md rendered) and the export-compliance answer (standard encryption, exempt) | review passes | Cue | docs/LICENCES.md |
| 13 | Sparkle-style updates are NOT used in the store build (the store updates it); the open-source build may ship its own update feed later | two update paths, one per channel | Cue | pipeline.md |
| 14 | TestFlight for the iPhone app and the Mac app before the public release | the owner and testers install signed builds | Nexus | |
| 15 | Microsoft publisher verification of the Nexus app registration (Partner Center MPN id linked to the Entra registration; the registration's publisher domain set) | other organisations see "verified publisher" on the consent page; their users can consent without an admin (tenants created after 2020 block unverified multi-tenant apps for user consent) | Nexus | owner 17:55: "same for validation from Azure and Google" |
| 16 | Google OAuth app verification for a Nexus-owned Google client (brand verification + the restricted-scope security assessment for the Gmail scope `https://mail.google.com/`; a verified consent screen with privacy policy and homepage) | "Sign in with Google" in the store build without the "unverified app" screen and the 100-user test cap; the open-source build keeps app passwords | Nexus + Google review (weeks, a paid CASA assessment for the restricted scope) | owner 17:55 |

Rule: a need that appears in a build (a refused entitlement, a permission prompt, a review guideline) is appended here the day it is found, with the build that found it.

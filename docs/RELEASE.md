# Release: how Whoosh reaches the owner's Mac and iPhone

Two paths. Path A runs today and needs no Apple account. Path B waits for the owner's Developer ID and is written as exact steps. Nothing on either path runs unattended: no notarisation request, upload, App Store submission or update feed leaves this Mac until the owner gives the go (docs/CONSTRAINTS.md: "Nothing leaves this Mac during the owner's absence").

Every install writes a row in docs/ARTIFACT.md. A build counts as installed only when the stamp (`NexusBuildLabel` + `NexusBuildCommit`) is read back from the install folder.

## Path A: no Developer ID (ad-hoc signing), in force now

| Step | Command | What proves it |
|---|---|---|
| 1. Build with a stamp | `sh scripts/build.sh <label> <worktree>` | `build-<label>.json` in the build tree; the five `Nexus*` keys in the bundle's `Info.plist`; signed with `SIGN_IDENTITY=-` (ad hoc) |
| 2. Round | `sh scripts/round.sh <label>` | `verdict-<label>.json` GREEN on the same commit |
| 3. Install | `LAUNCH_ENV="WHOOSH_FIXTURE_CORE=1" sh scripts/install.sh <label>` | the stamp read back from the install folder; a row in docs/ARTIFACT.md; the previous install kept as `Whoosh <date>.app` (the rollback) |
| 4. Smoke | runs inside step 3 | `checks/smoke-check.py` exits 0 on the installed bytes under the test identity |
| 5. Sweep | `sh scripts/sweep.sh` | old build trees and surplus rollback copies removed by name |

The install folder is `INSTALL_DIR` in docs/pipeline.env: `~/Applications` during the unattended build, `/Applications` after the owner's first review. Step 3 needs `LAUNCH_ENV` because the smoke archive runs on the fixture core until the Rust core serves real mail; docs/pipeline.env is the root's file, so setting it there is a finding in the release report.

**Rollback** (the line install.sh prints): quit Whoosh, then `rm -rf "<install folder>/Whoosh.app" && mv "<install folder>/Whoosh <date>.app" "<install folder>/Whoosh.app"`. A stamp that reads back wrong puts the previous install back by itself.

### Opening an ad-hoc build

A build that stays on this Mac carries no quarantine flag, so it opens at once. A copy that arrives by download, AirDrop or a message carries `com.apple.quarantine`, and Gatekeeper refuses an ad-hoc signature with no notarisation. The owner clears it with one of:

1. Terminal: `xattr -dr com.apple.quarantine /Applications/Whoosh.app`
2. System Settings, Privacy & Security, scroll to the line naming Whoosh, "Open Anyway", confirm with the password.

Neither step is needed for installs made by `scripts/install.sh` on the owner's own Mac. `spctl -a -vv` on the installed ad-hoc build r1 printed `~/Applications/Whoosh.app: rejected` (observed 2026-11-07, `_captures/release/readback-r1.txt`). That is the expected answer on this path, and the reason Path B exists. The installed copy carries `com.apple.provenance` and no `com.apple.quarantine`; it was not launched in place by this lane (the smoke runs a test-identity copy of it), so how Finder treats the first double-click of the installed copy is not tested here.

### Release mode of install.sh (the Gatekeeper step)

`WHOOSH_INSTALL_RELEASE=1 sh scripts/install.sh <label>` adds one step after the copy is in place and the stamp is read back: `spctl -a -vv` on the installed app. Unless its output says `accepted`, the install exits with code 3 and puts the previous install back. It prints one line, `gatekeeper: accepted, source <the source Gatekeeper names>` (for a notarised build, `source=Notarized Developer ID`). Without the variable nothing changes: no new step, no new line, the same exit codes (0 installed, 1 refused or failed, 2 bad usage or no build). An ad-hoc build is rejected by this step, as it must be; a notarised build needs the owner's Developer ID and credentials (B1). The assessment can be replaced by a stand-in only inside `sh scripts/install.sh --selftest`, which installs nothing real.

## Path B: Developer ID (pending: the owner gives the Developer ID)

par-release stays pending until these exist. Each item names what the owner does, then what the release seat does after.

### B1. Signing and notarisation (Mac)

Owner:
1. Enrol the organisation (or the owner) in the Apple Developer Program and read the Team ID in the developer account's Membership page.
2. In Xcode, Settings, Accounts, Manage Certificates: create a "Developer ID Application" certificate. The private key stays in the owner's login keychain.
3. Create an app-specific password at account.apple.com (Sign-In and Security) or an App Store Connect API key, then store it once on the Mac that notarises:
   `xcrun notarytool store-credentials whoosh-notary --apple-id <apple-id> --team-id <TEAMID> --password <app-specific-password>`
   The secret is typed by the owner into that command and goes to the keychain only. It is never written in this repo, a brief or a transcript.
4. Tell the release seat: the certificate's full name (`Developer ID Application: <Name> (<TEAMID>)`), the Team ID, and the keychain profile name.

Release seat then:
1. `docs/pipeline.env`: `SIGN_IDENTITY="Developer ID Application: <Name> (<TEAMID>)"` (the root's file; a request row). `apps/mac/project.yml`: `DEVELOPMENT_TEAM: <TEAMID>`, `ENABLE_HARDENED_RUNTIME: YES`, a Release configuration, and the entitlements file under `apps/mac/Signing/` (microphone for dictation, Keychain access group, outgoing network; no more).
2. Build Release: `sh scripts/build.sh <label> <worktree>` with a Release `BUILD_CMD`. Signing happens in build.sh with `--options runtime --timestamp` added.
3. Archive and submit: `ditto -c -k --keepParent Whoosh.app Whoosh.zip`, then `xcrun notarytool submit Whoosh.zip --keychain-profile whoosh-notary --wait`. A status other than Accepted: read `xcrun notarytool log <id>` and fix before anything else.
4. Staple: `xcrun stapler staple Whoosh.app`, then `xcrun stapler validate Whoosh.app`.
5. Install: `sh scripts/install.sh <label>`.
6. Prove it: `spctl -a -vv /Applications/Whoosh.app` must report `accepted` with `source=Notarized Developer ID`. This is the par-release acceptance line.

### B2. In-app updates (Mac): Sparkle

Owner: choose where the update files are served from (an HTTPS address the owner controls) and confirm the first public build number.

Release seat then:
1. Add Sparkle 2 (MIT) as a Swift package in `apps/mac/project.yml`.
2. Generate the update-signing key pair with Sparkle's `generate_keys`. The private key stays in the owner's login keychain; the public key goes in `Info.plist` as `SUPublicEDKey`; the feed address goes in as `SUFeedURL`.
3. After each notarised build: zip the stapled app, run `generate_appcast` on the folder, and place the feed and archive at the owner's address. Publishing is the owner's go every time.
4. Fixture for the acceptance line: an appcast under `apps/mac/Updates/` that offers the next build number to the installed one, checked offline before any feed exists.

### B3. iPhone: App Store and TestFlight

Owner:
1. In App Store Connect: create the app record for bundle id `io.nexus.whoosh`, set the name (the product name is still open; the bundle id is kept), and fill the privacy label (no account with us; mail stays on the device; Apple crash reports only).
2. In the developer account: enable Push Notifications for the identifier, create the APNs auth key (`.p8`), and hand its key id and team id to the relay lane through the secret route (never the file in the repo).

Release seat then:
1. `apps/ios/project.yml` (root-frozen: a request row): `DEVELOPMENT_TEAM`, `CODE_SIGNING_ALLOWED: YES`, automatic signing, the push entitlement under `apps/ios/Signing/`.
2. Archive for a device with `xcodebuild archive`, export with `-exportOptionsPlist` method `app-store-connect`, upload through Xcode Organizer or Transporter.
3. TestFlight first (the PRD rollout: self beta, then a beta list, then public once Google's review is through); App Store review after the beta holds.

### B4. Licensing

Mac (PRD default, tq-mac-licensing): Polar. Owner creates the Polar organisation and a product with a licence-key benefit, then gives the release seat the organisation id. The app validates a pasted key against Polar's licence-key endpoint and stores the result in the Keychain. Pricing is a later card. The licence check (`checks/licence-check.py`) stays about code licences; the unlock test in par-release uses a test key against a recorded fixture of Polar's reply.

iPhone: StoreKit through RevenueCat. Owner creates the RevenueCat project, links the App Store Connect app, defines the products in App Store Connect and gives the release seat the public SDK key (a public key; the secret API key stays with the owner).

### B5. Crash reports

Apple's, anonymised (tq-crash-reports): no third-party SDK, no analytics. iPhone: Xcode Organizer Crashes for TestFlight and App Store builds. Mac: the crash reports on the owner's own Mac land in `~/Library/Logs/DiagnosticReports`, which `scripts/round.sh` already watches. The plan row (docs/plan.json tq-crash-reports) expects full coverage on the direct-download build and says to revisit the pick if the self beta shows it too thin. That coverage claim is not tested.

## What is not decided or not tested

- Developer ID path: no step in B1 to B5 has been run. They are Apple's documented commands written for this repo, and each needs the owner's account.
- Mac licensing: Polar is a default the owner can still turn into Paddle.
- Product name: Whoosh is a placeholder. A rename changes the bundle name, `APP_NAME` in docs/pipeline.env and the install folder entry; the bundle id `io.nexus.whoosh` is kept by default.

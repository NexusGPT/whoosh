# Signing in to Microsoft 365 and Google Workspace

## The default: Whoosh's own registration

For a Microsoft 365 address, Whoosh signs in with its own Microsoft registration, the way Thunderbird signs in with Mozilla's. You type your address, click Sign in, and approve on Microsoft's page in your browser. There is no ID to paste.

- **What ships.** One Microsoft application (client) ID, registered by Whoosh's maintainer as a multi-tenant public client: any organisation's accounts can sign in, the app holds no secret (PKCE, a local redirect), and it asks only for mail: `IMAP.AccessAsUser.All`, `SMTP.Send`, `offline_access`. The ID sits in one file of the source, `Sources/Features/AddAccount/DefaultSignInClients.swift`, so anyone can read it. Your refresh token stays in your Keychain; nothing passes through a Whoosh server.
- **What your organisation may see.** Until Microsoft verifies the publisher, its consent page names the app "unverified". If your organisation lets people approve apps themselves, you approve once. If it does not, the page asks for an administrator: an admin can approve Whoosh for everyone in Entra (Enterprise applications › Whoosh › Permissions › Grant admin consent), or register your own app as below.
- **Using your organisation's own app instead.** Under Sign in, click "Use your organisation's own ID" and paste the ID of an app registered in your own admin (the steps below). Whoosh then signs in with yours. An admin who must keep every app in-house uses this.

Gmail and Google Workspace have no default: a personal Gmail address uses an app password, and a Workspace domain that prefers sign-in registers its own app (below).

## Your organisation's own app ID (the override)

Whoosh never submits anything to Microsoft or Google for you. To use your own registration, you register one app in your OWN admin console (five minutes, free, no review) and paste its client ID into Whoosh once. Whoosh keeps it in your Keychain and signs in with it (PKCE, a local redirect, no secret).

Why: Microsoft turned off password sign-in for IMAP in 2022, so a password cannot work on Microsoft 365. Gmail accepts an app password instead (Whoosh shows the link); a Google app ID is for Workspace domains that prefer sign-in.

## Microsoft 365 (your tenant)

1. Open https://entra.microsoft.com, sign in as an admin of your organisation.
2. Identity › Applications › App registrations › New registration.
3. Name: Whoosh. Supported account types: "Accounts in this organizational directory only" (one tenant). Redirect URI: platform "Mobile and desktop applications", value `http://localhost`. Register.
4. Authentication › Advanced settings › "Allow public client flows": Yes. Save.
5. API permissions › Add a permission › Microsoft Graph › Delegated: `IMAP.AccessAsUser.All`, `SMTP.Send`, `offline_access`. Add. If your tenant requires admin consent, click "Grant admin consent".
6. Overview › copy the "Application (client) ID" (a GUID such as 9e5f…-…).
7. In Whoosh › Add account: type your address and leave the password empty; when the sheet says Microsoft 365, click "Use your organisation's own ID" if it shows, paste the ID into "Your organisation's app ID" and click Sign in. Your browser opens on Microsoft's page; approve; Whoosh finishes alone.

Nothing is submitted to Microsoft. The ID stays in your Keychain; the refresh token too.

## Google Workspace (your domain)

1. Open https://console.cloud.google.com, pick or create a project.
2. APIs & Services › OAuth consent screen: User type "Internal" (your Workspace users only: no verification, ever). App name: Whoosh. Save.
3. Credentials › Create credentials › OAuth client ID › Application type "Desktop app". Create. Copy the client ID (…apps.googleusercontent.com).
4. In Whoosh › Add account: paste it into "Your organisation's app ID", click Sign in, approve in the browser.

For a personal @gmail.com address, use an app password instead: https://myaccount.google.com/apppasswords (two-step verification on), then type it in the password field. No app ID needed.

## For Whoosh's maintainer: registering the default

The steps of the Microsoft section above, with two changes: in step 3 pick "Accounts in any organizational directory" (multi-tenant), and skip "Grant admin consent" (each organisation consents for itself). Paste the Application (client) ID as the value of `microsoft` in `Sources/Features/AddAccount/DefaultSignInClients.swift`; `checks/no-vendor-id-check.py` allows exactly that one ID, on that line, and no client secret anywhere. Publisher verification (Entra › App registrations › Branding & properties) removes the "unverified" notice.

## What Whoosh stores

- Keychain, item `<address>:oauth-client`: the client ID (Whoosh's default or your own).
- Keychain, item `<address>:oauth-refresh`: the refresh token, rotated on refresh.
- Nothing in the app's database. In the repository: only Whoosh's own default Microsoft ID, in `DefaultSignInClients.swift`; `checks/no-vendor-id-check.py` fails the build on any other vendor ID or any client secret.

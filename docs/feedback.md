# In-app feedback

HideMyWindows → About → **Send feedback** files a GitHub issue on `MehdiMamas/HideMyWindows`, opened **with the reporter's own GitHub account**. Screenshots and scrubbed logs go to the private repo `MehdiMamas/hidemywindows-feedback`. The relay that does this lives in the private repo `MehdiMamas/hmw-feedback-relay` (Vercel, `https://hmw-feedback.vercel.app`).

## Moving parts

| Part | What | Where its secrets live |
| --- | --- | --- |
| **HideMyWindows Feedback** (GitHub App, sign-in) | Device-flow sign-in for reporters. Permissions: Issues read/write, Metadata read. Installed **only** on `MehdiMamas/HideMyWindows`. User tokens do not expire. Its user tokens can open issues on HMW and nothing else. The relay never holds its private key, so it can never file an issue as a bot. Also the web sign-in for `/admin`. | Client id is public (in the app). Client secret (admin sign-in only): Vercel env. |
| **HideMyWindows Feedback Attachments** (GitHub App) | Writes `reports/<id>/` and `moderation/bans/<github id>.json`. Permissions: Contents read/write, Metadata read. Installed **only** on `MehdiMamas/hidemywindows-feedback`. | App id + private key: Vercel env. |
| Relay (`hmw-feedback-relay`) | Verifies the reporter, checks bans and limits, stores attachments, opens the issue with the reporter's token, never stores it. | Vercel env only. |
| App (this repo) | Sign-in, form, scrubbing, ban screen. | Reporter token: Windows Credential Manager (`keyring`, service `HideMyWindows Feedback`). |
| Triage (local scheduled task on Mehdi's PC) | Debounced batch triage with one Sonnet `claude -p` session. Lives in `hmw-feedback-relay/triage/`. | Uses the local `gh` and `claude` logins (OS keychain). |

## Relay API

Base `https://hmw-feedback.vercel.app`. Every request sends `X-HMW-Client-Key: <public client key>` (not a secret; turns away requests that are not from the app).

### `POST /api/feedback`
Headers: `Authorization: Bearer <reporter GitHub user token>`, `Content-Type: application/json`.

```json
{
  "kind": "bug" | "idea",
  "title": "short title, 1-120 chars",
  "body": "what happened, 1-20000 chars",
  "appVersion": "2.3.2",
  "os": "Windows 11 Pro 10.0.26300",
  "attachments": [
    { "name": "screenshot.png", "type": "image/png", "data": "<base64>" },
    { "name": "diagnostics.txt", "type": "text/plain", "data": "<base64 of UTF-8>" }
  ]
}
```

Limits: request ≤ 4.4 MB, ≤ 5 attachments, ≤ 3.2 MB decoded total, only PNG/JPEG (magic bytes checked) and UTF-8 text. The relay scrubs text attachments and the body again (emails, `C:\Users\<name>`, `/Users/<name>`, `/home/<name>`).

Answers:
- `201 {"issue": 123, "url": "https://github.com/MehdiMamas/HideMyWindows/issues/123", "id": "<report id>"}`
- `400 {"error": "<readable sentence>"}` bad input
- `401 {"error": "...", "code": "github_login_required"}` missing/invalid token → app signs in again
- `403 {"error": "banned", "ban": {login, bannedAt, reason, reportExcerpt, issue, appealEmail}}`
- `429 {"error": "Try again in 4 minutes.", "retryAfter": 240}` + `Retry-After`
- `502 {"error": "..."}` GitHub refused (e.g. the token lacks access)
- `503 {"error": "The feedback service is not set up yet."}`

### `GET /api/feedback/status`
Bearer token. `200 {"banned": false, "login": "octocat"}` or `200 {"banned": true, "ban": {...}}`; `401` as above.

### Issue format
Title: `[Bug] <title>` or `[Idea] <title>`. First body line: `**From:** [@login](https://github.com/login) (verified, sent from HideMyWindows <appVersion>)`. Then the text, an **Attachments** section linking `https://github.com/MehdiMamas/hidemywindows-feedback/tree/main/reports/<id>` (private: only the maintainer can open it), and a hidden marker `<!-- hmw-feedback:<id> -->`. Labels cannot be set by a non-collaborator's token, so triage adds `feedback from user`, `bug`/`enhancement` and `triaged`.

### Private repo layout (`hidemywindows-feedback`)
- `reports/<id>/meta.json`: `{id, githubId, login, kind, title, issue, url, createdAt, appVersion, os, attachments: [names]}` — the source of truth for triage (an issue body marker alone can be forged).
- `reports/<id>/<attachment>`
- `moderation/bans/<github id>.json`: `{githubId, login, bannedAt, reason, reportText, issue, source: "triage"|"manual", notes: [{at, by, text}], unbannedAt, unbannedBy}`.

## Bans and appeals
A ban is keyed on the GitHub user id. The app checks `/api/feedback/status` when the form opens and shows the ban screen: why, the text written, the date, and the appeal address `me@mehdimamas.dev`. `/admin` (GitHub sign-in, allowlisted ids in `ADMIN_GITHUB_IDS`) lists bans with what was written, and lets the maintainer annotate, unban (the record stays) or ban by login.

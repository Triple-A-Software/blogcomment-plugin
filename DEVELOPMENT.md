# Development

Developer notes for the `blog-comments` Neleto plugin. For the user-facing
description, see [readme.md](readme.md).

> Status: **P1 + P2 + P3** of the [plugin roadmap](../plugin-cli/docs/plugin-roadmap.md).
> P1: flat comments, submit page, moderation UI, captcha, render helper. P2:
> threaded replies, email notifications, dashboard cards. P3: Akismet spam
> scoring, GDPR erase-by-email, reactions (likes).

## How it works

The plugin runs as a standard Neleto plugin subprocess and reads two databases:
`DATABASE_URL` (its own `comment` + `comment_settings`) and `CMS_DATABASE_URL`
(the `post` table, to validate a comment's target and show post titles in the
admin — the two databases are separate, so post titles are a second query, not a
join).

The CMS already owns posts; this plugin only owns the comments.

## Public flow

1. **Render** — a developer drops `{{ comments(post.id) }}` into the blog-post
   template. The `comments` helper (`/helper/comments`) renders the approved
   thread + a submit form. `post.id` is exposed in the post render context; if no
   id is passed it falls back to resolving the post from the `:slug` route param.
2. **Submit** — the form POSTs `application/x-www-form-urlencoded` to
   **`/comments/submit`**, a `pages` route with `allow_select_layout: false` so
   Neleto forwards the body through unchanged (mirrors form-plugin's public
   submit). The handler validates, stores the comment (default status
   `pending`), and **redirects back** to the post with a `?comment=<flag>` the
   helper turns into a status banner (`received` / `captcha` / `slow_down` /
   `error`).

## Spam defences (layered, safe by default)

- **Honeypot** — a visually-hidden `website` field; any value = bot → silently
  dropped (shown as success).
- **Time-trap** — a JS-stamped load timestamp; submits faster than 2s are
  rejected. Missing/garbled `ts` (no-JS visitors) is never penalised.
- **Moderation by default** — comments are `pending` until approved, so spam
  never reaches the page even if the above are bypassed.
- **Rate limit** — per-IP (5/min) plus a global cap (30/min). The Neleto proxy
  talks to the plugin over localhost and doesn't add `X-Forwarded-For`, so the
  real client IP is only available when a reverse proxy in front of Neleto sets
  it; the per-IP limit is best-effort and the global cap is the backstop.
- **Captcha (optional)** — hCaptcha or reCAPTCHA, verified server-side
  (`antispam::verify_captcha`). The widget is injected into the form only when a
  provider + site key are configured; a provider with no secret fails open (a
  config mistake shouldn't lock out every visitor — the honeypot + moderation
  still apply).

## Threaded replies

Comments carry a `parent_id`. `approved_comments` returns the flat approved set
(each with a like count); `render.rs` groups them by parent and renders the tree
recursively, indenting up to `MAX_DEPTH` (deeper replies keep nesting in markup
but stop indenting). Each comment gets a `<details>`-based reply form (no JS
needed) whose `parent_id` is preset. Reply forms **omit the captcha** (only the
top-level form carries it) to avoid many widgets in a long thread; replies are
lower-risk since the parent must be an approved comment on the same post
(`is_valid_parent`), and moderation still applies.

## Reactions (likes)

A like posts to **`/comments/react`** (a public `pages` route). Enhanced clients
send `Accept: application/json` and get `{likes}` back to update the count
in-place (`render.rs` `SCRIPT`); a plain form post redirects to the comment
anchor. Likes are deduped per IP in `comment_reaction` — best-effort behind the
proxy (no real IP → `"anon"`, so anonymous likes collapse together rather than
being unlimited).

## Email notifications

Sent via the Neleto backend's internal `POST /email/send`
(`email::send` → `http://localhost:$INTERNAL_BACKEND_PORT/email/send`), so the
plugin needs no SMTP config. A moderator address (`notify_email`) is emailed on
every new comment; the parent comment's author is emailed on a reply if they
left an address. Both are spawned (`tokio::spawn`) so mail never blocks or fails
the submit, and no-op with a warning when `INTERNAL_BACKEND_PORT` is unset.

## Akismet (optional)

When an `akismet_key` is set, `antispam::akismet_is_spam` calls Akismet's
`comment-check` on submit; a spam verdict stores the comment as `spam` (hidden)
instead of pending. Akismet keys on the site URL, derived from the forwarded
`Host` header (`site_url`); if that's missing/localhost the check is skipped. It
**fails open** — any Akismet error is treated as ham, since moderation is the
real gate.

## GDPR

`/api/comments/erase` (admin) handles a data-subject request by email: **delete**
removes all matching comments, **anonymize** keeps the text but strips
name/email/IP/UA. This is the operator-actioned flow (the subject asks, the
operator actions it); a public self-service flow would need email-verified
confirmation and isn't built.

## XSS

Comment text is escaped at render time (`utils::escape_html` /
`escape_multiline`, the latter turning newlines into `<br>`). Stored bodies are
raw text and never interpreted as HTML — the escaping boundary lives entirely in
`render.rs`. Notification email bodies escape the comment fields too.

## Manifest hooks

| Hook | Route | Purpose |
|---|---|---|
| `helpers` | `/helper/comments` | `{{ comments(post.id) }}` — renders threaded thread + form |
| `pages` | `/comments/submit` | Public form POST (no layout), redirects back |
| `pages` | `/comments/react` | Public like endpoint (JSON or redirect) |
| `dashboard_cards` | `/dashboard/pending`, `/dashboard/recent` | Moderation queue size + latest approved |
| `api` | `/api/comments`, `/api/comments/moderate`, `/api/comments/erase`, `/api/stats` | Moderation list + actions + GDPR erase + counts |
| `api` | `/api/settings` | Moderation / email / captcha / Akismet settings |
| `ui` | `/ui` | Moderation panel + settings + GDPR erase |

## Theming

Dashboard cards and the admin UI render in same-origin iframes themed with Nuxt
UI CSS variables. `templates/theme_head.html` (inlined into cards, duplicated in
the admin UI) copies the parent document's resolved `--ui-*` colors + `.dark`
class onto the iframe root, with standalone fallbacks.

## Running locally

```sh
cp .env.example .env      # point DATABASE_URL + CMS_DATABASE_URL at local Postgres
cargo run                 # (or `just dev` with cargo-watch)
```

The admin UI is a single static file (`ui/dist/index.html`) — no build step. It
derives the API base path from its own URL, so it works behind the Neleto proxy
at `/api/rest/plugins/blog-comments/…`.

Quick manual check of the submit path without the CMS wiring is awkward (it
validates the post against the CMS), but the render helper and anti-spam logic
are covered by unit tests (`cargo test`).

## Layout

```
src/
  main.rs        axum app + routing
  lib.rs         AppState, DB + template setup
  model.rs       CMS post row, comment rows (parent_id, likes), settings, stats
  render.rs      pure threaded thread + form HTML (the XSS boundary) + unit tests
  antispam.rs    honeypot / time-trap / rate-limit / captcha / Akismet + unit tests
  email.rs       send via the internal backend
  database.rs    CMS reads, comment CRUD, moderation, threads, likes, erase, settings
  api/
    public.rs    comments helper + /comments/submit + /comments/react
    admin.rs     list / moderate / erase / stats
    settings.rs  get / update settings
    dashboard.rs pending + recent cards
templates/       minijinja dashboard-card fragments
ui/dist/         static moderation panel
migrations/      plugin schema (comment, comment_settings)
```

## Roadmap (nice-to-haves beyond P1–P3)

- **Public self-service GDPR** — an email-verified "delete my comments" flow for
  visitors (the current erase is operator-actioned via the admin).
- **Per-comment reply captcha** — replies currently skip the captcha; add it (a
  single shared widget via JS) if abuse via replies shows up.
- **Reaction robustness** — likes dedup on a best-effort IP; a signed cookie
  would harden anonymous dedup behind the proxy.
- **Real client IP** — if Neleto ever forwards `X-Forwarded-For` to plugins, the
  per-IP rate limit and like dedup become reliable; revisit then.
- **Cookiebanner consent** — wire email/IP retention into the existing consent
  plugin.
```

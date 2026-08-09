# Development

Developer notes for the `blog-comments` Neleto plugin. For the user-facing
description, see [readme.md](readme.md).

> Status: **P1 + part of P2** of the [plugin roadmap](../plugin-cli/docs/plugin-roadmap.md).
> P1: flat comments, submit page, moderation UI, captcha, render helper. P2 so
> far: dashboard cards. Not yet done (P2/P3): threaded replies, email
> notifications, Akismet, GDPR delete-my-data, reactions.

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

## XSS

Comment text is escaped at render time (`utils::escape_html` /
`escape_multiline`, the latter turning newlines into `<br>`). Stored bodies are
raw text and never interpreted as HTML — the escaping boundary lives entirely in
`render.rs`.

## Manifest hooks

| Hook | Route | Purpose |
|---|---|---|
| `helpers` | `/helper/comments` | `{{ comments(post.id) }}` — renders thread + form |
| `pages` | `/comments/submit` | Public form POST (no layout), redirects back |
| `dashboard_cards` | `/dashboard/pending`, `/dashboard/recent` | Moderation queue size + latest approved |
| `api` | `/api/comments`, `/api/comments/moderate`, `/api/stats` | Moderation list + actions + counts |
| `api` | `/api/settings` | Moderation/email/captcha settings |
| `ui` | `/ui` | Moderation panel |

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
  model.rs       CMS post row, comment rows, settings, stats
  render.rs      pure thread + form HTML (the XSS boundary) + unit tests
  antispam.rs    honeypot / time-trap / rate-limit / captcha + unit tests
  database.rs    CMS reads, comment CRUD, moderation, stats, settings
  api/
    public.rs    comments helper + /comments/submit handler
    admin.rs     list / moderate / stats
    settings.rs  get / update settings
    dashboard.rs pending + recent cards
templates/       minijinja dashboard-card fragments
ui/dist/         static moderation panel
migrations/      plugin schema (comment, comment_settings)
```

## Roadmap (next)

- **P2:** threaded replies (`parent_id` is already in the schema; render + reply
  forms are the work) and email notifications to the author/admin on new
  comments (reuse form-plugin's email sending).
- **P3:** Akismet (or similar) spam scoring, GDPR "delete my data" by email,
  reactions/upvotes. Wire email/IP retention into the cookiebanner consent.
- **Real client IP** — if Neleto ever forwards `X-Forwarded-For` to plugins, the
  per-IP rate limit becomes reliable; revisit then.
```

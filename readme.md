# Blog Comments

Let your readers talk back. **Blog Comments** adds a clean, self-hosted comment
section to any blog post on your Neleto site — no Disqus, no third-party scripts
loading your visitors' data into someone else's cloud. Comments live in your own
database, and you decide what gets published.

Drop one helper into your blog-post template and a comment thread + form appears.
New comments wait in a moderation queue until you approve them, and two dashboard
cards keep you on top of what needs attention.

## Why you'll want it

- **Build a community.** Give readers a place to react, ask, and discuss right
  under your posts — with **threaded replies** and **likes**.
- **You're in control.** Every comment waits for your approval by default —
  nothing appears until you say so. Approve, mark spam, or delete in one click.
- **Spam-resistant out of the box.** A hidden honeypot field and a submit
  time-trap stop naive bots; switch on **hCaptcha**/**reCAPTCHA** for a real
  challenge, and add an **Akismet** key for automatic spam filtering — no coding.
- **Stay in the loop.** Get an **email** whenever a new comment needs review, and
  your commenters get notified when someone replies to them.
- **Self-hosted & private.** Comments never leave your server. No external
  tracking scripts on your pages.
- **Privacy-ready.** One-click **erase or anonymize** all comments for an email
  address, so you can honour data-deletion requests.
- **At a glance.** Dashboard cards show how many comments await review and your
  latest approved ones.
- **Works without JavaScript.** The comment form is a plain HTML form — it posts,
  replies, and even likes work with scripts disabled (they just get smoother with
  JS on).

## How to use it

1. **Install** Blog Comments from the plugin store.
2. Add the comment section to your **blog-post template** by dropping in the
   helper:

   ```hbs
   {{ comments(post.id) }}
   ```

   Put it wherever you want the thread to appear — usually below the article.
3. Open the **Blog Comments** admin panel to moderate. New comments land in
   **Pending**; approve the good ones and they appear on the post.
4. (Optional) In **Settings**, turn on a captcha, decide whether to ask for
   commenter emails, or allow comments to post without moderation.

## Moderation

Every comment shows up in the admin panel with the post it belongs to. Filter by
**Pending**, **Approved**, or **Spam**, and for each comment:

- **Approve** — publish it on the post.
- **Spam** — hide it and keep it out of the way.
- **Unpublish** — send an approved comment back to pending.
- **Delete** — remove it for good.

## Who can use it

- **Admins & developers** can moderate and change settings.
- **Editors** can moderate comments on posts.

## Good to know

- **Safe by design.** Comment text is always shown as plain text — visitors can't
  sneak in HTML or scripts.
- **Privacy.** Email is optional and never shown publicly. If you collect emails
  or run a comment section, mention it in your privacy policy and cookie/consent
  notice as appropriate.
- **Matches your theme.** The admin panel and dashboard cards follow your
  dashboard's colors and dark mode.

---

Building on or contributing to the plugin? See [DEVELOPMENT.md](DEVELOPMENT.md).

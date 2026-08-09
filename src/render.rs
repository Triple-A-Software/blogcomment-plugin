//! Pure HTML rendering for the public comment thread + submit form. Everything
//! visitor-supplied is escaped here (`escape_html` / `escape_multiline`), so
//! stored comment text can never inject markup — the XSS boundary lives in this
//! module.

use crate::{
    model::{CommentPublic, CommentSettings},
    utils::{escape_html, escape_multiline},
};

/// Render the whole `#comments` block: an optional status banner, the thread,
/// and the submit form. `path` is the current page path, submitted as the
/// redirect target so the browser returns here after posting.
pub fn thread_and_form(
    post_id: i32,
    path: &str,
    comments: &[CommentPublic],
    settings: &CommentSettings,
    flag: Option<&str>,
) -> String {
    let mut out = String::new();
    out.push_str(STYLE);
    out.push_str(r#"<section id="comments" class="bc">"#);

    if let Some(banner) = flag.and_then(status_banner) {
        out.push_str(&banner);
    }

    let count = comments.len();
    let noun = if count == 1 { "Kommentar" } else { "Kommentare" };
    out.push_str(&format!(r#"<h2 class="bc-heading">{count} {noun}</h2>"#));

    if comments.is_empty() {
        out.push_str(r#"<p class="bc-empty">Seien Sie der Erste, der kommentiert.</p>"#);
    } else {
        out.push_str(r#"<ol class="bc-list">"#);
        for c in comments {
            out.push_str(&format!(
                r#"<li class="bc-item"><div class="bc-meta"><span class="bc-author">{author}</span><time class="bc-date">{date}</time></div><div class="bc-body">{body}</div></li>"#,
                author = escape_html(&c.author_name),
                date = c.created_at.format("%d.%m.%Y %H:%M"),
                body = escape_multiline(&c.body),
            ));
        }
        out.push_str("</ol>");
    }

    out.push_str(&form(post_id, path, settings));
    out.push_str("</section>");
    out
}

fn form(post_id: i32, path: &str, settings: &CommentSettings) -> String {
    let mut f = String::new();
    f.push_str(r#"<form class="bc-form" method="post" action="/comments/submit">"#);
    f.push_str(&format!(r#"<input type="hidden" name="post_id" value="{post_id}">"#));
    f.push_str(&format!(
        r#"<input type="hidden" name="redirect" value="{}">"#,
        escape_html(path)
    ));
    f.push_str(r#"<input type="hidden" name="ts" class="bc-ts" value="">"#);
    // Honeypot: hidden from humans, tempting to bots. A filled value = spam.
    f.push_str(
        r#"<div class="bc-hp" aria-hidden="true"><label>Website<input type="text" name="website" tabindex="-1" autocomplete="off"></label></div>"#,
    );

    f.push_str(
        r#"<div class="bc-field"><label for="bc-name">Name</label><input id="bc-name" name="author_name" type="text" required maxlength="120"></div>"#,
    );
    if settings.collect_email {
        f.push_str(
            r#"<div class="bc-field"><label for="bc-email">E-Mail (wird nicht veröffentlicht)</label><input id="bc-email" name="author_email" type="email" maxlength="254"></div>"#,
        );
    }
    f.push_str(
        r#"<div class="bc-field"><label for="bc-body">Kommentar</label><textarea id="bc-body" name="body" required rows="4" maxlength="5000"></textarea></div>"#,
    );

    f.push_str(&captcha_widget(settings));
    f.push_str(r#"<button class="bc-submit" type="submit">Kommentar absenden</button></form>"#);
    // Stamp the load time for the submit time-trap (skipped when JS is off).
    f.push_str(r#"<script>document.querySelectorAll('input.bc-ts').forEach(function(el){el.value=Date.now();});</script>"#);
    f
}

fn captcha_widget(settings: &CommentSettings) -> String {
    let Some(site_key) = settings.captcha_site_key.as_deref().filter(|s| !s.is_empty()) else {
        return String::new();
    };
    let key = escape_html(site_key);
    match settings.captcha_provider.as_str() {
        "hcaptcha" => format!(
            r#"<div class="h-captcha" data-sitekey="{key}"></div><script src="https://hcaptcha.com/1/api.js" async defer></script>"#
        ),
        "recaptcha" => format!(
            r#"<div class="g-recaptcha" data-sitekey="{key}"></div><script src="https://www.google.com/recaptcha/api.js" async defer></script>"#
        ),
        _ => String::new(),
    }
}

fn status_banner(flag: &str) -> Option<String> {
    let (kind, msg) = match flag {
        "received" => ("ok", "Danke! Ihr Kommentar wird nach einer kurzen Prüfung veröffentlicht."),
        "posted" => ("ok", "Danke für Ihren Kommentar!"),
        "captcha" => ("err", "Bitte bestätigen Sie, dass Sie kein Roboter sind, und senden Sie erneut."),
        "slow_down" => ("err", "Zu viele Kommentare in kurzer Zeit. Bitte versuchen Sie es gleich noch einmal."),
        "error" => ("err", "Ihr Kommentar konnte nicht gespeichert werden. Bitte prüfen Sie Ihre Eingaben."),
        _ => return None,
    };
    Some(format!(r#"<p class="bc-banner bc-banner--{kind}">{msg}</p>"#))
}

const STYLE: &str = r#"<style>
.bc { max-width: 720px; margin: 32px auto; font-family: system-ui, -apple-system, sans-serif; }
.bc-heading { font-size: 20px; margin: 0 0 16px; }
.bc-empty { color: #6b7280; }
.bc-banner { padding: 10px 14px; border-radius: 8px; font-size: 14px; margin: 0 0 16px; }
.bc-banner--ok { background: #dcfce7; color: #166534; }
.bc-banner--err { background: #fee2e2; color: #991b1b; }
.bc-list { list-style: none; margin: 0 0 28px; padding: 0; display: flex; flex-direction: column; gap: 18px; }
.bc-item { border-bottom: 1px solid #e5e7eb; padding-bottom: 14px; }
.bc-meta { display: flex; align-items: baseline; gap: 10px; margin-bottom: 4px; }
.bc-author { font-weight: 600; }
.bc-date { color: #9ca3af; font-size: 12px; }
.bc-body { color: #374151; line-height: 1.55; word-wrap: break-word; overflow-wrap: anywhere; }
.bc-form { display: flex; flex-direction: column; gap: 12px; }
.bc-field { display: flex; flex-direction: column; gap: 4px; }
.bc-field label { font-size: 13px; font-weight: 600; }
.bc-field input, .bc-field textarea { padding: 9px 11px; border: 1px solid #d1d5db; border-radius: 8px; font: inherit; }
.bc-field textarea { resize: vertical; }
.bc-hp { position: absolute; left: -9999px; width: 1px; height: 1px; overflow: hidden; }
.bc-submit { align-self: flex-start; padding: 10px 18px; border: 0; border-radius: 8px; background: #111827; color: #fff; font: inherit; font-weight: 600; cursor: pointer; }
</style>"#;

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    fn comment(name: &str, body: &str) -> CommentPublic {
        CommentPublic {
            id: 1,
            author_name: name.into(),
            body: body.into(),
            created_at: Utc.with_ymd_and_hms(2026, 8, 9, 12, 0, 0).unwrap(),
        }
    }

    #[test]
    fn escapes_author_and_body() {
        let out = thread_and_form(
            7,
            "/blog/post",
            &[comment("<script>evil</script>", "line1\nline2 <b>x</b>")],
            &CommentSettings::default(),
            None,
        );
        assert!(out.contains("&lt;script&gt;evil"));
        assert!(!out.contains("<script>evil"));
        assert!(out.contains("line1<br>line2 &lt;b&gt;x&lt;/b&gt;"));
    }

    #[test]
    fn form_carries_post_id_and_redirect_and_honeypot() {
        let out = thread_and_form(42, "/blog/hi", &[], &CommentSettings::default(), None);
        assert!(out.contains(r#"name="post_id" value="42""#));
        assert!(out.contains(r#"name="redirect" value="/blog/hi""#));
        assert!(out.contains(r#"name="website""#));
        assert!(out.contains("Seien Sie der Erste"));
    }

    #[test]
    fn email_field_toggles_with_setting() {
        let mut s = CommentSettings::default();
        assert!(thread_and_form(1, "/", &[], &s, None).contains(r#"name="author_email""#));
        s.collect_email = false;
        assert!(!thread_and_form(1, "/", &[], &s, None).contains(r#"name="author_email""#));
    }

    #[test]
    fn captcha_widget_only_when_configured() {
        let mut s = CommentSettings::default();
        assert!(!thread_and_form(1, "/", &[], &s, None).contains("h-captcha"));
        s.captcha_provider = "hcaptcha".into();
        s.captcha_site_key = Some("KEY123".into());
        assert!(thread_and_form(1, "/", &[], &s, None).contains(r#"data-sitekey="KEY123""#));
    }

    #[test]
    fn banner_shown_for_flag() {
        let out = thread_and_form(1, "/", &[], &CommentSettings::default(), Some("received"));
        assert!(out.contains("bc-banner--ok"));
        assert!(out.contains("nach einer kurzen Prüfung"));
    }
}

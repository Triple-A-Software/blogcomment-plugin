//! Pure HTML rendering for the public comment thread + submit form. Everything
//! visitor-supplied is escaped here (`escape_html` / `escape_multiline`), so
//! stored comment text can never inject markup — the XSS boundary lives in this
//! module.

use std::collections::HashMap;

use crate::{
    model::{CommentPublic, CommentSettings},
    utils::{escape_html, escape_multiline},
};

/// How deep replies are visually indented; deeper replies keep nesting in the
/// markup but stop indenting further.
const MAX_DEPTH: usize = 4;

/// Render the whole `#comments` block: an optional status banner, the threaded
/// comment tree, and the submit form. `path` is the current page path,
/// submitted as the redirect target so the browser returns here after posting.
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
        // Group comment indices by parent so we can render the reply tree.
        let mut children: HashMap<Option<i64>, Vec<&CommentPublic>> = HashMap::new();
        for c in comments {
            children.entry(c.parent_id).or_default().push(c);
        }
        render_nodes(&mut out, &children, None, 0, post_id, path, settings);
    }

    // New top-level comment form (carries the captcha, if any).
    out.push_str(r#"<h3 class="bc-form-title">Kommentar schreiben</h3>"#);
    out.push_str(&comment_form(post_id, path, settings, None, true));
    out.push_str("</section>");
    out.push_str(SCRIPT);
    out
}

#[allow(clippy::too_many_arguments)]
fn render_nodes(
    out: &mut String,
    children: &HashMap<Option<i64>, Vec<&CommentPublic>>,
    parent: Option<i64>,
    depth: usize,
    post_id: i32,
    path: &str,
    settings: &CommentSettings,
) {
    let Some(nodes) = children.get(&parent) else {
        return;
    };
    out.push_str(&format!(r#"<ol class="bc-list bc-depth-{}">"#, depth.min(MAX_DEPTH)));
    for c in nodes {
        out.push_str(&format!(r#"<li class="bc-item" id="comment-{}">"#, c.id));
        out.push_str(&format!(
            r#"<div class="bc-meta"><span class="bc-author">{author}</span><time class="bc-date">{date}</time></div><div class="bc-body">{body}</div>"#,
            author = escape_html(&c.author_name),
            date = c.created_at.format("%d.%m.%Y %H:%M"),
            body = escape_multiline(&c.body),
        ));
        // Actions: like + reply.
        out.push_str(r#"<div class="bc-actions">"#);
        out.push_str(&format!(
            r#"<form class="bc-like" method="post" action="/comments/react"><input type="hidden" name="comment_id" value="{id}"><input type="hidden" name="redirect" value="{path}"><button class="bc-like-btn" type="submit">👍 <span id="likes-{id}">{likes}</span></button></form>"#,
            id = c.id,
            path = escape_html(path),
            likes = c.likes,
        ));
        out.push_str(r#"<details class="bc-reply"><summary>Antworten</summary>"#);
        out.push_str(&comment_form(post_id, path, settings, Some(c.id), false));
        out.push_str("</details></div>");

        // Replies.
        render_nodes(out, children, Some(c.id), depth + 1, post_id, path, settings);
        out.push_str("</li>");
    }
    out.push_str("</ol>");
}

fn comment_form(
    post_id: i32,
    path: &str,
    settings: &CommentSettings,
    parent_id: Option<i64>,
    with_captcha: bool,
) -> String {
    let mut f = String::new();
    f.push_str(r#"<form class="bc-form" method="post" action="/comments/submit">"#);
    f.push_str(&format!(r#"<input type="hidden" name="post_id" value="{post_id}">"#));
    if let Some(parent_id) = parent_id {
        f.push_str(&format!(r#"<input type="hidden" name="parent_id" value="{parent_id}">"#));
    }
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
        r#"<div class="bc-field"><label>Name<input name="author_name" type="text" required maxlength="120"></label></div>"#,
    );
    if settings.collect_email {
        f.push_str(
            r#"<div class="bc-field"><label>E-Mail (wird nicht veröffentlicht)<input name="author_email" type="email" maxlength="254"></label></div>"#,
        );
    }
    f.push_str(
        r#"<div class="bc-field"><label>Kommentar<textarea name="body" required rows="4" maxlength="5000"></textarea></label></div>"#,
    );

    if with_captcha {
        f.push_str(&captcha_widget(settings));
    }
    f.push_str(r#"<button class="bc-submit" type="submit">Absenden</button></form>"#);
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
.bc-form-title { font-size: 16px; margin: 28px 0 10px; }
.bc-empty { color: #6b7280; }
.bc-banner { padding: 10px 14px; border-radius: 8px; font-size: 14px; margin: 0 0 16px; }
.bc-banner--ok { background: #dcfce7; color: #166534; }
.bc-banner--err { background: #fee2e2; color: #991b1b; }
.bc-list { list-style: none; margin: 0 0 8px; padding: 0; display: flex; flex-direction: column; gap: 16px; }
.bc-list.bc-depth-1, .bc-list.bc-depth-2, .bc-list.bc-depth-3, .bc-list.bc-depth-4 { margin-top: 14px; padding-left: 20px; border-left: 2px solid #e5e7eb; }
.bc-item { }
.bc-meta { display: flex; align-items: baseline; gap: 10px; margin-bottom: 4px; }
.bc-author { font-weight: 600; }
.bc-date { color: #9ca3af; font-size: 12px; }
.bc-body { color: #374151; line-height: 1.55; word-wrap: break-word; overflow-wrap: anywhere; }
.bc-actions { display: flex; align-items: center; gap: 14px; margin-top: 6px; }
.bc-like { margin: 0; }
.bc-like-btn { background: none; border: 0; color: #6b7280; cursor: pointer; font: inherit; font-size: 13px; padding: 0; }
.bc-like-btn:disabled { color: #16a34a; cursor: default; }
.bc-reply { font-size: 13px; }
.bc-reply > summary { cursor: pointer; color: #6b7280; }
.bc-reply .bc-form { margin-top: 10px; }
.bc-form { display: flex; flex-direction: column; gap: 12px; }
.bc-field label { display: flex; flex-direction: column; gap: 4px; font-size: 13px; font-weight: 600; }
.bc-field input, .bc-field textarea { padding: 9px 11px; border: 1px solid #d1d5db; border-radius: 8px; font: inherit; font-weight: 400; }
.bc-field textarea { resize: vertical; }
.bc-hp { position: absolute; left: -9999px; width: 1px; height: 1px; overflow: hidden; }
.bc-submit { align-self: flex-start; padding: 10px 18px; border: 0; border-radius: 8px; background: #111827; color: #fff; font: inherit; font-weight: 600; cursor: pointer; }
</style>"#;

const SCRIPT: &str = r#"<script>
(function(){
  document.querySelectorAll('input.bc-ts').forEach(function(el){ el.value = Date.now(); });
  document.querySelectorAll('form.bc-like').forEach(function(f){
    f.addEventListener('submit', function(e){
      e.preventDefault();
      var id = f.querySelector('input[name=comment_id]').value;
      fetch('/comments/react', {
        method: 'POST',
        headers: { 'Accept': 'application/json', 'Content-Type': 'application/x-www-form-urlencoded' },
        body: new URLSearchParams(new FormData(f)).toString()
      }).then(function(r){ return r.ok ? r.json() : null; }).then(function(d){
        if (d && typeof d.likes === 'number') {
          var s = document.getElementById('likes-' + id);
          if (s) s.textContent = d.likes;
          var b = f.querySelector('button'); if (b) b.disabled = true;
        }
      }).catch(function(){});
    });
  });
})();
</script>"#;

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    fn comment(id: i64, parent: Option<i64>, name: &str, body: &str) -> CommentPublic {
        CommentPublic {
            id,
            parent_id: parent,
            author_name: name.into(),
            body: body.into(),
            created_at: Utc.with_ymd_and_hms(2026, 8, 9, 12, 0, 0).unwrap(),
            likes: 0,
        }
    }

    #[test]
    fn escapes_author_and_body() {
        let out = thread_and_form(
            7,
            "/blog/post",
            &[comment(1, None, "<script>evil</script>", "line1\nline2 <b>x</b>")],
            &CommentSettings::default(),
            None,
        );
        assert!(out.contains("&lt;script&gt;evil"));
        assert!(!out.contains("<script>evil"));
        assert!(out.contains("line1<br>line2 &lt;b&gt;x&lt;/b&gt;"));
    }

    #[test]
    fn top_form_carries_post_id_and_redirect_and_honeypot() {
        let out = thread_and_form(42, "/blog/hi", &[], &CommentSettings::default(), None);
        assert!(out.contains(r#"name="post_id" value="42""#));
        assert!(out.contains(r#"name="redirect" value="/blog/hi""#));
        assert!(out.contains(r#"name="website""#));
    }

    #[test]
    fn renders_replies_nested_with_parent_id() {
        let out = thread_and_form(
            5,
            "/p",
            &[
                comment(1, None, "Ann", "top"),
                comment(2, Some(1), "Bob", "reply"),
            ],
            &CommentSettings::default(),
            None,
        );
        assert!(out.contains(r#"id="comment-1""#));
        assert!(out.contains(r#"id="comment-2""#));
        assert!(out.contains("bc-depth-1")); // reply is indented one level
        assert!(out.contains(r#"name="parent_id" value="1""#)); // reply form targets parent
    }

    #[test]
    fn like_button_shows_count() {
        let mut c = comment(9, None, "Ann", "hi");
        c.likes = 3;
        let out = thread_and_form(1, "/", &[c], &CommentSettings::default(), None);
        assert!(out.contains(r#"id="likes-9""#));
        assert!(out.contains(">3</span>"));
        assert!(out.contains(r#"action="/comments/react""#));
    }

    #[test]
    fn email_field_toggles_with_setting() {
        let mut s = CommentSettings::default();
        assert!(thread_and_form(1, "/", &[], &s, None).contains(r#"name="author_email""#));
        s.collect_email = false;
        assert!(!thread_and_form(1, "/", &[], &s, None).contains(r#"name="author_email""#));
    }

    #[test]
    fn captcha_only_on_top_form_when_configured() {
        let s = CommentSettings {
            captcha_provider: "hcaptcha".into(),
            captcha_site_key: Some("KEY123".into()),
            ..CommentSettings::default()
        };
        // A thread with one comment: top form has the widget, the reply form does not.
        let out = thread_and_form(1, "/", &[comment(1, None, "A", "b")], &s, None);
        assert_eq!(out.matches("h-captcha").count(), 1);
    }
}

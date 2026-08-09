//! Pure HTML rendering for the public comment thread + submit form. Everything
//! visitor-supplied is escaped here (`escape_html` / `escape_multiline`), so
//! stored comment text can never inject markup — the XSS boundary lives in this
//! module.

use std::collections::HashMap;

use crate::{
    lang::Lang,
    model::{CommentPublic, CommentSettings},
    utils::{escape_html, escape_multiline},
};

/// How deep replies are visually indented; deeper replies keep nesting in the
/// markup but stop indenting further.
const MAX_DEPTH: usize = 4;

/// Render the whole `#comments` block: the threaded comment tree and the submit
/// form, in the page's language. `path` is the current page path, submitted as
/// the redirect target so the browser returns here after posting.
///
/// The post-submit confirmation banner is *not* rendered here — it's injected
/// client-side from the `?comment=<flag>` query param, because Neleto's page
/// cache key ignores the query string (a server-rendered banner would be cached
/// and shown to the wrong visitors, or missed entirely).
pub fn thread_and_form(
    post_id: i32,
    path: &str,
    comments: &[CommentPublic],
    settings: &CommentSettings,
    lang: Lang,
) -> String {
    let mut out = String::new();
    out.push_str(STYLE);
    out.push_str(r#"<section id="comments" class="bc">"#);

    let count = comments.len();
    out.push_str(&format!(r#"<h2 class="bc-heading">{}</h2>"#, lang.heading(count)));

    if comments.is_empty() {
        out.push_str(&format!(r#"<p class="bc-empty">{}</p>"#, lang.be_first()));
    } else {
        // Group comment indices by parent so we can render the reply tree.
        let mut children: HashMap<Option<i64>, Vec<&CommentPublic>> = HashMap::new();
        for c in comments {
            children.entry(c.parent_id).or_default().push(c);
        }
        render_nodes(&mut out, &children, None, 0, post_id, path, settings, lang);
    }

    // New top-level comment form (carries the captcha, if any).
    out.push_str(&format!(r#"<h3 class="bc-form-title">{}</h3>"#, lang.write_comment()));
    out.push_str(&comment_form(post_id, path, settings, None, true, lang));
    out.push_str("</section>");
    out.push_str(&script(lang));
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
    lang: Lang,
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
            date = c.created_at.format(lang.date_fmt()),
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
        out.push_str(&format!(r#"<details class="bc-reply"><summary>{}</summary>"#, lang.reply()));
        out.push_str(&comment_form(post_id, path, settings, Some(c.id), false, lang));
        out.push_str("</details></div>");

        // Replies.
        render_nodes(out, children, Some(c.id), depth + 1, post_id, path, settings, lang);
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
    lang: Lang,
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

    f.push_str(&format!(
        r#"<div class="bc-field"><label>{name}<input name="author_name" type="text" required maxlength="120"></label></div>"#,
        name = lang.name(),
    ));
    if settings.collect_email {
        f.push_str(&format!(
            r#"<div class="bc-field"><label>{email}<input name="author_email" type="email" maxlength="254"></label></div>"#,
            email = lang.email(),
        ));
    }
    f.push_str(&format!(
        r#"<div class="bc-field"><label>{comment}<textarea name="body" required rows="4" maxlength="5000"></textarea></label></div>"#,
        comment = lang.comment(),
    ));

    if with_captcha {
        f.push_str(&captcha_widget(settings));
    }
    f.push_str(&format!(
        r#"<button class="bc-submit" type="submit">{}</button></form>"#,
        lang.submit()
    ));
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

/// The inline behaviour script for the comment block, localized for `lang`.
///
/// Rendered with `inline="true"` so Neleto's HTML rewriter leaves it in place
/// instead of hoisting it into `<head>` — but it's *also* written to work if it
/// is hoisted: the like handler is bound via event delegation on `document`
/// (which always exists), so a click is intercepted rather than falling back to
/// a native form submit that reloads the page. DOM-dependent setup waits for
/// `DOMContentLoaded`.
///
/// It also injects the post-submit confirmation banner from `?comment=<flag>`
/// on the client, since the server can't (Neleto caches the page ignoring the
/// query string). The localized flag→message table is emitted as `B`.
fn script(lang: Lang) -> String {
    let banners: serde_json::Map<String, serde_json::Value> = lang
        .banners()
        .iter()
        .map(|(flag, kind, msg)| (flag.to_string(), serde_json::json!([kind, msg])))
        .collect();
    let banners = serde_json::Value::Object(banners).to_string();
    SCRIPT_TEMPLATE.replace("/*BANNERS*/null", &banners)
}

const SCRIPT_TEMPLATE: &str = r#"<script inline="true">
(function(){
  var B = /*BANNERS*/null;
  function ready(fn){ if (document.readyState !== 'loading') fn(); else document.addEventListener('DOMContentLoaded', fn); }
  ready(function(){
    document.querySelectorAll('input.bc-ts').forEach(function(el){ el.value = Date.now(); });
    // Post-submit confirmation banner, from the ?comment=<flag> the submit redirect adds.
    try {
      var params = new URLSearchParams(location.search);
      var flag = params.get('comment');
      var sec = document.getElementById('comments');
      if (flag && B[flag] && sec) {
        var p = document.createElement('p');
        p.className = 'bc-banner bc-banner--' + B[flag][0];
        p.textContent = B[flag][1];
        sec.insertBefore(p, sec.firstChild);
        // Drop the flag so a refresh doesn't re-show the banner.
        params.delete('comment');
        var qs = params.toString();
        history.replaceState(null, '', location.pathname + (qs ? '?' + qs : '') + '#comments');
      }
    } catch (e) {}
  });
  // Like via delegation on document — survives this script being hoisted to <head>.
  document.addEventListener('submit', function(e){
    var f = e.target;
    if (!(f instanceof HTMLFormElement) || !f.classList.contains('bc-like')) return;
    e.preventDefault();
    var idEl = f.querySelector('input[name=comment_id]');
    if (!idEl) return;
    var id = idEl.value;
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
            Lang::De,
        );
        assert!(out.contains("&lt;script&gt;evil"));
        assert!(!out.contains("<script>evil"));
        assert!(out.contains("line1<br>line2 &lt;b&gt;x&lt;/b&gt;"));
    }

    #[test]
    fn top_form_carries_post_id_and_redirect_and_honeypot() {
        let out = thread_and_form(42, "/blog/hi", &[], &CommentSettings::default(), Lang::De);
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
            Lang::De,
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
        let out = thread_and_form(1, "/", &[c], &CommentSettings::default(), Lang::De);
        assert!(out.contains(r#"id="likes-9""#));
        assert!(out.contains(">3</span>"));
        assert!(out.contains(r#"action="/comments/react""#));
    }

    #[test]
    fn email_field_toggles_with_setting() {
        let mut s = CommentSettings::default();
        assert!(thread_and_form(1, "/", &[], &s, Lang::De).contains(r#"name="author_email""#));
        s.collect_email = false;
        assert!(!thread_and_form(1, "/", &[], &s, Lang::De).contains(r#"name="author_email""#));
    }

    #[test]
    fn captcha_only_on_top_form_when_configured() {
        let s = CommentSettings {
            captcha_provider: "hcaptcha".into(),
            captcha_site_key: Some("KEY123".into()),
            ..CommentSettings::default()
        };
        // A thread with one comment: top form has the widget, the reply form does not.
        let out = thread_and_form(1, "/", &[comment(1, None, "A", "b")], &s, Lang::De);
        assert_eq!(out.matches("h-captcha").count(), 1);
    }

    #[test]
    fn renders_in_the_requested_language() {
        let de = thread_and_form(1, "/", &[], &CommentSettings::default(), Lang::De);
        assert!(de.contains("Kommentare"));
        assert!(de.contains("Seien Sie der Erste"));
        assert!(de.contains(">Absenden</button>"));

        let en = thread_and_form(1, "/", &[], &CommentSettings::default(), Lang::En);
        assert!(en.contains("Comments"));
        assert!(en.contains("Be the first to comment."));
        assert!(en.contains(">Submit</button>"));
        assert!(!en.contains("Kommentare"));
    }

    #[test]
    fn heading_is_singular_for_one_comment() {
        let out = thread_and_form(1, "/", &[comment(1, None, "A", "b")], &CommentSettings::default(), Lang::En);
        assert!(out.contains(">1 Comment</h2>"));
    }

    #[test]
    fn script_is_inline_delegated_and_carries_localized_banners() {
        let out = thread_and_form(1, "/", &[], &CommentSettings::default(), Lang::En);
        // inline="true" keeps Neleto from hoisting the script into <head>...
        assert!(out.contains(r#"<script inline="true">"#));
        // ...and delegation means a hoisted script still catches the like submit.
        assert!(out.contains("document.addEventListener('submit'"));
        // Localized confirmation-banner table is embedded for the client to render.
        assert!(out.contains("Your comment will be published"));
        let de = thread_and_form(1, "/", &[], &CommentSettings::default(), Lang::De);
        assert!(de.contains("kurzen Prüfung veröffentlicht"));
    }
}

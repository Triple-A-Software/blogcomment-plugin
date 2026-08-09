//! Spam defences, layered so the plugin is safe out of the box: a honeypot
//! field and a submit time-trap catch naive bots, moderation-by-default keeps
//! anything unverified off the page, and an optional hCaptcha/reCaptcha adds a
//! real challenge when the operator configures one.

use std::time::{SystemTime, UNIX_EPOCH};

use axum::http::HeaderMap;
use serde::Deserialize;

use crate::model::CommentSettings;

/// Best-effort real client IP. The Neleto proxy talks to us over localhost and
/// doesn't add `X-Forwarded-For` itself, but a reverse proxy in front of Neleto
/// usually does, and those headers are forwarded through — so read them when
/// present. Returns `None` when we can't identify the client (localhost/empty).
pub fn client_ip(headers: &HeaderMap) -> Option<String> {
    let raw = headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .or_else(|| headers.get("x-real-ip").and_then(|v| v.to_str().ok()))
        .map(str::trim)
        .filter(|s| !s.is_empty())?;
    if raw == "127.0.0.1" || raw == "::1" {
        return None;
    }
    Some(raw.to_string())
}

/// A bot filled the hidden honeypot field a human never sees.
pub fn honeypot_tripped(honeypot: Option<&str>) -> bool {
    honeypot.map(|s| !s.trim().is_empty()).unwrap_or(false)
}

/// The form was submitted implausibly fast. `ts` is a millisecond timestamp the
/// form stamps on load (client clock); missing/garbled values are treated as
/// fine so non-JS visitors are never blocked.
pub fn too_fast(ts: Option<&str>, min_secs: u64) -> bool {
    let Some(ts) = ts.and_then(|s| s.trim().parse::<u128>().ok()) else {
        return false;
    };
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    now_ms.saturating_sub(ts) < (min_secs as u128) * 1000
}

#[derive(Deserialize)]
struct CaptchaVerify {
    success: bool,
}

/// Verify the captcha response with the provider. Returns `true` when the
/// challenge passes — and also when no captcha is configured, or when the
/// operator enabled a provider but hasn't set a secret yet (a misconfiguration
/// shouldn't silently reject every visitor; those still face the honeypot and
/// moderation).
pub async fn verify_captcha(
    settings: &CommentSettings,
    hcaptcha: Option<&str>,
    recaptcha: Option<&str>,
) -> bool {
    let (url, response) = match settings.captcha_provider.as_str() {
        "hcaptcha" => ("https://api.hcaptcha.com/siteverify", hcaptcha),
        "recaptcha" => ("https://www.google.com/recaptcha/api/siteverify", recaptcha),
        _ => return true,
    };
    let Some(secret) = settings.captcha_secret.as_deref().filter(|s| !s.is_empty()) else {
        tracing::warn!("captcha provider set but no secret configured — skipping verification");
        return true;
    };
    let Some(response) = response.filter(|s| !s.is_empty()) else {
        return false;
    };
    let form = format!(
        "secret={}&response={}",
        urlencoding::encode(secret),
        urlencoding::encode(response)
    );
    match reqwest::Client::new()
        .post(url)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .header("Accept", "application/json")
        .body(form)
        .send()
        .await
    {
        Ok(res) => res.json::<CaptchaVerify>().await.map(|v| v.success).unwrap_or(false),
        Err(e) => {
            tracing::warn!("captcha verification request failed: {e}");
            false
        }
    }
}

/// Ask Akismet whether a comment is spam. `blog` is the site URL Akismet keys
/// on. Fails open (returns `false`) on any error — moderation still applies, so
/// a flaky Akismet never blocks legitimate comments.
#[allow(clippy::too_many_arguments)]
pub async fn akismet_is_spam(
    key: &str,
    blog: &str,
    user_ip: &str,
    user_agent: &str,
    author: &str,
    email: Option<&str>,
    content: &str,
) -> bool {
    let enc = urlencoding::encode;
    let mut form = format!(
        "blog={}&user_ip={}&user_agent={}&comment_type=comment&comment_author={}&comment_content={}",
        enc(blog),
        enc(user_ip),
        enc(user_agent),
        enc(author),
        enc(content),
    );
    if let Some(email) = email {
        form.push_str(&format!("&comment_author_email={}", enc(email)));
    }
    let url = format!("https://{key}.rest.akismet.com/1.1/comment-check");
    match reqwest::Client::new()
        .post(&url)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(form)
        .send()
        .await
    {
        Ok(res) => res.text().await.map(|t| t.trim() == "true").unwrap_or(false),
        Err(e) => {
            tracing::warn!("akismet check failed: {e}");
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn honeypot() {
        assert!(honeypot_tripped(Some("http://spam")));
        assert!(!honeypot_tripped(Some("")));
        assert!(!honeypot_tripped(Some("   ")));
        assert!(!honeypot_tripped(None));
    }

    #[test]
    fn time_trap_ignores_missing_or_bad_ts() {
        assert!(!too_fast(None, 2));
        assert!(!too_fast(Some("not-a-number"), 2));
    }

    #[test]
    fn time_trap_flags_instant_submit() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis()
            .to_string();
        assert!(too_fast(Some(&now), 2));
    }

    #[test]
    fn time_trap_passes_old_ts() {
        assert!(!too_fast(Some("1000"), 2)); // 1970 — long ago
    }
}

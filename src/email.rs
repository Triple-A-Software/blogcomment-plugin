//! Email notifications. Plugins don't hold SMTP credentials — they hand the
//! message to the Neleto backend's internal `/email/send` endpoint (the same
//! path form-plugin uses), which delivers it with the CMS's configured mailer.

use serde_json::json;

/// Send an HTML email via the internal backend. Best-effort and non-fatal:
/// callers spawn this so a mail failure never affects the comment flow. No-ops
/// (with a warning) when `INTERNAL_BACKEND_PORT` isn't set, e.g. in local dev.
pub async fn send(receiver: &str, subject: &str, html: &str) {
    let Ok(port) = std::env::var("INTERNAL_BACKEND_PORT") else {
        tracing::warn!("INTERNAL_BACKEND_PORT not set — skipping email to {receiver}");
        return;
    };
    let body = json!({
        "receiver": receiver,
        "subject": subject,
        "content": html,
        "attachments": null,
    });
    if let Err(e) = reqwest::Client::new()
        .post(format!("http://localhost:{port}/email/send"))
        .json(&body)
        .send()
        .await
    {
        tracing::warn!("failed to send email to {receiver}: {e}");
    }
}

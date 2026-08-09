//! Minimal i18n for the visitor-facing comment UI. The active language comes
//! from the CMS page (`page.language` in the inline-helper request), so the
//! thread renders in whatever language the surrounding page is in. Everything a
//! visitor sees is looked up here; German is the only non-English fallback.

/// The languages the public UI is translated into. Extend `from_code` and every
/// `match` below to add more.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    En,
    De,
}

impl Lang {
    /// Pick a language from a CMS page language code such as `"de"`, `"de-DE"`
    /// or `"en-US"`. Anything that isn't German falls back to English.
    pub fn from_code(code: Option<&str>) -> Self {
        match code {
            Some(c) if c.trim().to_ascii_lowercase().starts_with("de") => Lang::De,
            _ => Lang::En,
        }
    }

    /// The heading above the thread, e.g. `"3 Kommentare"` / `"1 Comment"`.
    pub fn heading(self, count: usize) -> String {
        match (self, count) {
            (Lang::De, 1) => "1 Kommentar".to_string(),
            (Lang::De, n) => format!("{n} Kommentare"),
            (Lang::En, 1) => "1 Comment".to_string(),
            (Lang::En, n) => format!("{n} Comments"),
        }
    }

    pub fn be_first(self) -> &'static str {
        match self {
            Lang::De => "Seien Sie der Erste, der kommentiert.",
            Lang::En => "Be the first to comment.",
        }
    }

    pub fn write_comment(self) -> &'static str {
        match self {
            Lang::De => "Kommentar schreiben",
            Lang::En => "Write a comment",
        }
    }

    pub fn reply(self) -> &'static str {
        match self {
            Lang::De => "Antworten",
            Lang::En => "Reply",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Lang::De => "Name",
            Lang::En => "Name",
        }
    }

    pub fn email(self) -> &'static str {
        match self {
            Lang::De => "E-Mail (wird nicht veröffentlicht)",
            Lang::En => "Email (not published)",
        }
    }

    pub fn comment(self) -> &'static str {
        match self {
            Lang::De => "Kommentar",
            Lang::En => "Comment",
        }
    }

    pub fn submit(self) -> &'static str {
        match self {
            Lang::De => "Absenden",
            Lang::En => "Submit",
        }
    }

    /// chrono strftime format for comment timestamps in this language.
    pub fn date_fmt(self) -> &'static str {
        match self {
            Lang::De => "%d.%m.%Y %H:%M",
            Lang::En => "%b %-d, %Y %H:%M",
        }
    }

    /// The `(kind, message)` for a post-submit status flag, where `kind` is
    /// `"ok"` or `"err"`. Returns `None` for an unknown flag. These flags are the
    /// values `submit` redirects back with (see `redirect_back`).
    pub fn banner(self, flag: &str) -> Option<(&'static str, &'static str)> {
        Some(match (self, flag) {
            (Lang::De, "received") => ("ok", "Danke! Ihr Kommentar wird nach einer kurzen Prüfung veröffentlicht."),
            (Lang::De, "posted") => ("ok", "Danke für Ihren Kommentar!"),
            (Lang::De, "captcha") => ("err", "Bitte bestätigen Sie, dass Sie kein Roboter sind, und senden Sie erneut."),
            (Lang::De, "slow_down") => ("err", "Zu viele Kommentare in kurzer Zeit. Bitte versuchen Sie es gleich noch einmal."),
            (Lang::De, "error") => ("err", "Ihr Kommentar konnte nicht gespeichert werden. Bitte prüfen Sie Ihre Eingaben."),
            (Lang::En, "received") => ("ok", "Thanks! Your comment will be published after a quick review."),
            (Lang::En, "posted") => ("ok", "Thanks for your comment!"),
            (Lang::En, "captcha") => ("err", "Please confirm that you are not a robot and submit again."),
            (Lang::En, "slow_down") => ("err", "Too many comments in a short time. Please try again in a moment."),
            (Lang::En, "error") => ("err", "Your comment could not be saved. Please check your input."),
            _ => return None,
        })
    }

    /// Every `(flag, kind, message)` triple for this language — used to emit the
    /// client-side banner lookup table (the banner is rendered in the browser
    /// from `?comment=<flag>`, because Neleto's page cache ignores query params).
    pub fn banners(self) -> [(&'static str, &'static str, &'static str); 5] {
        ["received", "posted", "captcha", "slow_down", "error"].map(|flag| {
            let (kind, msg) = self.banner(flag).unwrap();
            (flag, kind, msg)
        })
    }
}

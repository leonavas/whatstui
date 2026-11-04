//! The interface's language: English unless the config says
//! `language = pt-BR` (the settings dialog flips it). Every string the user
//! sees goes through `tr!(english, portuguese)`; both arms are expressions,
//! so `tr!(format!(..), format!(..))` works and only the chosen one is built.

use std::sync::atomic::{AtomicBool, Ordering};

/// The config key, and its value for Brazilian Portuguese.
pub const KEY: &str = "language";
pub const PT_BR: &str = "pt-BR";

static PT: AtomicBool = AtomicBool::new(false);

/// Reads the config once at start-up (after `bridge::migrate`).
pub fn init() {
    set_pt(crate::config::value(KEY).is_some_and(|v| is_pt(&v)));
}

pub fn is_pt(v: &str) -> bool {
    let v = v.to_ascii_lowercase();
    v == "pt-br" || v == "pt" || v == "pt_br"
}

pub fn pt() -> bool {
    PT.load(Ordering::Relaxed)
}

pub fn set_pt(on: bool) {
    PT.store(on, Ordering::Relaxed);
}

/// The language's code, as the Go helper gets it in `WHATSTUI_LANG`.
pub fn code() -> &'static str {
    if pt() { PT_BR } else { "en" }
}

/// `tr!(english, portuguese)`: the one for the language in use.
macro_rules! tr {
    ($en:expr, $pt:expr $(,)?) => {
        if $crate::i18n::pt() { $pt } else { $en }
    };
}

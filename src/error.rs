// SPDX-License-Identifier: AGPL-3.0-only WITH App-Fair-Distribution-Exception
//! Carry message constructors across worker threads; format only on the UI thread.
use crate::res;
#[derive(Clone, Debug)]
pub struct AppError {
    message: fn() -> day::LocalizedText,
    diagnostic: String,
}
impl AppError {
    pub fn new(message: fn() -> day::LocalizedText) -> Self {
        Self {
            message,
            diagnostic: String::new(),
        }
    }
    pub fn detail(message: fn() -> day::LocalizedText, detail: impl ToString) -> Self {
        Self {
            message,
            diagnostic: detail.to_string(),
        }
    }
    pub fn localized(&self) -> String {
        (self.message)().format()
    }
}
impl From<String> for AppError {
    fn from(detail: String) -> Self {
        Self::detail(res::str::operation_failed, detail)
    }
}
impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Diagnostic output only; UI uses localized(). No locale access on worker threads.
        write!(f, "Stanza operation failed: {}", self.diagnostic)
    }
}
impl std::error::Error for AppError {}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn worker_errors_use_the_ui_locale_when_presented() {
        day::install_locales(res::locales::DEFAULT, res::locales::CATALOG);
        let error = std::thread::spawn(|| AppError::new(res::str::epub_invalid))
            .join()
            .unwrap();
        day::prelude::set_locale("en");
        let english = error.localized();
        day::prelude::set_locale("fr");
        let french = error.localized();
        assert_ne!(english, french);
        assert_eq!(french, res::str::epub_invalid().format());
        day::prelude::set_locale("en");
    }
}

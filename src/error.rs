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
    /// Format the summary and retained diagnostics on the UI thread.
    pub fn explanation(&self) -> String {
        let summary = self.localized();
        if self.diagnostic.trim().is_empty() {
            summary
        } else {
            format!(
                "{summary}\n\n{}",
                res::str::error_details(self.diagnostic.as_str()).format()
            )
        }
    }
    pub(crate) fn reader(error: day_piece_webview::EvalError) -> Self {
        use day_piece_webview::EvalError;
        let code = match &error {
            EvalError::ViewGone => "reader.javascript.view_gone",
            EvalError::Unsupported => "reader.javascript.unsupported",
            EvalError::Threw { .. } => "reader.javascript.exception",
            EvalError::Engine(_) => "reader.javascript.invalid_reply",
        };
        Self::detail(res::str::reader_update_failed, format!("{code}\n{error}"))
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
        assert_eq!(error.explanation(), english);
        let detailed = AppError::reader(day_piece_webview::EvalError::ViewGone);
        let explanation = detailed.explanation();
        assert!(explanation.contains("reader.javascript.view_gone"));
        assert!(explanation.contains("the web view is gone"));
        assert!(explanation.starts_with(&res::str::reader_update_failed().format()));
        day::prelude::set_locale("fr");
        let french = error.localized();
        assert_ne!(english, french);
        assert_eq!(french, res::str::epub_invalid().format());
        day::prelude::set_locale("en");
    }
}

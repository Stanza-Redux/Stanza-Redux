// SPDX-License-Identifier: AGPL-3.0-only WITH App-Fair-Distribution-Exception
use crate::res;
use day_sync::{Error, StoreErrorKind};
use day_sync_webdav::webdav::{ConnectionStep, Error as DavError};
use std::{error::Error as _, time::Duration};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Stage {
    Address,
    Account,
    Probe(ConnectionStep),
    Keychain,
    Library,
    Sync,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Hint {
    General,
    Credentials,
    Capacity,
    MissingFolder,
    Network,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Failure {
    stage: Stage,
    hint: Hint,
    // Diagnostic codes and HTTP status only. URLs, headers, platform error strings,
    // server response bodies and database messages may all contain private data.
    code: String,
    http: Option<u16>,
    pub(super) retry_after: Option<Duration>,
}

impl Failure {
    pub(super) fn dav(stage: Stage, error: &DavError) -> Self {
        let mut failure = Self::new(stage, error.diagnostic_code());
        match error {
            DavError::Status {
                status,
                retry_after,
                ..
            } => {
                failure.http = Some(*status);
                failure.retry_after = *retry_after;
                failure.hint = match status {
                    401 | 403 => Hint::Credentials,
                    404 if stage == Stage::Probe(ConnectionStep::OpenCollection) => {
                        Hint::MissingFolder
                    }
                    507 => Hint::Capacity,
                    _ => Hint::General,
                };
            }
            DavError::Redirect { status, .. } => failure.http = Some(*status),
            DavError::Transport { .. } | DavError::Deferred(_) => failure.hint = Hint::Network,
            _ => {}
        }
        failure
    }

    pub(super) fn sync(stage: Stage, error: &Error) -> Self {
        match error {
            Error::Store(store) => {
                if let Some(dav) = store
                    .source()
                    .and_then(|source| source.downcast_ref::<DavError>())
                {
                    return Self::dav(stage, dav);
                }
                let mut failure = Self::new(stage, store.to_string());
                failure.retry_after = store.retry_after;
                failure.hint = match store.kind {
                    StoreErrorKind::Unauthorized | StoreErrorKind::Forbidden => Hint::Credentials,
                    StoreErrorKind::InsufficientStorage => Hint::Capacity,
                    StoreErrorKind::Unavailable => Hint::Network,
                    _ => Hint::General,
                };
                failure
            }
            Error::Database(db) => Self::new(stage, format!("sync.database.{:?}", db.kind)),
            _ => Self::new(stage, error.to_string()),
        }
    }

    pub(super) fn keychain() -> Self {
        Self::new(Stage::Keychain, "keychain.access".into())
    }

    fn new(stage: Stage, code: String) -> Self {
        Self {
            stage,
            hint: Hint::General,
            code,
            http: None,
            retry_after: None,
        }
    }

    pub(super) fn pauses_retry(&self) -> bool {
        matches!(self.hint, Hint::Credentials | Hint::Capacity)
    }

    pub(super) fn summary(&self) -> day::LocalizedText {
        match self.hint {
            Hint::MissingFolder => res::str::dav_missing_folder(),
            Hint::Credentials => res::str::dav_credentials(),
            Hint::Capacity => res::str::dav_capacity(),
            Hint::Network => match self.code.as_str() {
                "network.expensive" => res::str::dav_metered(),
                "network.constrained" => res::str::dav_data_saver(),
                "network.cellular" => res::str::dav_cellular(),
                _ => res::str::network_failed(),
            },
            Hint::General if self.stage == Stage::Address => res::str::dav_invalid_url(),
            Hint::General if self.stage == Stage::Keychain => res::str::dav_keychain_failed(),
            Hint::General => res::str::dav_failed(),
        }
    }

    pub(super) fn detail(&self) -> String {
        use ConnectionStep as Step;
        let operation = match self.stage {
            Stage::Probe(Step::OpenCollection) => res::str::dav_check_folder(),
            Stage::Probe(Step::CheckCreateOnly) => res::str::dav_check_overwrite(),
            Stage::Probe(Step::CreateAppFolder) => res::str::dav_create_folder(),
            Stage::Probe(Step::CreateCollection) => res::str::dav_test_file("MKCOL"),
            Stage::Probe(Step::CreateFile) => res::str::dav_test_file("PUT"),
            Stage::Probe(Step::ListFiles) => res::str::dav_test_file("PROPFIND"),
            Stage::Probe(Step::ReadFile) => res::str::dav_test_file("GET"),
            Stage::Probe(Step::RemoveCollection) => res::str::dav_test_file("DELETE"),
            Stage::Address => res::str::dav_endpoint(),
            Stage::Account => res::str::dav_connect(),
            Stage::Keychain => res::str::dav_keychain_failed(),
            Stage::Library => res::str::dav_open_library(),
            Stage::Sync => res::str::dav_title(),
        };
        let reason = match self.http {
            Some(status) => res::str::http_failed(status.to_string()),
            None => res::str::dav_error_code(self.code.clone()),
        };
        res::str::dav_detail(res::str::DavDetail {
            operation: operation.format(),
            reason: reason.format(),
        })
        .format()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use day_sync_webdav::webdav::{WriteOutcome, http::HttpError};

    #[test]
    fn missing_folder_requires_the_root_check_and_details_never_copy_server_data() {
        let missing = DavError::Status {
            status: 404,
            outcome: WriteOutcome::NotSent,
            retry_after: None,
        };
        let root = Failure::dav(Stage::Probe(ConnectionStep::OpenCollection), &missing);
        assert_eq!(root.hint, Hint::MissingFolder);
        assert_eq!(root.http, Some(404));
        for stage in [
            Stage::Probe(ConnectionStep::ReadFile),
            Stage::Library,
            Stage::Sync,
        ] {
            assert_ne!(Failure::dav(stage, &missing).hint, Hint::MissingFolder);
        }
        let tls = DavError::Transport {
            source: HttpError::Tls("synthetic private server text".into()),
            outcome: WriteOutcome::NotSent,
        };
        let failure = Failure::dav(Stage::Probe(ConnectionStep::OpenCollection), &tls);
        assert_eq!(failure.code, "http.tls");
        assert!(!format!("{failure:?}").contains("synthetic private"));
    }
}

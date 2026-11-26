// SPDX-License-Identifier: GPL-2.0-or-later
//! UI-task progress: transfer bytes, book counts, and explicit synchronization phases.
use crate::res;
use day::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Direction {
    Upload,
    Download,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) enum Work {
    #[default]
    Checking,
    Opening,
    Metadata,
    Descriptions {
        title: String,
        index: usize,
        total: usize,
    },
    Transfer {
        direction: Direction,
        book: String,
        title: String,
        index: usize,
        total: usize,
        bytes: u64,
        total_bytes: u64,
    },
    Finishing,
}
thread_local! { static WORK: Signal<Work> = Signal::global(Work::Checking); }
pub(super) fn current() -> Signal<Work> {
    WORK.with(|p| *p)
}
pub(super) fn set(work: Work) {
    current().set(work);
}

pub(super) fn transferred(direction: Direction, id: &str, bytes: u64, total_bytes: u64) {
    let mut work = current().get_untracked();
    if let Work::Transfer {
        direction: active,
        book,
        bytes: done,
        total_bytes: total,
        ..
    } = &mut work
        && *active == direction
        && book == id
    {
        *done = bytes.min(total_bytes);
        *total = total_bytes;
        current().set_if_changed(work);
    }
}
impl Work {
    /// This is a per-phase bar. Hold the last book below 100% until publication/indexing
    /// finishes; verified bytes alone do not establish a successful synchronization.
    pub(super) fn fraction(&self) -> Option<f64> {
        match self {
            Self::Transfer {
                index,
                total,
                bytes,
                total_bytes,
                ..
            } if *total > 0 => {
                let within = if *total_bytes == 0 {
                    0.
                } else {
                    (*bytes as f64 / *total_bytes as f64).min(0.99)
                };
                Some(((*index as f64 + within) / *total as f64).clamp(0., 1.))
            }
            Self::Descriptions { index, total, .. } if *total > 0 => {
                Some(*index as f64 / *total as f64)
            }
            _ => None,
        }
    }
    pub(super) fn message(&self) -> day::LocalizedText {
        match self {
            Self::Checking => res::str::dav_checking(),
            Self::Opening => res::str::dav_open_library(),
            Self::Metadata => res::str::dav_sync_metadata(),
            Self::Descriptions { title, .. } => res::str::dav_sync_description(title.clone()),
            Self::Transfer {
                direction: Direction::Upload,
                title,
                ..
            } => res::str::dav_uploading(title.clone()),
            Self::Transfer {
                direction: Direction::Download,
                title,
                ..
            } => res::str::dav_downloading(title.clone()),
            Self::Finishing => res::str::dav_finishing(),
        }
    }
    pub(super) fn detail(&self) -> String {
        match self {
            Self::Transfer { index, total, .. } | Self::Descriptions { index, total, .. } => {
                res::str::dav_book_count(res::str::DavBookCount {
                    current: (index + 1).to_string(),
                    total: total.to_string(),
                })
                .format()
            }
            _ => String::new(),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn progress_is_verified_bounded_and_scoped_to_the_active_book() {
        set(Work::Transfer {
            direction: Direction::Upload,
            book: "fixture-a".into(),
            title: "Synthetic title".into(),
            index: 1,
            total: 4,
            bytes: 0,
            total_bytes: 100,
        });
        transferred(Direction::Download, "fixture-a", 50, 100);
        transferred(Direction::Upload, "fixture-b", 50, 100);
        assert_eq!(current().get_untracked().fraction(), Some(0.25));
        transferred(Direction::Upload, "fixture-a", 50, 100);
        assert_eq!(current().get_untracked().fraction(), Some(0.375));
        transferred(Direction::Upload, "fixture-a", 200, 100);
        assert!(current().get_untracked().fraction().unwrap() < 0.5);
        set(Work::Metadata);
        assert_eq!(current().get_untracked().fraction(), None);
    }
}

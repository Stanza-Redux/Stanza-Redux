// SPDX-License-Identifier: AGPL-3.0-only WITH App-Fair-Distribution-Exception
//! One editor for adding and updating catalog sources. Validation never mutates the library.
use crate::*;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

#[derive(Clone, PartialEq)]
pub enum EditorRoute {
    Add,
    Edit(String),
}
impl Route for EditorRoute {
    fn key(&self) -> String {
        match self {
            Self::Add => "catalog-add".into(),
            Self::Edit(url) => format!("catalog-edit:{url}"),
        }
    }
    fn from_key(_: &str) -> Option<Self> {
        None
    }
    fn title(&self) -> String {
        match self {
            Self::Add => res::str::add_catalog().format(),
            Self::Edit(_) => res::str::edit_catalog().format(),
        }
    }
}

fn updated_catalogs(
    mut catalogs: Vec<opds::Link>,
    original: Option<&str>,
    url: String,
    title: String,
) -> Result<Vec<opds::Link>, AppError> {
    if catalogs
        .iter()
        .any(|c| c.url == url && Some(c.url.as_str()) != original)
    {
        return Err(AppError::new(res::str::catalog_exists));
    }
    let entry = opds::Link {
        url,
        title,
        ..Default::default()
    };
    if let Some(original) = original {
        let Some(index) = catalogs.iter().position(|c| c.url == original) else {
            return Err(AppError::new(res::str::catalog_invalid));
        };
        catalogs[index] = entry;
    } else {
        catalogs.push(entry);
    }
    Ok(catalogs)
}

pub fn view(a: App, route: EditorRoute) -> impl Piece {
    let original = match &route {
        EditorRoute::Add => None,
        EditorRoute::Edit(url) => Some(url.clone()),
    };
    let existing = original.as_ref().and_then(|url| {
        a.catalogs()
            .unwrap_or_default()
            .into_iter()
            .find(|c| &c.url == url)
    });
    let title = Signal::new(
        existing
            .as_ref()
            .map(|c| c.title.clone())
            .unwrap_or_default(),
    );
    let url = Signal::new(existing.as_ref().map(|c| c.url.clone()).unwrap_or_default());
    let validated = Signal::new(None::<(String, opds::Feed)>);
    let busy = Signal::new(false);
    let error = Signal::new(String::new());
    let generation = Rc::new(Cell::new(0_u64));
    let task = Rc::new(RefCell::new(None::<day::TaskHandle>));
    let cancel = {
        let generation = generation.clone();
        let task = task.clone();
        move || {
            generation.set(generation.get().wrapping_add(1));
            if let Some(task) = task.borrow_mut().take() {
                task.abort()
            }
        }
    };
    let invalidate = cancel.clone();
    watch(
        move || url.get(),
        move |new, old| {
            // A native text field may echo a programmatic update or commit the same preview
            // on focus loss. Only a changed URL invalidates the validation request/result.
            if old == Some(new) {
                return;
            }
            invalidate();
            validated.set(None);
            error.set(String::new());
            busy.set(false);
        },
    );
    Scope::current().on_cleanup(cancel);
    let validate = move || {
        if let Some(task) = task.borrow_mut().take() {
            task.abort()
        }
        let request = generation.get().wrapping_add(1);
        generation.set(request);
        let address = match opds::resolve(url.get_untracked().trim(), "") {
            Ok(url) => url,
            Err(e) => {
                error.set(e.localized());
                return;
            }
        };
        let requested = url.get_untracked();
        let initial_title = title.get_untracked();
        busy.set(true);
        validated.set(None);
        error.set(String::new());
        let generation = generation.clone();
        *task.borrow_mut() = Some(day::task(async move {
            let result = browser::load_feed(address.clone()).await;
            if generation.get() != request || url.get_untracked() != requested {
                return;
            }
            busy.set(false);
            match result {
                Ok(feed) if feed.is_catalog => {
                    // Preserve a user-supplied title, including edits made during the request.
                    if initial_title.trim().is_empty() && title.get_untracked() == initial_title {
                        title.set(feed.title.clone());
                    }
                    validated.set(Some((address, feed)));
                }
                Ok(_) => error.set(res::str::catalog_invalid().format()),
                Err(e) => error.set(e.localized()),
            }
        }));
    };
    let save = move || {
        let Some((address, feed)) = validated.get_untracked() else {
            return;
        };
        if busy.get_untracked()
            || title.get_untracked().trim().is_empty()
            || opds::resolve(url.get_untracked().trim(), "").ok().as_ref() != Some(&address)
        {
            return;
        }
        let catalogs = match a.catalogs() {
            Ok(c) => c,
            Err(e) => {
                error.set(e.localized());
                return;
            }
        };
        match updated_catalogs(
            catalogs,
            original.as_deref(),
            address.clone(),
            title.get_untracked().trim().into(),
        ) {
            Ok(catalogs) => {
                let Some(db) = a.database.get_untracked() else {
                    error.set(res::str::database_unavailable().format());
                    return;
                };
                match db.save_catalogs(&catalogs) {
                    Ok(()) => {
                        a.catalog_editor.set(None);
                        if original.is_none() {
                            a.browse_validated(address, feed);
                        }
                    }
                    Err(e) => {
                        day::warn!("Catalog save failed: {e}");
                        error.set(AppError::from(e.to_string()).localized());
                    }
                }
            }
            Err(e) => error.set(e.localized()),
        }
    };
    column((
        row((
            button(res::str::cancel())
                .action(move || a.catalog_editor.set(None))
                .id("catalog-editor-cancel"),
            label(route.title()).font(Font::Headline).grow_w(),
            button(res::str::save())
                .action(save)
                .enabled(move || {
                    !busy.get() && validated.get().is_some() && !title.get().trim().is_empty()
                })
                .id("catalog-editor-save"),
        ))
        .spacing(12.)
        .padding(16.),
        scroll(
            form((
                section((
                    labeled(
                        res::str::catalog_url(),
                        text_field(url).id("catalog-editor-url"),
                    ),
                    row((
                        button(res::str::validate_catalog())
                            .action(validate)
                            .enabled(move || !busy.get() && !url.get().trim().is_empty())
                            .id("catalog-editor-validate"),
                        when(
                            move || busy.get(),
                            move || {
                                row((
                                    spinner().frame(20., 20.),
                                    label(res::str::catalog_validating()),
                                ))
                                .spacing(8.)
                            },
                        ),
                    ))
                    .spacing(12.),
                    when(
                        move || !error.get().is_empty(),
                        move || label(move || error.get()).id("catalog-editor-error"),
                    ),
                )),
                section((
                    labeled(
                        res::str::catalog_name(),
                        text_field(title).id("catalog-editor-title"),
                    ),
                    label(res::str::catalog_title_hint())
                        .font(Font::Footnote)
                        .secondary(),
                )),
                when(
                    move || validated.get().is_some(),
                    move || {
                        let feed = validated.get_untracked().unwrap().1;
                        let has_description = !feed.description.is_empty();
                        let has_updated = !feed.updated.is_empty();
                        section((
                            label(res::str::catalog_validated())
                                .font(Font::Headline)
                                .id("catalog-editor-valid"),
                            labeled(res::str::catalog_title(), label(feed.title)),
                            label(res::str::catalog_counts(
                                feed.entries.len() as i64,
                                (feed.navigation.len() + feed.facets.len()) as i64,
                            )),
                            when(
                                move || has_description,
                                move || label(feed.description.clone()),
                            ),
                            when(
                                move || has_updated,
                                move || {
                                    labeled(
                                        res::str::catalog_updated(),
                                        label(feed.updated.clone()),
                                    )
                                },
                            ),
                        ))
                        .title(res::str::catalog_validation())
                    },
                ),
            ))
            .padding(20.),
        )
        .grow(),
    ))
    .max_width(640.)
    .grow_h()
    .id("catalog-editor")
}

#[cfg(test)]
mod tests {
    use super::*;
    fn catalog(url: &str) -> opds::Link {
        opds::Link {
            url: url.into(),
            title: url.into(),
            ..Default::default()
        }
    }
    #[test]
    fn editing_preserves_order_and_rejects_duplicates() {
        let updated = updated_catalogs(
            vec![catalog("https://one.test/"), catalog("https://two.test/")],
            Some("https://one.test/"),
            "https://new.test/".into(),
            "Renamed fixture".into(),
        )
        .unwrap();
        assert_eq!(updated[0].title, "Renamed fixture");
        assert_eq!(updated[1].url, "https://two.test/");
        assert!(
            updated_catalogs(
                updated.clone(),
                None,
                "https://new.test/".into(),
                "Duplicate fixture".into()
            )
            .is_err()
        );
        assert!(
            updated_catalogs(
                updated,
                Some("https://missing.test/"),
                "https://three.test/".into(),
                "Missing fixture".into()
            )
            .is_err()
        );
    }
    #[test]
    fn validation_distinguishes_catalogs_from_other_documents() {
        assert!(
            !opds::parse(
                br#"{"metadata":{"title":"Non-catalog fixture"}}"#,
                "https://example.org/"
            )
            .unwrap()
            .is_catalog
        );
        let feed = opds::parse(br#"{"metadata":{"title":"Catalog fixture","description":"Description fixture","modified":"2026-09-29"},"publications":[]}"#, "https://example.org/").unwrap();
        assert!(feed.is_catalog);
        assert_eq!(feed.description, "Description fixture");
        assert_eq!(feed.updated, "2026-09-29");
    }
}

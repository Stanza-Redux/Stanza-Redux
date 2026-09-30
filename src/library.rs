// SPDX-License-Identifier: AGPL-3.0-only WITH App-Fair-Distribution-Exception
//! Native library navigation; queries and membership come from the persistence store.
use crate::storage::{BookRecordFields, FolderRecordFields};
use crate::*;
use day::model::Source;

#[derive(Clone, Debug, PartialEq)]
pub enum Page {
    Recents,
    Titles,
    Authors,
    Genres,
    Author(String),
    Genre(String),
    Book(Saved),
}
impl Route for Page {
    fn key(&self) -> String {
        format!("{self:?}")
    }
    fn from_key(_: &str) -> Option<Self> {
        None
    }
    fn title(&self) -> String {
        match self {
            Self::Recents => res::str::library_recents().format(),
            Self::Titles => res::str::library_titles().format(),
            Self::Authors => res::str::library_authors().format(),
            Self::Genres => res::str::library_genres().format(),
            Self::Author(name) if name.is_empty() => res::str::unknown_author().format(),
            Self::Genre(name) if name.is_empty() => res::str::unknown_genre().format(),
            Self::Author(name) | Self::Genre(name) => name.clone(),
            Self::Book(b) => b.title.clone(),
        }
    }
}
fn push(a: App, page: Page) {
    a.library_query.set(String::new());
    a.library_path.update(|p| p.push(page));
}
fn heading(title: String) -> Vec<ToolbarEntry> {
    if cfg!(feature = "uikit") {
        vec![toolbar_label("library-heading", title).placement(ToolbarPlacement::Principal)]
    } else {
        vec![]
    }
}
fn actions(a: App, title: String) -> Vec<ToolbarEntry> {
    if a.section.get() != "library"
        || a.reader_open.get().is_some()
        || a.catalog_editor.get().is_some()
    {
        return vec![];
    }
    let mut actions = heading(title);
    actions.push(
        documents::open_command(a)
            .toolbar_item()
            .placement(ToolbarPlacement::Primary),
    );
    actions
}
pub fn view(a: App) -> impl Piece {
    nav_stack(a.library_path, folders(a, None))
        .title(res::str::library())
        .toolbar(move || {
            if a.library_path.get().is_empty() {
                actions(a, res::str::library().format())
            } else {
                vec![]
            }
        })
        .destination(move |page: &Page| {
            let current = page.clone();
            let title = page.title();
            let body = match page {
                Page::Authors => folders(a, Some("author")).any(),
                Page::Genres => folders(a, Some("genre")).any(),
                Page::Book(b) => scroll(local_details(a, b.clone())).any(),
                page => books(a, page.clone()).any(),
            };
            body.toolbar(move || {
                if a.library_path.get().last() != Some(&current) {
                    return vec![];
                }
                let mut entries = actions(a, title.clone());
                if let Page::Book(book) = &current {
                    let book = book.clone();
                    entries.push(
                        toolbar_button("delete-book", res::str::delete())
                            .icon(Symbol::Delete)
                            .placement(ToolbarPlacement::Secondary)
                            .action(move || remove(a, book.clone())),
                    );
                }
                entries
            })
            .any()
        })
        .id("library-stack")
        .grow()
        .overlay(when(
            move || a.busy.get(),
            move || {
                column((spinner().frame(28., 28.), label(res::str::loading())))
                    .spacing(8.)
                    .padding(20.)
                    .background(Color::hex(0xf1f3f6))
                    .corner_radius(12.)
                    .id("library-loading")
            },
        ))
}
fn folders(a: App, kind: Option<&'static str>) -> impl Piece {
    if let Some(kind) = kind {
        let Some(db) = a.database.get_untracked() else {
            return label(res::str::database_unavailable()).any();
        };
        return list(db.folder_query(kind), move |slot| {
            row((
                label(move || {
                    let name = slot.name().read();
                    if name.is_empty() {
                        if kind == "author" {
                            res::str::unknown_author().format()
                        } else {
                            res::str::unknown_genre().format()
                        }
                    } else {
                        name
                    }
                })
                .font(Font::Headline)
                .single_line()
                .grow(),
                vector(res::vectors::chevron_forward)
                    .frame(12., 24.)
                    .decorative(),
            ))
            .spacing(12.)
            .padding(16.)
        })
        .row_height(RowHeight::Uniform(64.))
        .on_select(move |folder| {
            let name = folder.name().read();
            push(
                a,
                if kind == "author" {
                    Page::Author(name)
                } else {
                    Page::Genre(name)
                },
            );
        })
        .id(if kind == "author" {
            "library-authors"
        } else {
            "library-genres"
        })
        .grow()
        .any();
    }
    list(
        items(
            || vec![Page::Recents, Page::Titles, Page::Authors, Page::Genres],
            |p: &Page| p.key(),
        ),
        move |slot| {
            row((
                label(move || slot.get().title())
                    .font(Font::Headline)
                    .single_line()
                    .grow(),
                vector(res::vectors::chevron_forward)
                    .frame(12., 24.)
                    .decorative(),
            ))
            .spacing(12.)
            .padding(16.)
        },
    )
    .row_height(RowHeight::Uniform(64.))
    .on_select(move |key| {
        if let Some(page) = [Page::Recents, Page::Titles, Page::Authors, Page::Genres]
            .into_iter()
            .find(|p| p.key() == key)
        {
            push(a, page);
        }
    })
    .id("library-folders")
    .grow()
    .any()
}
fn books(a: App, page: Page) -> impl Piece {
    let Some(db) = a.database.get_untracked() else {
        return label(res::str::database_unavailable()).any();
    };
    let recent = page == Page::Recents;
    let facet = match page {
        Page::Author(name) => Some(("author".into(), name)),
        Page::Genre(name) => Some(("genre".into(), name)),
        _ => None,
    };
    let query = db.book_query(facet, recent.then_some(50), move || a.library_query.get());
    let empty = query.clone();
    column((
        text_field(a.library_query)
            .placeholder(res::str::search_library())
            .id("library-search")
            .padding(12.),
        when(
            move || empty.count() == 0,
            move || {
                label(if recent {
                    res::str::library_recents_hint()
                } else {
                    res::str::library_no_results()
                })
                .secondary()
                .padding(20.)
                .id("library-empty")
            },
        ),
        list(query, move |slot| {
            row((
                covers::view(
                    move || {
                        slot.cover()
                            .read()
                            .map(|p| format!("library:{p}"))
                            .unwrap_or_default()
                    },
                    46.,
                    64.,
                ),
                column((
                    label(move || slot.title().read())
                        .font(Font::Headline)
                        .single_line(),
                    label(move || slot.author().read())
                        .font(Font::Footnote)
                        .secondary()
                        .single_line(),
                    label(move || slot.genres().read())
                        .font(Font::Caption)
                        .secondary()
                        .single_line(),
                ))
                .align(HAlign::Leading)
                .spacing(4.)
                .grow(),
                vector(res::vectors::chevron_forward)
                    .frame(12., 24.)
                    .decorative(),
            ))
            .spacing(12.)
            .padding(16.)
        })
        .row_height(RowHeight::Uniform(96.))
        .on_select(move |row| {
            if let Some(book) = row.with_value_untracked(|b| b.cloned()).map(Saved::from) {
                if cfg!(feature = "appkit") {
                    a.library_selection.set(Some(book));
                } else {
                    push(a, Page::Book(book));
                }
            }
        })
        .id("library-list")
        .grow(),
    ))
    .grow()
    .any()
}
#[cfg(feature = "appkit")]
pub fn desktop_detail(a: App) -> impl Piece {
    // Keyed replacement keeps asynchronous metadata/cover tasks bound to the selected book.
    column((
        each(
            items(
                move || a.library_selection.get().into_iter().collect::<Vec<_>>(),
                |b: &Saved| b.id.clone(),
            ),
            move |slot| {
                let book = slot.get();
                let remove_book = book.clone();
                scroll(local_details(a, book))
                    .toolbar(move || {
                        vec![
                            toolbar_button("delete-book", res::str::delete())
                                .icon(Symbol::Delete)
                                .placement(ToolbarPlacement::Secondary)
                                .action({
                                    let book = remove_book.clone();
                                    move || remove(a, book.clone())
                                }),
                        ]
                    })
                    .grow()
            },
        ),
        when(
            move || a.library_selection.get().is_none(),
            || {
                label(res::str::library_select_book())
                    .secondary()
                    .padding(24.)
            },
        ),
    ))
    .grow()
}
fn remove(a: App, book: Saved) {
    day::task(async move {
        if !confirm(res::str::remove_book_question()).await {
            return;
        }
        let Some(db) = a.database.get_untracked() else {
            return;
        };
        if let Err(e) = db.remove(&book.id) {
            a.error(e.to_string());
            return;
        }

        a.library_selection.set(None);
        a.library_path.update(|p| {
            if matches!(p.last(), Some(Page::Book(_))) {
                p.pop();
            }
        });
        if let Err(e) = day_part_fs::remove_future(&format!("books/{}.epub", book.id)).await {
            a.error(e.to_string());
        }
        if let Some(path) = book.cover {
            let _ = day_part_fs::remove_future(&path).await;
        }
    });
}

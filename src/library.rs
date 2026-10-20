// SPDX-License-Identifier: AGPL-3.0-only WITH App-Fair-Distribution-Exception
//! Native library navigation; queries and membership come from the persistence store.
use crate::storage::{BookRecord, BookRecordFields, FolderRecord, FolderRecordFields};
use crate::*;
use day::model::Source;
use day_persistence::CountQuery;
use std::rc::Rc;

/// How many books the Recents collection lists.
const RECENTS_LIMIT: usize = 50;

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
                    .spacing(Space::S)
                    .card()
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
            .spacing(Space::M)
            .padding(Space::L)
        })
        .row_height(RowHeight::Automatic)
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
    // Live counts for the summary under each collection's name; absent (an empty summary)
    // while the database is still opening.
    let counts = a.database.get_untracked().map(|db| {
        Rc::new(Counts {
            recents: Box::new(db.recent_count(RECENTS_LIMIT)),
            books: db.book_count(),
            authors: db.folder_count("author"),
            genres: db.folder_count("genre"),
        })
    });
    list(
        items(
            || vec![Page::Recents, Page::Titles, Page::Authors, Page::Genres],
            |p: &Page| p.key(),
        ),
        move |slot| {
            // One glyph per collection; the rows recycle, so each follows the slot.
            let glyph = |page: Page, name: day::VectorName| {
                when(
                    move || slot.get() == page,
                    move || vector(name.clone()).template().frame(24., 24.).decorative(),
                )
            };
            let counts = counts.clone();
            row((
                column((
                    glyph(Page::Recents, res::vectors::library_recents),
                    glyph(Page::Titles, res::vectors::library_titles),
                    glyph(Page::Authors, res::vectors::library_authors),
                    glyph(Page::Genres, res::vectors::library_genres),
                ))
                .frame(24., 24.),
                column((
                    label(move || slot.get().title())
                        .font(Font::Headline)
                        .single_line(),
                    label(move || summary(counts.as_deref(), &slot.get()))
                        .font(Font::Footnote)
                        .secondary()
                        .single_line(),
                ))
                .spacing(2.)
                .align(HAlign::Leading)
                .grow(),
                vector(res::vectors::chevron_forward)
                    .frame(12., 24.)
                    .decorative(),
            ))
            .spacing(Space::M)
            .align(VAlign::Center)
            .padding(Insets::symmetric(16., 10.))
        },
    )
    // As tall as a name and its summary at the platform's text size, which the reader may
    // have made larger (Android's type scale starts larger than iOS's, too).
    .row_height(RowHeight::Automatic)
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
/// The live counts behind the top-level collections' summaries.
struct Counts {
    recents: Box<dyn Fn() -> usize>,
    books: CountQuery<BookRecord>,
    authors: CountQuery<FolderRecord>,
    genres: CountQuery<FolderRecord>,
}

/// The summary line under a collection's name: how many entries the page will list. Tracked
/// reads, so an import or a removal updates the line in place.
fn summary(counts: Option<&Counts>, page: &Page) -> String {
    let Some(c) = counts else {
        return String::new();
    };
    match page {
        Page::Recents => res::str::library_recents_summary((c.recents)() as i64).format(),
        Page::Titles => res::str::library_titles_summary(c.books.get() as i64).format(),
        Page::Authors => res::str::library_authors_summary(c.authors.get() as i64).format(),
        Page::Genres => res::str::library_genres_summary(c.genres.get() as i64).format(),
        _ => String::new(),
    }
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
    let query = db.book_query(facet, recent.then_some(RECENTS_LIMIT), move || {
        a.library_query.get()
    });
    let empty = query.clone();
    column((
        text_field(a.library_query)
            .placeholder(res::str::search_library())
            .id("library-search")
            .padding(Space::M),
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
                .spacing(Space::XS)
                .grow(),
                vector(res::vectors::chevron_forward)
                    .frame(12., 24.)
                    .decorative(),
            ))
            .spacing(Space::M)
            .padding(Space::L)
        })
        .row_height(RowHeight::Automatic)
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
        keyed(
            move || a.library_selection.get(),
            move |book| {
                let remove_book = book.clone();
                scroll(local_details(a, book.clone()))
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
                    .padding(Space::XL)
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

// SPDX-License-Identifier: AGPL-3.0-only WITH App-Fair-Distribution-Exception
//! Native catalog drill-down. Each pushed page owns its feed snapshot.
use crate::storage::CatalogRecordFields;
use crate::*;
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone)]
pub struct CatalogPage {
    id: u64,
    depth: usize,
    manager: bool,
    pub feed: Rc<opds::Feed>,
    pub publication: Option<opds::Publication>,
    loading: bool,
    failure: Option<AppError>,
    request: Option<day::TaskHandle>,
}
impl PartialEq for CatalogPage {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Route for CatalogPage {
    fn key(&self) -> String {
        self.id.to_string()
    }
    // Feeds are acquired asynchronously; arbitrary deep links cannot recreate cached pages.
    fn from_key(_: &str) -> Option<Self> {
        None
    }
    fn title(&self) -> String {
        if self.manager {
            return res::str::manage_catalogs().format();
        }
        self.publication
            .as_ref()
            .map(|p| p.title.clone())
            .unwrap_or_else(|| self.feed.title.clone())
    }
}
impl App {
    pub fn browse_validated(self, url: String, feed: opds::Feed) {
        self.url.set(url);
        let title = feed.title.clone();
        self.load_catalog(true, title, async move { Ok(feed) });
    }
    pub fn browse_named(self, url: String, root: bool, title: String) {
        self.url.set(url.clone());
        self.load_catalog(root, title, async move { load_feed(url).await });
    }
    fn load_catalog(
        self,
        root: bool,
        title: String,
        load: impl std::future::Future<Output = Result<opds::Feed, AppError>> + 'static,
    ) {
        self.cancel();
        self.status.set(String::new());
        self.section.set("catalogs".into());
        let id = self.catalog_serial.get_untracked() + 1;
        self.catalog_serial.set(id);
        let depth = if root {
            1
        } else {
            self.catalog_path.get_untracked().len() + 1
        };
        let page = CatalogPage {
            id,
            depth,
            manager: false,
            feed: Rc::new(opds::Feed {
                title,
                ..Default::default()
            }),
            publication: None,
            loading: true,
            failure: None,
            request: None,
        };
        // Push before I/O; the destination owns loading and its native Back stays usable.
        let mut path = if root {
            Vec::new()
        } else {
            self.catalog_path.get_untracked()
        };
        path.push(page);
        self.catalog_forward.set(Vec::new());
        self.catalog_path.set(path);
        let task = day::task(async move {
            let result = load.await;
            // Finishing an already-cancelled parse must never reinsert the destination.
            self.catalog_path.update(|path| {
                if let Some(page) = path.iter_mut().find(|page| page.id == id) {
                    page.loading = false;
                    page.request = None;
                    match result {
                        Ok(feed) => page.feed = Rc::new(feed),
                        Err(error) => page.failure = Some(error),
                    }
                }
            });
        });
        self.catalog_path.update(|path| {
            if let Some(page) = path.iter_mut().find(|page| page.id == id) {
                page.request = Some(task);
            }
        });
    }
    pub fn catalog_step(self, forward: bool) {
        if self.reader_open.get_untracked().is_some() {
            return;
        }
        if forward {
            let mut redo = self.catalog_forward.get_untracked();
            if let Some(page) = redo.pop() {
                self.catalog_forward.set(redo);
                self.catalog_path.update(|p| p.push(page));
            }
        } else {
            self.catalog_path.update(|p| {
                p.pop();
            });
        }
    }
    pub fn select_publication(self, feed: Rc<opds::Feed>, publication: opds::Publication) {
        let id = self.catalog_serial.get_untracked() + 1;
        self.catalog_serial.set(id);
        self.catalog_forward.set(Vec::new());
        self.catalog_path.update(|p| {
            p.push(CatalogPage {
                id,
                depth: p.len() + 1,
                feed,
                publication: Some(publication),
                manager: false,
                loading: false,
                failure: None,
                request: None,
            })
        });
    }
    pub fn manage_catalogs(self) {
        let id = self.catalog_serial.get_untracked() + 1;
        self.catalog_serial.set(id);
        self.catalog_path.update(|p| {
            p.push(CatalogPage {
                id,
                depth: p.len() + 1,
                manager: true,
                feed: Rc::new(opds::Feed::default()),
                publication: None,
                loading: false,
                failure: None,
                request: None,
            })
        });
    }
    pub fn save_catalogs(self, next: Vec<opds::Link>) {
        if let Some(db) = self.database.get_untracked() {
            match db.save_catalogs(&next) {
                Ok(()) => {}
                Err(e) => self.error(e.to_string()),
            }
        }
    }
}
// Native back gestures and programmatic pops share the same path binding. Keep forward
// history outside page scopes so popping a page never loses the saved destination.
pub fn observe_path(a: App) {
    watch(
        move || a.catalog_path.get(),
        move |next, old| {
            if let Some(old) = old {
                for removed in old.iter().filter(|p| !next.iter().any(|n| n.id == p.id)) {
                    if let Some(task) = &removed.request {
                        task.abort();
                    }
                }
                if next.len() < old.len() && old.starts_with(next) {
                    a.catalog_forward.update(|f| {
                        f.extend(
                            old[next.len()..]
                                .iter()
                                .rev()
                                .filter(|p| !p.manager && !p.loading && p.failure.is_none())
                                .cloned(),
                        )
                    });
                }
            }
            if old.is_some_and(|old| next.len() < old.len()) {
                a.cancel();
            }
            a.catalog_focus.set(true);
            a.status.set(String::new());
        },
    );
}
/// Trackpad deltas are incremental, including inertial tails. One navigation per gesture.
#[derive(Default)]
pub struct HorizontalGesture {
    x: f64,
    y: f64,
    last: Option<u64>,
    fired: bool,
}
impl HorizontalGesture {
    pub fn update(&mut self, phase: DragPhase, x: f64, y: f64, now: u64) -> Option<bool> {
        if phase == DragPhase::Began
            || self
                .last
                .is_none_or(|t| now.saturating_sub(t) > 240 || now < t)
        {
            self.x = 0.;
            self.y = 0.;
            self.fired = false;
        }
        self.last = Some(now);
        self.x += x;
        self.y += y;
        if !self.fired && self.x.abs() > 70. && self.x.abs() > self.y.abs() * 1.5 {
            self.fired = true;
            return Some(self.x < 0.);
        }
        None
    }
}
pub fn view(a: App) -> impl Piece {
    let gesture = Rc::new(RefCell::new(HorizontalGesture::default()));
    // The tab owns the navigation controller directly: no padded page or hand-built
    // header around it. UIKit can extend this host under its floating tab bar.
    column((nav_stack(a.catalog_path, sources(a))
        .title(res::str::catalogs())
        .toolbar(move || {
            if a.catalog_path.get().is_empty() {
                catalog_actions(a, true, None, res::str::catalogs().format())
            } else {
                vec![]
            }
        })
        .destination(move |page: &CatalogPage| {
            let page_id = page.id;
            let depth = page.depth;
            if page.manager {
                manager(a)
                    .toolbar(move || {
                        let mut actions = title_action(res::str::manage_catalogs().format());
                        actions.push(
                            toolbar_button("catalog-manager-done", res::str::done())
                                .placement(ToolbarPlacement::Primary)
                                .action(move || a.catalog_step(false)),
                        );
                        actions
                    })
                    .any()
            } else {
                // Unmount hidden row scopes to abort their thumbnail tasks. Route data stays
                // cached, so Back reconstructs the preceding list without fetching its feed.
                when(
                    move || a.catalog_path.get().last().is_some_and(|p| p.id == page_id),
                    move || {
                        when(
                            move || current(a, page_id).is_some_and(|p| p.loading),
                            move || {
                                column((
                                    spacer(),
                                    row((spacer(), loading_indicator(), spacer())),
                                    spacer(),
                                ))
                                .grow()
                            },
                        )
                        .otherwise(move || {
                            let Some(page) = current(a, page_id) else {
                                return spacer().any();
                            };
                            if let Some(error) = &page.failure {
                                return column((
                                    spacer(),
                                    label(error.localized())
                                        .secondary()
                                        .padding(24.)
                                        .id("catalog-error"),
                                    spacer(),
                                ))
                                .grow()
                                .any();
                            }
                            if let Some(publication) = page.publication {
                                scroll(crate::details(a, publication))
                                    .overlay(loading(a, depth))
                                    .any()
                            } else {
                                results(a, page.feed, depth).any()
                            }
                        })
                    },
                )
                .toolbar(move || {
                    let Some(page) = current(a, page_id)
                        .filter(|_| a.catalog_path.get().last().is_some_and(|p| p.id == page_id))
                    else {
                        return vec![];
                    };
                    let title = page.title();
                    if page.loading {
                        let mut entries = title_action(title);
                        entries.push(
                            toolbar_button("catalog-cancel", res::str::cancel())
                                .icon(Symbol::Close)
                                .placement(ToolbarPlacement::Primary)
                                .action(move || a.catalog_step(false)),
                        );
                        entries
                    } else {
                        let search = if page.publication.is_none() {
                            page.feed.search.clone()
                        } else {
                            None
                        };
                        catalog_actions(a, false, search, title)
                    }
                })
                .any()
            }
        })
        .id("catalog-stack")
        .grow(),))
    .focusable()
    .focused(a.catalog_focus)
    .on_key(move |k| catalog_key(a, &k.key))
    .on_pan(move |p| {
        if let Some(forward) =
            gesture
                .borrow_mut()
                .update(p.phase, p.delta.x, p.delta.y, crate::now_ms())
        {
            a.catalog_step(forward)
        }
    })
    .id("catalog-browser")
}
fn current(a: App, id: u64) -> Option<CatalogPage> {
    a.catalog_path.get().into_iter().find(|p| p.id == id)
}
fn title_action(title: String) -> Vec<ToolbarEntry> {
    if cfg!(feature = "uikit") {
        vec![toolbar_label("catalog-heading", title).placement(ToolbarPlacement::Principal)]
    } else {
        vec![]
    }
}
fn catalog_actions(
    a: App,
    root: bool,
    search_link: Option<opds::Link>,
    title: String,
) -> Vec<ToolbarEntry> {
    if a.section.get() != "catalogs"
        || a.reader_open.get().is_some()
        || a.catalog_editor.get().is_some()
    {
        return vec![];
    }
    let mut actions = title_action(title);
    if a.busy.get() {
        actions.push(
            toolbar_button("catalog-cancel", res::str::cancel())
                .icon(Symbol::Close)
                .placement(ToolbarPlacement::Primary)
                .action(move || a.cancel()),
        );
    }
    if let Some(link) = search_link {
        actions.push(
            toolbar_button("catalog-search", res::str::search())
                .icon(Symbol::Search)
                .placement(ToolbarPlacement::Primary)
                .action(move || {
                    let link = link.clone();
                    day::task(async move {
                        if let Some(query) = prompt(res::str::search()).await {
                            if !query.trim().is_empty() {
                                search(a, link, query);
                            }
                        }
                    });
                }),
        );
    }
    actions.push(
        toolbar_button("manage-catalogs", res::str::manage())
            .icon(Symbol::Settings)
            .placement(if root {
                ToolbarPlacement::Primary
            } else {
                ToolbarPlacement::Secondary
            })
            .action(move || a.manage_catalogs()),
    );
    if cfg!(any(
        target_os = "ios",
        target_os = "android",
        target_env = "ohos",
        target_arch = "wasm32"
    )) {
        actions.push(
            toolbar_button("add-opds-catalog", res::str::add_catalog())
                .icon(Symbol::Add)
                .placement(ToolbarPlacement::Secondary)
                .action(move || a.catalog_editor.set(Some(catalog_editor::EditorRoute::Add))),
        );
    }

    if !a.catalog_forward.get().is_empty() {
        actions.push(
            toolbar_button("catalog-forward", res::str::forward())
                .icon(Symbol::Forward)
                .placement(ToolbarPlacement::Secondary)
                .action(move || a.catalog_step(true)),
        );
    }
    actions
}
fn catalog_key(a: App, key: &str) {
    match key {
        "ArrowLeft" => a.catalog_step(false),
        "ArrowRight" => a.catalog_step(true),
        _ => {}
    }
}
fn loading_indicator() -> impl Piece {
    column((spinner().frame(28., 28.), label(res::str::loading())))
        .spacing(8.)
        .id("catalog-loading")
}
fn loading(a: App, depth: usize) -> impl Piece {
    when(
        move || a.busy.get() && a.catalog_path.get().len() == depth,
        move || {
            loading_indicator()
                .padding(20.)
                .background(Color::hex(0xf1f3f6))
                .corner_radius(12.)
        },
    )
}
fn sources(a: App) -> impl Piece {
    let Some(db) = a.database.get_untracked() else {
        return label(res::str::database_unavailable()).any();
    };
    list(db.catalog_query(), move |slot| {
        row((
            column((
                label(move || slot.title().read()).font(Font::Headline),
                label(move || slot.url().read()).font(Font::Footnote),
            ))
            .spacing(4.)
            .align(HAlign::Leading)
            .grow(),
            vector(res::vectors::chevron_forward)
                .frame(12., 24.)
                .decorative(),
        ))
        .spacing(12.)
        .padding(16.)
        .context_menu_fn(move |_| {
            let url = slot.url().read();
            vec![
                menu_item(res::str::edit_catalog().format())
                    .icon(Symbol::Edit)
                    .id("edit-opds-catalog")
                    .action(move || {
                        a.catalog_editor
                            .set(Some(catalog_editor::EditorRoute::Edit(url.clone())))
                    }),
            ]
        })
    })
    .row_height(RowHeight::Uniform(76.))
    .on_select(move |catalog| {
        a.browse_named(catalog.url().read(), true, catalog.title().read());
    })
    .id("catalog-sources")
    .grow()
    .overlay(loading(a, 0))
    .any()
}
#[derive(Clone)]
enum CatalogRow {
    Link(opds::Link),
    Book(opds::Publication),
    Next(String),
}
impl CatalogRow {
    fn genre(&self) -> String {
        match self {
            Self::Book(p) => p.metadata.subjects.join(" · "),
            _ => String::new(),
        }
    }
    fn cover(&self) -> String {
        match self {
            Self::Book(p) => p.cover.clone(),
            _ => String::new(),
        }
    }
    fn title(&self) -> String {
        match self {
            Self::Link(l) => l.title.clone(),
            Self::Book(p) => p.title.clone(),
            Self::Next(_) => res::str::next_results().format(),
        }
    }
    fn subtitle(&self) -> String {
        match self {
            Self::Book(p) => p.author.clone(),
            _ => String::new(),
        }
    }
}
fn results(a: App, feed: Rc<opds::Feed>, depth: usize) -> impl Piece {
    let list_id = format!("catalog-list-{depth}-{}", feed.title);
    let detail_feed = feed.clone();
    let mut rows: Vec<CatalogRow> = feed
        .navigation
        .iter()
        .chain(&feed.facets)
        .cloned()
        .map(CatalogRow::Link)
        .collect();
    rows.extend(feed.entries.iter().cloned().map(CatalogRow::Book));
    if let Some(next) = &feed.next {
        rows.push(CatalogRow::Next(next.clone()));
    }
    let rows = Rc::new(rows);
    let source = rows.clone();
    list(
        items(
            move || source.iter().cloned().enumerate().collect::<Vec<_>>(),
            |(i, _): &(usize, CatalogRow)| *i,
        ),
        move |slot| {
            row((
                when(
                    move || matches!(slot.get().1, CatalogRow::Book(_)),
                    move || covers::view(move || slot.get().1.cover(), 46., 64.),
                ),
                column((
                    label(move || slot.get().1.title())
                        .font(Font::Headline)
                        .single_line(),
                    label(move || slot.get().1.subtitle())
                        .font(Font::Footnote)
                        .secondary()
                        .single_line(),
                    when(
                        move || !slot.get().1.genre().is_empty(),
                        move || {
                            label(move || slot.get().1.genre())
                                .font(Font::Caption)
                                .secondary()
                                .single_line()
                        },
                    ),
                ))
                .spacing(4.)
                .align(HAlign::Leading)
                .grow(),
                vector(res::vectors::chevron_forward)
                    .frame(12., 24.)
                    .decorative(),
            ))
            .spacing(12.)
            .padding(16.)
        },
    )
    .row_height(RowHeight::Uniform(96.))
    .on_select(move |i| match &rows[i] {
        CatalogRow::Link(l) => a.browse_named(l.url.clone(), false, l.title.clone()),
        CatalogRow::Next(u) => a.browse(u.clone(), false),
        CatalogRow::Book(p) => a.select_publication(detail_feed.clone(), p.clone()),
    })
    .id(list_id)
    .grow()
    .overlay(loading(a, depth))
}
pub(crate) async fn load_feed(url: String) -> Result<opds::Feed, AppError> {
    let (bytes, final_url) = fetch_bytes(&url, 8 * 1024 * 1024, None).await?;
    covers::background(move || opds::parse(&bytes, &final_url))
        .await
        .map_err(AppError::from)?
}
fn search(a: App, link: opds::Link, query: String) {
    a.load_catalog(false, res::str::search().format(), async move {
        let template = if link.mime.contains("opensearch") {
            let (bytes, url) = fetch_bytes(&link.url, 1024 * 1024, None).await?;
            opds::open_search(&bytes, &url)?
        } else {
            link.url
        };
        load_feed(opds::search_url(&template, &query)?).await
    });
}
fn manager(a: App) -> impl Piece {
    let Some(db) = a.database.get_untracked() else {
        return label(res::str::database_unavailable()).any();
    };
    column((scroll(
        column((each(db.catalog_query(), move |item| {
            let c = opds::Link {
                title: item.title().read(),
                url: item.url().read(),
                ..Default::default()
            };
            let up = c.url.clone();
            let down = c.url.clone();
            let remove = c.url.clone();
            column((
                label(move || item.title().read()).font(Font::Headline),
                label(c.url.clone()).font(Font::Footnote),
                row((
                    button(res::str::edit_catalog())
                        .action({
                            let url = c.url.clone();
                            move || {
                                a.catalog_editor
                                    .set(Some(catalog_editor::EditorRoute::Edit(url.clone())))
                            }
                        })
                        .id(format!("catalog-edit-{}", c.url)),
                    button(res::str::move_up())
                        .enabled({
                            let u = up.clone();
                            move || {
                                a.catalogs()
                                    .is_ok_and(|c| c.first().is_some_and(|c| c.url != u))
                            }
                        })
                        .action(move || reorder(a, &up, -1))
                        .id(format!("catalog-up-{}", c.url)),
                    button(res::str::move_down())
                        .enabled({
                            let u = down.clone();
                            move || {
                                a.catalogs()
                                    .is_ok_and(|c| c.last().is_some_and(|c| c.url != u))
                            }
                        })
                        .action(move || reorder(a, &down, 1))
                        .id(format!("catalog-down-{}", c.url)),
                    button(res::str::delete())
                        .action(move || {
                            let mut c = match a.catalogs() {
                                Ok(c) => c,
                                Err(e) => {
                                    a.error(e);
                                    return;
                                }
                            };
                            c.retain(|c| c.url != remove);
                            a.save_catalogs(c)
                        })
                        .id(format!("catalog-remove-{}", c.url)),
                ))
                .spacing(8.),
            ))
            .spacing(6.)
            .padding(10.)
        }),))
        .spacing(10.),
    )
    .grow()
    .id("catalog-manager-list"),))
    .spacing(12.)
    .padding(16.)
    .any()
}
fn reorder(a: App, url: &str, delta: isize) {
    let mut c = match a.catalogs() {
        Ok(c) => c,
        Err(e) => {
            a.error(e);
            return;
        }
    };
    if let Some(i) = c.iter().position(|c| c.url == url) {
        let j = i as isize + delta;
        if j >= 0 && (j as usize) < c.len() {
            c.swap(i, j as usize);
            a.save_catalogs(c)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn horizontal_gesture_latches() {
        let mut g = HorizontalGesture::default();
        assert_eq!(g.update(DragPhase::Began, 2., 80., 0), None);
        assert_eq!(g.update(DragPhase::Changed, 15., 40., 0), None);
        assert_eq!(g.update(DragPhase::Began, -90., 2., 0), Some(true));
        assert_eq!(g.update(DragPhase::Changed, -100., 0., 0), None);
    }
}

// SPDX-License-Identifier: AGPL-3.0-only WITH App-Fair-Distribution-Exception
//! Native catalog drill-down. Each pushed page owns its feed snapshot.
use crate::storage::CatalogRecordFields;
use crate::*;
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone)]
enum CatalogRequest {
    Feed(String),
    Search(opds::Link, String),
}
impl CatalogRequest {
    async fn load(self) -> Result<opds::Feed, AppError> {
        match self {
            Self::Feed(url) => load_feed(url).await,
            Self::Search(link, query) => {
                let template = if link.mime.contains("opensearch") {
                    let (bytes, url) = fetch_bytes(&link.url, 1024 * 1024, None).await?;
                    opds::open_search(&bytes, &url)?
                } else {
                    link.url
                };
                load_feed(opds::search_url(&template, &query)?).await
            }
        }
    }
}
#[derive(Clone)]
pub struct CatalogPage {
    id: u64,
    depth: usize,
    manager: bool,
    pub feed: Rc<opds::Feed>,
    /// The name of the link that opened this page. Catalogs often give every feed one title.
    label: String,
    pub publication: Option<opds::Publication>,
    loading: bool,
    failure: Option<AppError>,
    request: Option<day::TaskHandle>,
    retry: Option<CatalogRequest>,
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
        match &self.publication {
            Some(p) => p.title.clone(),
            None if self.label.is_empty() => self.feed.title.clone(),
            None => self.label.clone(),
        }
    }
}
impl App {
    pub fn browse_validated(self, url: String, feed: opds::Feed) {
        self.url.set(url.clone());
        let title = feed.title.clone();
        self.load_catalog(true, false, title, CatalogRequest::Feed(url), async move {
            Ok(feed)
        });
    }
    pub fn browse_named(self, url: String, root: bool, title: String) {
        self.url.set(url.clone());
        self.load_catalog(
            root,
            false,
            title,
            CatalogRequest::Feed(url.clone()),
            async move { load_feed(url).await },
        );
    }
    fn load_catalog(
        self,
        root: bool,
        replace: bool,
        title: String,
        retry: CatalogRequest,
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
            self.catalog_path.get_untracked().len() + 1 - usize::from(replace)
        };
        let page = CatalogPage {
            id,
            depth,
            manager: false,
            feed: Rc::new(opds::Feed {
                title: title.clone(),
                ..Default::default()
            }),
            label: title,
            publication: None,
            loading: true,
            failure: None,
            request: None,
            retry: Some(retry),
        };
        // Push before I/O; the destination owns loading and its native Back stays usable.
        let mut path = if root {
            Vec::new()
        } else {
            self.catalog_path.get_untracked()
        };
        if replace {
            path.pop();
        }
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
    fn reload_catalog(self, id: u64) {
        let Some(page) = self
            .catalog_path
            .get_untracked()
            .last()
            .cloned()
            .filter(|p| p.id == id && !p.loading)
        else {
            return;
        };
        let Some(retry) = page.retry.clone() else {
            return;
        };
        self.load_catalog(false, true, page.title(), retry.clone(), retry.load());
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
    pub fn select_publication(self, publication: opds::Publication) {
        let id = self.catalog_serial.get_untracked() + 1;
        self.catalog_serial.set(id);
        self.catalog_forward.set(Vec::new());
        self.catalog_path.update(|p| {
            p.push(CatalogPage {
                id,
                depth: p.len() + 1,
                feed: Rc::new(opds::Feed::default()),
                label: String::new(),
                publication: Some(publication),
                manager: false,
                loading: false,
                failure: None,
                request: None,
                retry: None,
            })
        });
    }
    /// A facet refines the current destination instead of stacking another page above it.
    fn apply_facet(self, page_id: u64, url: String) {
        let Some(page) = self
            .catalog_path
            .get_untracked()
            .last()
            .cloned()
            .filter(|p| p.id == page_id && !p.loading)
        else {
            return;
        };
        let title = page.title();
        self.load_catalog(
            false,
            true,
            title,
            CatalogRequest::Feed(url.clone()),
            async move { load_feed(url).await },
        );
    }
    /// Appends the next page of results to a loaded destination, keeping its cached feed in step.
    fn append_results(self, page_id: u64, feed: opds::Feed) {
        self.catalog_path.update(|path| {
            if let Some(page) = path.iter_mut().find(|p| p.id == page_id) {
                let mut merged = (*page.feed).clone();
                merged.entries.extend(feed.entries);
                merged.next = feed.next;
                page.feed = Rc::new(merged);
            }
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
                label: String::new(),
                publication: None,
                loading: false,
                failure: None,
                request: None,
                retry: None,
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
                        let style = ManagerStyle::current();
                        let mut actions = title_action(res::str::manage_catalogs().format());
                        // Android's back arrow already closes the page; a Done there would
                        // only fold into the overflow menu.
                        if style != ManagerStyle::Touch {
                            actions.push(
                                toolbar_button("catalog-manager-done", res::str::done())
                                    .placement(ToolbarPlacement::Primary)
                                    .action(move || a.catalog_step(false)),
                            );
                        }
                        // The desktops add from the bar under the list, the other phones
                        // from a floating button over it.
                        if style == ManagerStyle::Editing {
                            actions.push(
                                toolbar_button("catalog-manager-add", res::str::add_catalog())
                                    .icon(Symbol::Add)
                                    .placement(ToolbarPlacement::Primary)
                                    .action(move || {
                                        a.catalog_editor.set(Some(catalog_editor::EditorRoute::Add))
                                    }),
                            );
                        }
                        actions
                    })
                    .any()
            } else {
                // A page stays built while pages are pushed above it, so Back shows the same
                // list, at the same scroll position, without rebuilding it.
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
                            button(res::str::retry())
                                .prominent()
                                .action(move || a.reload_catalog(page_id))
                                .id("catalog-retry"),
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
                        let title = page.title();
                        results(a, page_id, page.feed, title, depth).any()
                    }
                })
                .toolbar(move || {
                    let Some(page) = current(a, page_id).filter(|_| {
                        a.catalog_path.get().last().is_some_and(|p| p.id == page_id)
                            && a.section.get() == "catalogs"
                            && a.reader_open.get().is_none()
                            && a.catalog_editor.get().is_none()
                    }) else {
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
                        let mut actions = catalog_actions(a, false, search, title);
                        if page.publication.is_none() && !page.feed.facets.is_empty() {
                            actions.push(facet_menu(a, page_id, &page.feed.facets));
                        }
                        if page.retry.is_some() && page.publication.is_none() {
                            actions.push(
                                toolbar_button("catalog-refresh", res::str::refresh())
                                    .icon(Symbol::Refresh)
                                    .placement(ToolbarPlacement::Secondary)
                                    .action(move || a.reload_catalog(page_id)),
                            );
                        }
                        actions
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
    // Adding a catalog belongs to the catalog list, not to the pages of one catalog.
    if root
        && cfg!(any(
            target_os = "ios",
            target_os = "android",
            target_env = "ohos",
            target_arch = "wasm32"
        ))
    {
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
/// Facets are refinements of the current results, so they live in one pull-down rather than
/// in the list: each group is a submenu, and the facet that produced this feed is checked.
fn facet_menu(a: App, page_id: u64, facets: &[opds::FacetGroup]) -> ToolbarEntry {
    let item = |group: usize, index: usize, l: &opds::Link| {
        let url = l.url.clone();
        let label = match l.count {
            Some(n) => {
                res::str::facet_count(l.title.clone(), day::format_decimal(n as f64, 0)).format()
            }
            None => l.title.clone(),
        };
        menu_item(label)
            .id(format!("catalog-facet-{group}-{index}"))
            .checked(l.active)
            .action(move || a.apply_facet(page_id, url.clone()))
    };
    let entries = match facets {
        [only] if only.title.is_empty() => {
            let links = only.links.iter().enumerate();
            links.map(|(i, l)| item(0, i, l)).collect()
        }
        _ => facets
            .iter()
            .enumerate()
            .map(|(g, group)| {
                let links = group.links.iter().enumerate();
                sub_menu(
                    group.title.clone(),
                    links.map(|(i, l)| item(g, i, l)).collect(),
                )
            })
            .collect(),
    };
    toolbar_menu("catalog-facets", res::str::sort_filter(), entries)
        .icon(Symbol::Filter)
        .placement(ToolbarPlacement::Primary)
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
    let catalogs = db.catalog_query();
    let empty = catalogs.clone();
    column((
        column((
            label(res::str::catalog_heading()).font(Font::Title).bold(),
            label(res::str::catalog_hint())
                .font(Font::Footnote)
                .secondary(),
        ))
        .align(HAlign::Leading)
        .spacing(6.)
        .padding(20.)
        .grow_w(),
        when(
            move || empty.try_collect().is_ok_and(|rows| rows.is_empty()),
            move || {
                column((
                    label(res::str::catalog_empty()).secondary(),
                    button(res::str::add_catalog()).prominent().action(move || {
                        a.catalog_editor.set(Some(catalog_editor::EditorRoute::Add))
                    }),
                ))
                .spacing(16.)
                .padding(24.)
            },
        ),
        list(catalogs, move |slot| {
            row((
                covers::favicon(move || opds::favicon_url(&slot.url().read())),
                column((
                    label(move || slot.title().read())
                        .font(Font::Headline)
                        .single_line(),
                    label(move || opds::catalog_host(&slot.url().read()))
                        .font(Font::Footnote)
                        .secondary()
                        .single_line(),
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
        .row_height(RowHeight::Automatic)
        .on_select(move |catalog| {
            a.browse_named(catalog.url().read(), true, catalog.title().read());
        })
        .id("catalog-sources")
        .grow(),
    ))
    .grow()
    .overlay(loading(a, 0))
    .any()
}
#[derive(Clone)]
enum CatalogRow {
    Link(opds::Link),
    Shelf(opds::Group),
    Book(Box<opds::Publication>),
    More,
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
            Self::Shelf(g) => g.title.clone(),
            Self::More => res::str::next_results().format(),
        }
    }
    fn subtitle(&self) -> String {
        match self {
            Self::Book(p) => p.author.clone(),
            Self::Link(l) => l.description.clone(),
            _ => String::new(),
        }
    }
    fn count(&self) -> String {
        match self {
            Self::Link(opds::Link { count: Some(n), .. }) => day::format_decimal(*n as f64, 0),
            _ => String::new(),
        }
    }
}
/// A grouped feed is a page of shelves. Its navigation links sit above them as buttons, and
/// any loose publications form one more shelf, so every row keeps the shelf height.
fn feed_rows(feed: &opds::Feed) -> Vec<CatalogRow> {
    if !feed.groups.is_empty() {
        let mut rows: Vec<CatalogRow> =
            feed.groups.iter().cloned().map(CatalogRow::Shelf).collect();
        if !feed.entries.is_empty() {
            rows.push(CatalogRow::Shelf(opds::Group {
                title: feed.title.clone(),
                total: None,
                more: feed.next.clone(),
                entries: feed.entries.clone(),
            }));
        }
        return rows;
    }
    let mut rows: Vec<CatalogRow> = feed
        .navigation
        .iter()
        .cloned()
        .map(CatalogRow::Link)
        .collect();
    rows.extend(
        feed.entries
            .iter()
            .cloned()
            .map(|p| CatalogRow::Book(Box::new(p))),
    );
    if feed.next.is_some() {
        rows.push(CatalogRow::More);
    }
    rows
}
/// The feed's size and any refinement already applied, such as "Sort By: Newest".
fn feed_summary(feed: &opds::Feed) -> String {
    let count = match feed.total {
        Some(n) => res::str::catalog_total(day::format_decimal(n as f64, 0)).format(),
        // Shelves hold samples; each shelf states its own size.
        None if !feed.groups.is_empty() => String::new(),
        None => {
            res::str::catalog_counts(feed.book_count() as i64, feed.link_count() as i64).format()
        }
    };
    // The first facet of a group is its default (Any, or the catalog's usual order).
    let refinements: Vec<String> = feed
        .facets
        .iter()
        .filter_map(|g| {
            let i = g.links.iter().position(|l| l.active)?;
            (i > 0)
                .then(|| res::str::facet_choice(g.title.clone(), g.links[i].title.clone()).format())
        })
        .collect();
    std::iter::once(count)
        .chain(refinements)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" · ")
}
fn results(a: App, page_id: u64, feed: Rc<opds::Feed>, title: String, depth: usize) -> impl Piece {
    let list_id = format!("catalog-list-{depth}-{}", feed.title);
    let shelves = !feed.groups.is_empty();
    let navigation = if shelves {
        feed.navigation.clone()
    } else {
        Vec::new()
    };
    let rows = Signal::new(feed_rows(&feed));
    let next = Signal::new(feed.next.clone());
    // Signals are disposed before cleanups run; keep the handle outside the reactive graph.
    let more = Rc::new(RefCell::new(None::<day::TaskHandle>));
    let more_loading = Signal::new(false);
    Scope::current().on_cleanup({
        let more = more.clone();
        move || {
            if let Some(task) = more.borrow_mut().take() {
                task.abort()
            }
        }
    });
    // Further results join this page, so a long search remains one scrolling list.
    let load_more = move || {
        let Some(url) = next.get_untracked() else {
            return;
        };
        if more_loading.get_untracked() {
            return;
        }
        more_loading.set(true);
        let task = day::task(async move {
            match load_feed(url).await {
                Ok(page) => {
                    next.set(page.next.clone());
                    rows.update(|rows| {
                        rows.retain(|r| !matches!(r, CatalogRow::More));
                        rows.extend(
                            page.entries
                                .iter()
                                .cloned()
                                .map(|p| CatalogRow::Book(Box::new(p))),
                        );
                        if page.next.is_some() {
                            rows.push(CatalogRow::More);
                        }
                    });
                    a.append_results(page_id, page);
                }
                Err(error) => a.error(error),
            }
            more_loading.set(false);
        });
        *more.borrow_mut() = Some(task);
    };
    let first_visible = Signal::new(0usize);
    watch(move || first_visible.get(), {
        let load_more = load_more.clone();
        move |first, _| {
            if rows.get_untracked().len().saturating_sub(*first) < 12 {
                load_more()
            }
        }
    });
    let summary = feed_summary(&feed);
    let no_results = rows.get_untracked().is_empty();
    let icon = feed.icon.clone();
    column((
        row((
            covers::favicon(move || icon.clone()),
            column((
                label(title).font(Font::Headline).single_line(),
                label(summary)
                    .font(Font::Caption)
                    .secondary()
                    .id("catalog-summary"),
                when(
                    {
                        let description = feed.description.clone();
                        move || !description.is_empty()
                    },
                    {
                        let description = feed.description.clone();
                        move || {
                            label(description.clone())
                                .font(Font::Footnote)
                                .secondary()
                                .single_line()
                        }
                    },
                ),
            ))
            .align(HAlign::Leading)
            .spacing(4.)
            .grow(),
        ))
        .spacing(12.)
        .padding(16.),
        when(
            {
                let has_navigation = !navigation.is_empty();
                move || has_navigation
            },
            move || {
                let links = navigation
                    .clone()
                    .into_iter()
                    .enumerate()
                    .collect::<Vec<_>>();
                scroll(
                    row((each(
                        items(move || links.clone(), |(i, _): &(usize, opds::Link)| *i),
                        move |link| {
                            button(move || link.get().1.title)
                                .bordered()
                                .action(move || {
                                    let l = link.get().1;
                                    a.browse_named(l.url, false, l.title)
                                })
                                .id(format!("catalog-nav-{}", link.key()))
                        },
                    ),))
                    .spacing(8.)
                    .padding(Insets::symmetric(16., 0.)),
                )
                .horizontal()
                .height(44.)
            },
        ),
        when(
            move || no_results,
            || {
                label(res::str::catalog_no_results())
                    .secondary()
                    .padding(24.)
                    .id("catalog-empty")
            },
        ),
        list(
            items(
                move || rows.get().into_iter().enumerate().collect::<Vec<_>>(),
                |(i, _): &(usize, CatalogRow)| *i,
            ),
            move |slot| {
                when(
                    move || matches!(slot.get().1, CatalogRow::Shelf(_)),
                    move || {
                        shelf(a, move || match slot.get().1 {
                            CatalogRow::Shelf(g) => g,
                            _ => opds::Group::default(),
                        })
                    },
                )
                .otherwise(move || catalog_row(slot, more_loading))
            },
        )
        .on_select(move |i| match rows.get_untracked().get(i) {
            Some(CatalogRow::Link(l)) => a.browse_named(l.url.clone(), false, l.title.clone()),
            Some(CatalogRow::More) => load_more(),
            Some(CatalogRow::Book(p)) => a.select_publication((**p).clone()),
            // A shelf's books and its See All button handle their own taps.
            Some(CatalogRow::Shelf(_)) | None => {}
        })
        .row_height(RowHeight::Automatic)
        .first_visible_row(first_visible)
        .id(list_id)
        .grow(),
    ))
    .grow()
    .overlay(loading(a, depth))
}
fn catalog_row(
    slot: ItemSlot<(usize, CatalogRow), usize>,
    more_loading: Signal<bool>,
) -> impl Piece {
    row((
        when(
            move || matches!(slot.get().1, CatalogRow::Book(_)),
            move || covers::view(move || slot.get().1.cover(), 46., 64.),
        )
        .otherwise(move || {
            vector(res::vectors::catalog_folder)
                .frame(28., 28.)
                .padding(9.)
                .background(Color::hex(0xeee8da))
                .corner_radius(12.)
                .decorative()
        }),
        column((
            label(move || {
                if matches!(slot.get().1, CatalogRow::More) && more_loading.get() {
                    res::str::loading().format()
                } else {
                    slot.get().1.title()
                }
            })
            .font(Font::Headline)
            .single_line(),
            when(
                move || !slot.get().1.subtitle().is_empty(),
                move || {
                    label(move || slot.get().1.subtitle())
                        .font(Font::Footnote)
                        .secondary()
                        .single_line()
                },
            ),
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
        label(move || slot.get().1.count())
            .font(Font::Footnote)
            .secondary(),
        vector(res::vectors::chevron_forward)
            .frame(12., 24.)
            .decorative(),
    ))
    .spacing(12.)
    .padding(16.)
    .height(96.)
}
/// One group of a grouped feed: a heading with the shelf's size and See All, above a
/// horizontally scrolling row of covers.
fn shelf(a: App, group: impl Fn() -> opds::Group + Copy + 'static) -> impl Piece {
    column((
        row((
            column((
                label(move || group().title)
                    .font(Font::Title3)
                    .bold()
                    .single_line(),
                label(move || {
                    group()
                        .total
                        .map(|n| res::str::catalog_total(day::format_decimal(n as f64, 0)).format())
                        .unwrap_or_default()
                })
                .font(Font::Caption)
                .secondary(),
            ))
            .align(HAlign::Leading)
            .spacing(2.)
            .grow(),
            when(
                move || group().more.is_some(),
                move || {
                    button(res::str::see_all())
                        .action(move || {
                            let g = group();
                            if let Some(url) = g.more {
                                a.browse_named(url, false, g.title)
                            }
                        })
                        .id_of(move || format!("shelf-see-all-{}", group().title))
                },
            ),
        ))
        .spacing(12.)
        .padding(Insets::symmetric(16., 0.)),
        scroll(
            row((each(
                items(
                    move || group().entries.into_iter().enumerate().collect::<Vec<_>>(),
                    |(i, p): &(usize, opds::Publication)| format!("{i}:{}", p.title),
                ),
                move |book| {
                    column((
                        covers::view(move || book.get().1.cover, 96., 138.),
                        label(move || book.get().1.title)
                            .font(Font::Footnote)
                            .single_line(),
                        label(move || book.get().1.author)
                            .font(Font::Caption)
                            .secondary()
                            .single_line(),
                    ))
                    .spacing(4.)
                    .align(HAlign::Leading)
                    .width(96.)
                    .on_tap(move || a.select_publication(book.get().1))
                    .id_of(move || format!("shelf-book-{}-{}", group().title, book.get().0))
                },
            ),))
            .spacing(12.)
            .align(VAlign::Top)
            .padding(Insets::symmetric(16., 0.)),
        )
        .horizontal()
        .height(186.),
    ))
    .spacing(8.)
    .padding(Insets::symmetric(0., 12.))
    .height(250.)
}
pub(crate) async fn load_feed(url: String) -> Result<opds::Feed, AppError> {
    let (bytes, final_url) = fetch_bytes(&url, 8 * 1024 * 1024, None).await?;
    covers::background(move || opds::parse(&bytes, &final_url))
        .await
        .map_err(AppError::from)?
}
fn search(a: App, link: opds::Link, query: String) {
    let query = query.trim().to_owned();
    let request = CatalogRequest::Search(link, query.clone());
    a.load_catalog(
        false,
        false,
        res::str::search_results(query).format(),
        request.clone(),
        request.load(),
    );
}
/// How the catalog manager edits on this platform. iOS has the list editing mode (delete
/// controls, reorder grips); the other phones drag on a long press and, on Android, delete with a
/// swipe; the desktops and the web select a row and act on it from the bar under the list.
#[derive(Clone, Copy, PartialEq)]
enum ManagerStyle {
    Editing,
    Touch,
    Desktop,
}
impl ManagerStyle {
    fn current() -> Self {
        if capability(Cap::ListEditing) == Support::Native {
            Self::Editing
        } else if cfg!(any(target_os = "android", target_env = "ohos")) {
            Self::Touch
        } else {
            Self::Desktop
        }
    }
}
fn manager(a: App) -> impl Piece {
    let Some(db) = a.database.get_untracked() else {
        return label(res::str::database_unavailable()).any();
    };
    let style = ManagerStyle::current();
    let catalogs = db.catalog_query();
    // The desktop bar acts on the selected catalog, by URL so a reorder cannot retarget it.
    let selected = Signal::new(None::<String>);
    let present = catalogs.clone();
    let chosen = move || {
        let url = selected.get()?;
        present
            .try_collect()
            .ok()?
            .iter()
            .any(|c| c.url == url)
            .then_some(url)
    };
    let rows = list(catalogs, move |slot| {
        row((
            covers::favicon(move || opds::favicon_url(&slot.url().read())),
            column((
                label(move || slot.title().read())
                    .font(Font::Headline)
                    .single_line(),
                label(move || slot.url().read())
                    .font(Font::Footnote)
                    .secondary()
                    .single_line(),
            ))
            .spacing(3.)
            .align(HAlign::Leading)
            .grow_w(),
            when(
                move || style != ManagerStyle::Desktop,
                move || {
                    vector(res::vectors::chevron_forward)
                        .frame(12., 24.)
                        .decorative()
                },
            ),
        ))
        .spacing(12.)
        .padding(Insets::symmetric(16., 10.))
        .context_menu_fn(move |_| {
            let edit = slot.url().read();
            let remove = edit.clone();
            vec![
                menu_item(res::str::edit_catalog().format())
                    .icon(Symbol::Edit)
                    .id("catalog-edit")
                    .action(move || edit_catalog(a, edit.clone())),
                menu_item(res::str::delete().format())
                    .icon(Symbol::Delete)
                    .id("catalog-remove")
                    .action(move || remove_catalog(a, &remove)),
            ]
        })
        // Recycled cells rebind to other catalogs, so the id follows the slot.
        .id_of(move || format!("catalog-row-{}", slot.url().read()))
    })
    .row_height(RowHeight::Automatic)
    // A desktop table draws its grid through the empty space below the rows as well.
    .separators(style != ManagerStyle::Desktop)
    .reorderable(true)
    .on_reorder(move |from, to| {
        let mut c = match a.catalogs() {
            Ok(c) => c,
            Err(e) => return a.error(e),
        };
        if from < c.len() && to < c.len() {
            let moved = c.remove(from);
            c.insert(to, moved);
            a.save_catalogs(c);
        }
    })
    .deletable(true)
    .delete_label(res::str::delete().format())
    .on_delete(move |index| {
        let mut c = match a.catalogs() {
            Ok(c) => c,
            Err(e) => return a.error(e),
        };
        if index < c.len() {
            c.remove(index);
            a.save_catalogs(c);
        }
    })
    .editing(style == ManagerStyle::Editing)
    .on_selection(move |rows| selected.set(rows.first().map(|c| c.url().read())))
    .on_activate(move |c| edit_catalog(a, c.url().read()));
    // macOS has no delete swipe but does have the two-finger row actions Mail uses.
    let rows = if capability(Cap::ListDelete) != Support::Native
        && capability(Cap::ListSwipeActions) == Support::Native
    {
        rows.swipe_trailing(move |index| {
            vec![
                swipe_action(res::str::delete().format())
                    .destructive(true)
                    .symbol(Symbol::Delete)
                    .action(move || {
                        if let Some(c) = a.catalogs().ok().and_then(|c| c.get(index).cloned()) {
                            remove_catalog(a, &c.url);
                        }
                    }),
            ]
        })
    } else {
        rows
    };
    let rows = rows.id("catalog-manager-list").grow();
    match style {
        ManagerStyle::Editing => rows.any(),
        // Material's add control for a list screen is the floating action button.
        ManagerStyle::Touch => column((
            rows.overlay_aligned(
                Alignment::BottomTrailing,
                button(res::str::add_catalog())
                    .icon(Symbol::Add)
                    .icon_only()
                    .prominent()
                    .action(move || a.catalog_editor.set(Some(catalog_editor::EditorRoute::Add)))
                    .id("catalog-manager-add")
                    .frame(56., 56.)
                    .padding(16.),
            )
            .grow(),
            label(res::str::catalog_manager_touch_hint())
                .font(Font::Footnote)
                .secondary()
                .padding(16.),
        ))
        .grow()
        .any(),
        ManagerStyle::Desktop => {
            let chosen = Rc::new(chosen);
            let can_remove = chosen.clone();
            let can_edit = chosen.clone();
            let remove = chosen.clone();
            column((
                rows,
                divider(),
                row((
                    button(res::str::add_catalog())
                        .icon(Symbol::Add)
                        .icon_only()
                        .plain()
                        .action(move || {
                            a.catalog_editor.set(Some(catalog_editor::EditorRoute::Add))
                        })
                        .id("catalog-manager-add"),
                    button(res::str::delete())
                        .icon(Symbol::Remove)
                        .icon_only()
                        .plain()
                        .enabled(move || can_remove().is_some())
                        .action(move || {
                            if let Some(url) = remove() {
                                remove_catalog(a, &url);
                            }
                        })
                        .id("catalog-manager-remove"),
                    button(res::str::edit_catalog())
                        .icon(Symbol::Edit)
                        .icon_only()
                        .plain()
                        .enabled(move || can_edit().is_some())
                        .action(move || {
                            if let Some(url) = chosen() {
                                edit_catalog(a, url);
                            }
                        })
                        .id("catalog-manager-edit"),
                    label(res::str::catalog_manager_desktop_hint())
                        .font(Font::Footnote)
                        .secondary()
                        .single_line()
                        .grow_w(),
                ))
                .spacing(6.)
                .padding(Insets::symmetric(12., 8.)),
            ))
            .grow()
            .any()
        }
    }
}
fn edit_catalog(a: App, url: String) {
    a.catalog_editor
        .set(Some(catalog_editor::EditorRoute::Edit(url)));
}
pub fn remove_catalog(a: App, url: &str) {
    let mut c = match a.catalogs() {
        Ok(c) => c,
        Err(e) => return a.error(e),
    };
    c.retain(|c| c.url != url);
    a.save_catalogs(c)
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

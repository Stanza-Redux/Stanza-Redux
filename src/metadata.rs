// SPDX-License-Identifier: AGPL-3.0-only WITH App-Fair-Distribution-Exception
use crate::*;
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Metadata {
    pub authors: Vec<String>,
    pub description: String,
    pub subtitle: String,
    pub subjects: Vec<String>,
    /// The catalog's shelves holding the book (Project Gutenberg's bookshelves).
    pub shelves: Vec<String>,
    /// The people besides its authors, with their roles, where the catalog names them.
    pub credits: Vec<Credit>,
    pub publisher: String,
    pub published: String,
    pub modified: String,
    pub languages: Vec<String>,
    pub identifiers: Vec<String>,
    pub contributors: Vec<String>,
    pub rights: String,
    pub series: String,
    pub pages: Option<u64>,
    pub bytes: Option<u64>,
    pub chapters: Option<u64>,
    pub acquisition: String,
}
/// One person's part in a book besides writing it: `role` is the OPDS 2 key (`translator`,
/// `editor`, `illustrator`, `artist`, `colorist`, `narrator`, `contributor`).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Credit {
    pub role: String,
    pub name: String,
}
/// A role as the reader's language names it.
fn role_label(role: &str) -> String {
    match role {
        "translator" => res::str::role_translator(),
        "editor" => res::str::role_editor(),
        "illustrator" => res::str::role_illustrator(),
        "artist" | "colorist" => res::str::role_artist(),
        "narrator" => res::str::role_narrator(),
        _ => res::str::contributors(),
    }
    .format()
}
impl Metadata {
    /// EPUB metadata wins; the catalog fills missing fields only.
    pub fn supplement(&mut self, other: &Self) {
        macro_rules! fill { ($($f:ident),*) => { $(if self.$f.is_empty() { self.$f = other.$f.clone(); })* }; }
        fill!(
            authors,
            description,
            subtitle,
            subjects,
            shelves,
            credits,
            publisher,
            published,
            modified,
            languages,
            identifiers,
            contributors,
            rights,
            series,
            acquisition
        );
        self.pages = self.pages.or(other.pages);
        self.bytes = self.bytes.or(other.bytes);
    }
}
// OPDS descriptions may be HTML or XHTML. Treat them as untrusted plain text, never executable markup.
pub fn plain(s: &str) -> String {
    let wrapped = format!("<div>{s}</div>");
    if let Ok(doc) = roxmltree::Document::parse(&wrapped) {
        return doc
            .descendants()
            .filter(|n| n.is_text())
            .filter_map(|n| n.text())
            .collect::<Vec<_>>()
            .join(" ")
            .trim()
            .into();
    }
    let mut tag = false;
    s.chars()
        .filter(|&c| {
            if c == '<' {
                tag = true;
                false
            } else if c == '>' {
                tag = false;
                false
            } else {
                !tag
            }
        })
        .collect::<String>()
        .replace("&amp;", "&")
        .replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}
pub fn panel(m: Metadata) -> impl Piece {
    // People by role, in the order the catalog lists them; without roles, one Contributors row.
    let mut credits: Vec<(String, String)> = Vec::new();
    for c in &m.credits {
        let role = role_label(&c.role);
        match credits.iter_mut().find(|(r, _)| *r == role) {
            Some((_, names)) => *names = format!("{names}, {}", c.name),
            None => credits.push((role, c.name.clone())),
        }
    }
    if credits.is_empty() {
        credits.push((res::str::contributors().format(), m.contributors.join(", ")));
    }
    let mut rows = vec![
        (res::str::genre().format(), m.subjects.join(" · ")),
        (res::str::bookshelves().format(), m.shelves.join(" · ")),
        (res::str::publisher().format(), m.publisher),
        (res::str::published().format(), m.published),
        (res::str::language().format(), m.languages.join(", ")),
        (res::str::series().format(), m.series),
        (
            res::str::identifier().format(),
            m.identifiers.join(
                "
",
            ),
        ),
        (res::str::rights().format(), m.rights),
        (res::str::updated().format(), m.modified),
        (
            res::str::format().format(),
            res::str::format_epub().format(),
        ),
    ];
    // After Series, where Contributors stood.
    let series = res::str::series().format();
    let at = rows
        .iter()
        .position(|(l, _)| *l == series)
        .map_or(rows.len(), |i| i + 1);
    rows.splice(at..at, credits);
    if let Some(n) = m.pages {
        rows.push((res::str::pages().format(), n.to_string()));
    }
    if let Some(n) = m.chapters {
        rows.push((res::str::chapters().format(), n.to_string()));
    }
    if let Some(n) = m.bytes {
        rows.push((
            res::str::file_size().format(),
            res::str::file_size_mb(day::format_decimal(n as f64 / 1048576., 1)).format(),
        ));
    }
    rows.retain(|(_, v)| !v.trim().is_empty());
    let has_description = !m.description.is_empty();
    column((
        has_description.then(|| {
            column((
                label(res::str::about_book()).font(Font::Title).bold(),
                label(plain(&m.description)),
            ))
            .align(HAlign::Leading)
            .spacing(Space::M)
        }),
        section((each(
            items(move || rows.clone(), |r: &(String, String)| r.0.clone()),
            |r| {
                column((
                    label(move || r.get().0).font(Font::Footnote).secondary(),
                    label(move || r.get().1),
                ))
                .align(HAlign::Leading)
                .spacing(Space::XS)
                .padding(Space::XS)
            },
        ),))
        .title(res::str::book_information()),
    ))
    .align(HAlign::Leading)
    .spacing(Space::XL)
    .id("book-metadata")
}

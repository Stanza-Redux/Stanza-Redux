// SPDX-License-Identifier: AGPL-3.0-only WITH App-Fair-Distribution-Exception
use crate::*;
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Metadata {
    pub authors: Vec<String>,
    pub description: String,
    pub subtitle: String,
    pub subjects: Vec<String>,
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
impl Metadata {
    /// EPUB metadata wins; the catalog fills missing fields only.
    pub fn supplement(&mut self, other: &Self) {
        macro_rules! fill { ($($f:ident),*) => { $(if self.$f.is_empty() { self.$f = other.$f.clone(); })* }; }
        fill!(
            authors,
            description,
            subtitle,
            subjects,
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
    let mut rows = vec![
        (res::str::genre().format(), m.subjects.join(" · ")),
        (res::str::publisher().format(), m.publisher),
        (res::str::published().format(), m.published),
        (res::str::language().format(), m.languages.join(", ")),
        (res::str::series().format(), m.series),
        (res::str::contributors().format(), m.contributors.join(", ")),
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
        when(
            move || has_description,
            move || {
                column((
                    label(res::str::about_book()).font(Font::Title).bold(),
                    label(plain(&m.description)),
                ))
                .align(HAlign::Leading)
                .spacing(12.)
            },
        ),
        section((each(
            items(move || rows.clone(), |r: &(String, String)| r.0.clone()),
            |r| {
                column((
                    label(move || r.get().0).font(Font::Footnote).secondary(),
                    label(move || r.get().1),
                ))
                .align(HAlign::Leading)
                .spacing(4.)
                .padding(4.)
            },
        ),))
        .title(res::str::book_information()),
    ))
    .align(HAlign::Leading)
    .spacing(24.)
    .id("book-metadata")
}

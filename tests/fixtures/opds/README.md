# OPDS feed samples

Responses from two public catalogs, fetched 2026-10-14 and trimmed to a few publications and
facet links each. Every remaining field is unmodified. The parser tests in `src/opds.rs` read
these files to check the shapes the live catalogs publish.

| File | Source |
|---|---|
| `gutenberg-root.json` | `https://opds-test.pglaf.org/opds/` (Project Gutenberg's OPDS 2 preview) |
| `gutenberg-search.json` | `https://opds-test.pglaf.org/opds/search?query=dickens` |
| `gutenberg-publication.json` | `https://opds-test.pglaf.org/opds/publications?id=79774` |
| `gutenberg-publication-roles.json` | `https://opds-test.pglaf.org/opds/publications?id=79827` (fetched 2026-10-20, untrimmed) |
| `gutenberg-publication-shelves.json` | `https://opds-test.pglaf.org/opds/publications?id=2701` (fetched 2026-10-20, untrimmed) |
| `archive-root.json` | `https://archive.org/services/opds` |
| `archive-search.json` | `https://archive.org/services/opds/catalog?query=pride%20and%20prejudice%20austen&type=search` |

The records are catalog metadata (titles, creators, subjects and links) published by Project
Gutenberg and the Internet Archive. They are test inputs only and are not bundled in the app.

app_title = Stanza Redux
language_name = English
library = Library
catalogs = Catalogs
reader = Read
settings = Settings
error = Unable to complete:
loading = Loading…
downloading = Downloading…
chapter = Chapter
cancel = Cancel
library_title = Your next chapter
library_hint = Downloaded books stay on this device, ready to read offline.
read = Read
delete = Remove
catalog_url = OPDS catalog URL
go = Open
save = Save catalog
back = Back
search = Search
details = Book details
next_results = Next results
download = Download EPUB
epub_only = Unencrypted EPUB books are supported.
previous = Previous
next = Next
reader_empty = Choose a book from your library or download one from a catalog.
font_size = Text size
margins = Side margins
vertical_margin = Vertical margin
line_height = Line spacing
theme = Theme
light = Light
sepia = Sepia
dark = Dark
font = Typeface
columns = Columns
columns_hint = Two columns are used on wide windows; narrow screens always show one.
import = Import EPUB
too_large = This book exceeds the 64 MB limit.
serif = Serif
sans = Sans serif
mono = Monospace
font_montserrat = Montserrat
font_noto_sans = Noto Sans
font_noto_serif = Noto Serif
custom = Custom
paper_color = Custom paper color
ink_color = Custom text color
custom_hint = Choose the Custom theme to use these colors.
back_library = Library
contents = Contents
done = Done
reader_gestures = Tap the edges or swipe to turn pages. Tap the middle for controls.
view_menu = View
manage = Manage
search_library = Search your library
recent = Recently added
title_sort = Title
author_sort = Author
language = Language

database_unavailable = Database unavailable. Restart the app or check available storage.

forward = Forward
browse = Browse
manage_catalogs = Manage catalogs
catalog_name = Catalog name
add_catalog = Add OPDS catalog
catalog_exists = This catalog is already in your list.
remove_catalog = Remove catalog
catalog_manager_touch_hint = Touch and hold a catalog to move it. Tap it to edit or remove it.
catalog_manager_desktop_hint = Drag catalogs to reorder them. Double-click one to edit it.
choose_publication = Select a book to see its details.

genre = Genre
publisher = Publisher
published = Published
series = Series
contributors = Contributors
identifier = Identifier
rights = Rights
updated = Updated
format = Format
pages = Pages
chapters = Chapters
file_size = File size
about_book = About this book
book_information = Book information
open_book = Open book
justified = Justified text
hyphenation = Hyphenation
hide_status_bar = Hide status bar
left_tap_advances = Left tap advances
paragraph_spacing = Paragraph spacing
typography = Typography
page_layout = Page layout
settings_reading = Reading
settings_reader_theme = Reader theme
settings_controls = Controls

# App-owned text, including reader chrome and error messages.
error_status = Unable to complete: { $message }
operation_failed = The operation could not be completed. Please try again.
network_failed = Unable to connect. Check your connection and try again.
http_failed = The server returned HTTP { $status }.
download_limit = The download exceeds the allowed size.
download_progress = Downloading… { $received } MB
download_progress_total = Downloading… { $received } / { $total } MB
file_size_mb = { $size } MB
format_epub = EPUB
column_one = 1
column_two = 2
language_english = English
language_french = Français
catalog_standard = Standard Ebooks
catalog_gutenberg = Project Gutenberg
catalog_ebooks_gratuits = Ebooks Gratuits
position_text = Chapter { $chapter }/{ $chapters } · { $page }/{ $pages }
appearance_glyph = Aa
epub_invalid = This file is not a valid EPUB book.
epub_resources_limit = This EPUB contains too many resources.
epub_unsafe_path = This EPUB contains an unsafe resource path.
epub_size_limit = This EPUB exceeds the resource size limit.
epub_resource_missing = A resource required by this EPUB is missing.
epub_package_missing = The EPUB package document is missing.
epub_chapter_missing = A chapter required by this EPUB is missing.
epub_no_chapters = This EPUB has no readable chapters.
epub_encryption_invalid = This EPUB contains invalid encryption information.
epub_font_invalid = An embedded font in this EPUB is invalid or missing.
epub_drm = DRM-encrypted EPUB books are not supported.
catalog_http_only = Catalog links must use HTTP or HTTPS.
catalog_invalid = This document is not a valid OPDS catalog.
search_invalid = This catalog’s search format is not supported.
search_missing = This catalog does not provide a search template.
legacy_import_failed = Some books from the previous version could not be imported.
sample_unavailable = The bundled sample book could not be opened.

library_recents = Recents
library_titles = Titles
library_authors = Authors
library_genres = Genres
library_recents_summary = { $n ->
    [0] No books opened yet
    [one] { $n } recently opened book
   *[other] { $n } recently opened books
}
library_titles_summary = { $n ->
    [0] No books yet
    [one] { $n } book
   *[other] { $n } books
}
library_authors_summary = { $n ->
    [0] No authors yet
    [one] { $n } author
   *[other] { $n } authors
}
library_genres_summary = { $n ->
    [0] No genres yet
    [one] { $n } genre
   *[other] { $n } genres
}
unknown_author = Unknown author
unknown_genre = Unclassified
library_no_results = No books here yet.
library_recents_hint = Your 50 most recently opened books appear here.
library_select_book = Select a book to see its details.
remove_book_question = Remove this book from your library?

edit_catalog = Edit catalog
validate_catalog = Validate
catalog_validation = Catalog information
catalog_validating = Checking catalog…
catalog_validated = Catalog verified
catalog_counts = { $books } books · { $links } browsing links
catalog_title = Published title
catalog_updated = Updated
catalog_description = Description
catalog_title_hint = Choose the name shown in your catalog list.

file_menu = File
open_book_file = Open Book…
increase_font = Increase Font Size
decrease_font = Decrease Font Size
book_import_failed = Could not import the book.

bookmark_add = Bookmark this page
bookmark_remove = Remove bookmark
bookmarks = Bookmarks
bookmarks_empty = No bookmarks yet. Use the bookmark button to save a page.
chapter_position = Position in chapter
return_position = Return to previous position
contents_search = Find a chapter
contents_empty = No matching chapters.
chapter_percent = { $percent } through chapter

# Catalog discovery and recovery.
catalog_heading = Discover your next book
catalog_hint = Browse independent catalogs and bring a new story to your library.
catalog_empty = Add a catalog to start exploring.
catalog_no_results = No results here. Try another search or go back to browse.
retry = Try again
refresh = Refresh

# Catalog shelves, facets and book availability.
see_all = See all
catalog_total = { $count } books
search_results = Results for “{ $query }”
sort_filter = Sort and filter
facet_count = { $facet } ({ $count })
facet_choice = { $group }: { $facet }
more_by = More by { $author }
epub_checking = Checking for an EPUB edition…
unavailable_account = This catalog lends or sells this book through an account. Stanza Redux downloads open EPUB books only.
unavailable_pdf = This catalog offers this book only as a PDF.
unavailable_audio = This catalog offers this book only as an audiobook.
unavailable_other = This catalog offers no EPUB edition of this book.
view_on_web = View on website

// Copyright 2025 The App Fair Project
// SPDX-License-Identifier: GPL-2.0-or-later

import SwiftUI
import StanzaModel
#if !SKIP
import UIKit
#endif

enum TOCTab: String, CaseIterable {
    case contents = "Contents"
    case bookmarks = "Bookmarks"
}

struct BookLocationsBrowser: View {
    let publication: Pub
    @State var bookmarks: [BookmarkRecord]
    let currentLocator: Loc?
    let database: BookDatabase?
    let bookID: Int64
    let onNavigateToTOC: (Lnk) -> Void
    let onNavigateToBookmark: (BookmarkRecord) -> Void
    let onBookmarksChanged: () -> Void
    let onDismiss: () -> Void
    @State var selectedTab: TOCTab = .contents
    @State var editingBookmark: BookmarkRecord? = nil
    @State var showEditSheet: Bool = false

    /// The title of the chapter the reader is currently in.
    var currentChapterTitle: String? {
        currentLocator?.title
    }

    var body: some View {
        NavigationStack {
            VStack(spacing: 0) {
                Picker("View", selection: $selectedTab) {
                    ForEach(TOCTab.allCases, id: \.self) { tab in
                        Text(tab.rawValue).tag(tab)
                    }
                }
                .pickerStyle(.segmented)
                .padding(.horizontal)
                .padding(.vertical, 8)
                .accessibilityIdentifier("tocTabPicker")

                if selectedTab == .contents {
                    tocList
                } else {
                    bookmarksList
                }
            }
            .navigationTitle(selectedTab == .contents ? "Table of Contents" : "Bookmarks")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Done") {
                        onDismiss()
                    }
                    .accessibilityIdentifier("tocDoneButton")
                }
            }
            .sheet(isPresented: $showEditSheet) {
                if let bookmark = editingBookmark {
                    BookmarkEditSheet(bookmark: bookmark, database: database) { updated in
                        refreshBookmarks()
                        onBookmarksChanged()
                    }
                }
            }
        }
    }

    /// Whether the given TOC link matches the current reading position.
    func isCurrentChapter(_ link: Lnk) -> Bool {
        guard let current = currentChapterTitle, let linkTitle = link.title else { return false }
        return current == linkTitle
    }

    @ViewBuilder var tocList: some View {
        List {
            ForEach(Array(publication.manifest.tableOfContents.enumerated()), id: \.offset) { index, link in
                Button {
                    onNavigateToTOC(link)
                } label: {
                    Text(link.title ?? "Chapter \(index + 1)")
                        .foregroundStyle(.primary)
                }
                .buttonStyle(.plain)
                .listRowBackground(isCurrentChapter(link) ? Color.accentColor.opacity(0.12) : nil)
                if !link.children.isEmpty {
                    ForEach(Array(link.children.enumerated()), id: \.offset) { childIndex, child in
                        Button {
                            onNavigateToTOC(child)
                        } label: {
                            Text(child.title ?? "Section \(childIndex + 1)")
                                .foregroundStyle(.primary)
                                .padding(.leading, 20)
                        }
                        .buttonStyle(.plain)
                        .listRowBackground(isCurrentChapter(child) ? Color.accentColor.opacity(0.12) : nil)
                    }
                }
            }
        }
    }

    @ViewBuilder var bookmarksList: some View {
        if bookmarks.isEmpty {
            VStack(spacing: 12) {
                Image("bookmark", bundle: .module)
                    .font(.system(size: 48))
                    .foregroundStyle(.secondary)
                    .accessibilityIdentifier("noBookmarksIcon")
                    .accessibilityLabel("No bookmarks")
                Text("No Bookmarks")
                    .font(.title3)
                    .foregroundStyle(.secondary)
                    .accessibilityIdentifier("noBookmarksTitle")
                Text("Tap the bookmark icon while reading to add one.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .multilineTextAlignment(.center)
                    .padding(.horizontal)
                    .accessibilityIdentifier("noBookmarksMessage")
            }
            .frame(maxHeight: .infinity)
        } else {
            List {
                ForEach(Array(bookmarks.enumerated()), id: \.offset) { index, bookmark in
                    Button {
                        onNavigateToBookmark(bookmark)
                    } label: {
                        VStack(alignment: .leading, spacing: 4) {
                            HStack {
                                Text(bookmark.chapter.isEmpty ? "Bookmark" : bookmark.chapter)
                                    .font(.headline)
                                    .foregroundStyle(.primary)
                                    .lineLimit(1)
                                Spacer()
                                Text(bookmark.progressLabel)
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                            }
                            if !bookmark.excerpt.isEmpty {
                                Text(bookmark.excerpt)
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                                    .lineLimit(2)
                            }
                            if !bookmark.notes.isEmpty {
                                Text(bookmark.notes)
                                    .font(.caption2)
                                    .foregroundStyle(.secondary)
                                    .lineLimit(1)
                            }
                        }
                        .padding(.vertical, 2)
                    }
                    .buttonStyle(.plain)
                    // 'fun contextMenu(menuItems: () -> View): View' is deprecated. This API is not yet available in Skip. Consider placing it within a #if !SKIP block. You can file an issue against the owning library at https://github.com/skiptools, or see the library README for information on adding support.
                    #if !SKIP
                    .contextMenu {
                        Button {
                            shareBookmark(bookmark)
                        } label: {
                            Label(title: { Text("Share") }, icon: { Image("share", bundle: .module) })
                        }
                        .accessibilityIdentifier("shareBookmarkButton")
                        Button {
                            editingBookmark = bookmark
                            showEditSheet = true
                        } label: {
                            Label(title: { Text("Edit Notes") }, icon: { Image("edit", bundle: .module) })
                        }
                        .accessibilityIdentifier("editBookmarkButton")
                        Button(role: .destructive) {
                            deleteBookmark(bookmark)
                        } label: {
                            Label(title: { Text("Delete") }, icon: { Image("delete", bundle: .module) })
                        }
                        .accessibilityIdentifier("deleteBookmarkButton")
                    }
                    #endif
                }
                .onDelete { indices in
                    let sorted = Array(indices).sorted(by: >)
                    for index in sorted {
                        let bookmark = bookmarks[index]
                        deleteBookmark(bookmark)
                    }
                }
                .onMove { source, destination in
                    moveBookmarks(from: source, to: destination)
                }
            }
        }
    }

    func refreshBookmarks() {
        guard let db = database else { return }
        do {
            self.bookmarks = try db.bookmarks(forBookID: bookID)
        } catch {
            logger.error("Failed to refresh bookmarks: \(error)")
        }
    }

    func deleteBookmark(_ bookmark: BookmarkRecord) {
        guard let db = database else { return }
        do {
            try db.deleteBookmark(id: bookmark.id)
            refreshBookmarks()
            onBookmarksChanged()
        } catch {
            logger.error("Failed to delete bookmark: \(error)")
        }
    }

    func moveBookmarks(from source: IndexSet, to destination: Int) {
        guard let db = database else { return }
        var reordered = bookmarks
        reordered.move(fromOffsets: source, toOffset: destination)
        for i in reordered.indices {
            var bm = reordered[i]
            bm.sortOrder = Int64(i)
            do {
                try db.updateBookmark(bm)
            } catch {
                logger.error("Failed to reorder bookmark: \(error)")
            }
        }
        refreshBookmarks()
        onBookmarksChanged()
    }

    func shareBookmark(_ bookmark: BookmarkRecord) {
        var text = ""
        if !bookmark.chapter.isEmpty {
            text += bookmark.chapter + "\n"
        }
        text += "Progress: " + bookmark.progressLabel + "\n"
        if !bookmark.excerpt.isEmpty {
            text += "\"\(bookmark.excerpt)\"\n"
        }
        if !bookmark.notes.isEmpty {
            text += "Notes: " + bookmark.notes + "\n"
        }

        #if !SKIP
        let activityVC = UIActivityViewController(activityItems: [text], applicationActivities: nil)
        if let windowScene = UIApplication.shared.connectedScenes.first as? UIWindowScene,
           let rootVC = windowScene.windows.first?.rootViewController {
            var topVC = rootVC
            while let presented = topVC.presentedViewController {
                topVC = presented
            }
            topVC.present(activityVC, animated: true)
        }
        #else
        let context = ProcessInfo.processInfo.androidContext
        let intent = android.content.Intent(android.content.Intent.ACTION_SEND)
        intent.setType("text/plain")
        intent.putExtra(android.content.Intent.EXTRA_TEXT, text)
        let chooser = android.content.Intent.createChooser(intent, "Share Bookmark")
        chooser.addFlags(android.content.Intent.FLAG_ACTIVITY_NEW_TASK)
        context.startActivity(chooser)
        #endif
    }
}

struct BookmarkEditSheet: View {
    let bookmark: BookmarkRecord
    let database: BookDatabase?
    let onSave: (BookmarkRecord) -> Void
    @State var editNotes: String
    @Environment(\.dismiss) var dismiss

    init(bookmark: BookmarkRecord, database: BookDatabase?, onSave: @escaping (BookmarkRecord) -> Void) {
        self.bookmark = bookmark
        self.database = database
        self.onSave = onSave
        self._editNotes = State(initialValue: bookmark.notes)
    }

    var body: some View {
        NavigationStack {
            Form {
                Section("Bookmark Info") {
                    if !bookmark.chapter.isEmpty {
                        HStack {
                            Text("Chapter")
                            Spacer()
                            Text(bookmark.chapter)
                                .foregroundStyle(.secondary)
                        }
                    }
                    HStack {
                        Text("Progress")
                        Spacer()
                        Text(bookmark.progressLabel)
                            .foregroundStyle(.secondary)
                    }
                    if !bookmark.excerpt.isEmpty {
                        VStack(alignment: .leading, spacing: 4) {
                            Text("Excerpt")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                            Text(bookmark.excerpt)
                                .font(.body)
                        }
                    }
                }
                Section("Notes") {
                    TextField("Add notes...", text: $editNotes)
                        .accessibilityIdentifier("bookmarkNotesField")
                }
            }
            .navigationTitle("Edit Bookmark")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                        .accessibilityIdentifier("bookmarkEditCancelButton")
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Save") {
                        saveNotes()
                    }
                    .accessibilityIdentifier("bookmarkEditSaveButton")
                }
            }
        }
    }

    func saveNotes() {
        guard let db = database else { return }
        var updated = bookmark
        updated.notes = editNotes
        do {
            try db.updateBookmark(updated)
            logger.info("Updated bookmark notes for id=\(bookmark.id)")
            onSave(updated)
            dismiss()
        } catch {
            logger.error("Failed to save bookmark notes: \(error)")
        }
    }
}

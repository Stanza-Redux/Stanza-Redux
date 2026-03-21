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

    /// Flattens the TOC hierarchy into a single list with depth for indentation.
    var flatTOCEntries: [(id: String, link: Lnk, depth: Int)] {
        var entries: [(id: String, link: Lnk, depth: Int)] = []
        for (index, link) in publication.manifest.tableOfContents.enumerated() {
            entries.append((id: "toc_\(index)", link: link, depth: 0))
            for (childIndex, child) in link.children.enumerated() {
                entries.append((id: "toc_\(index)_\(childIndex)", link: child, depth: 1))
            }
        }
        return entries
    }

    @ViewBuilder var tocList: some View {
        List {
            ForEach(flatTOCEntries, id: \.id) { entry in
                Button {
                    onNavigateToTOC(entry.link)
                } label: {
                    Text(entry.link.title ?? "Chapter")
                        .foregroundStyle(.primary)
                        .padding(.leading, CGFloat(entry.depth * 20))
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
                .buttonStyle(.plain)
                #if !SKIP // needed to make the entire area tappable on iOS
                .contentShape(Rectangle())
                #endif
                .listRowBackground(isCurrentChapter(entry.link) ? Color.accentColor.opacity(0.12) : nil)
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
                    #if !SKIP // needed to make the entire area tappable on iOS
                    .contentShape(Rectangle())
                    #endif
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

// Copyright 2025 The App Fair Project
// SPDX-License-Identifier: GPL-2.0-or-later

import SwiftUI
import StanzaModel

struct BookDetailView: View {
    let bookID: Int64
    let database: BookDatabase?
    var onUpdate: (() -> Void)? = nil
    @State var book: BookRecord? = nil
    @State var isEditing = false

    var body: some View {
        Group {
            if let book = book {
                List {
                    Section("Book Info") {
                        HStack {
                            Text("Title")
                            Spacer()
                            Text(book.title).foregroundStyle(.secondary)
                        }
                        if !book.author.isEmpty {
                            HStack {
                                Text("Author")
                                Spacer()
                                Text(book.author).foregroundStyle(.secondary)
                            }
                        }
                        if let identifier = book.identifier {
                            HStack {
                                Text("Identifier")
                                Spacer()
                                Text(identifier).foregroundStyle(.secondary)
                            }
                        }
                    }
                    Section("Progress") {
                        HStack {
                            Text("Chapters")
                            Spacer()
                            Text("\(book.currentItem)/\(book.totalItems)").foregroundStyle(.secondary)
                        }
                        ProgressView(value: book.progress)
                        HStack {
                            Text("Progress")
                            Spacer()
                            Text("\(Int(book.progress * 100))%").foregroundStyle(.secondary)
                        }
                        if let dateOpened = book.dateLastOpened {
                            HStack {
                                Text("Last Opened")
                                Spacer()
                                Text(dateOpened.description).foregroundStyle(.secondary)
                            }
                        }
                    }
                    Section("File") {
                        Text(book.filePath)
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                }
                .navigationTitle(book.title)
                .toolbar {
                    ToolbarItem(placement: .automatic) {
                        Button("Edit") {
                            isEditing = true
                        }
                        .accessibilityIdentifier("editBookButton")
                    }
                }
                .sheet(isPresented: $isEditing) {
                    BookEditView(book: book, database: database) { updatedBook in
                        self.book = updatedBook
                        onUpdate?()
                    }
                }
            } else {
                Text("Book not found")
            }
        }
        .task {
            do {
                self.book = try database?.book(id: bookID)
            } catch {
                logger.error("Failed to load book: \(error)")
            }
        }
    }
}

struct BookEditView: View {
    @State var editTitle: String
    @State var editAuthor: String
    @State var editIdentifier: String
    let bookID: Int64
    let database: BookDatabase?
    let onSave: (BookRecord) -> Void
    @Environment(\.dismiss) var dismiss

    init(book: BookRecord, database: BookDatabase?, onSave: @escaping (BookRecord) -> Void) {
        self._editTitle = State(initialValue: book.title)
        self._editAuthor = State(initialValue: book.author)
        self._editIdentifier = State(initialValue: book.identifier ?? "")
        self.bookID = book.id
        self.database = database
        self.onSave = onSave
    }

    var body: some View {
        NavigationStack {
            Form {
                Section("Metadata") {
                    TextField("Title", text: $editTitle)
                        .accessibilityIdentifier("editTitleField")
                    TextField("Author", text: $editAuthor)
                        .accessibilityIdentifier("editAuthorField")
                    TextField("Identifier", text: $editIdentifier)
                        .accessibilityIdentifier("editIdentifierField")
                }
            }
            .navigationTitle("Edit Book")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                        .accessibilityIdentifier("editCancelButton")
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Save") { saveChanges() }
                        .accessibilityIdentifier("editSaveButton")
                }
            }
        }
    }

    private func saveChanges() {
        guard let db = database else { return }
        logger.info("Saving book edits for id=\(bookID): title='\(editTitle)', author='\(editAuthor)'")
        do {
            guard var record = try db.book(id: bookID) else { return }
            record.title = editTitle
            record.author = editAuthor
            record.identifier = editIdentifier.isEmpty ? nil : editIdentifier
            try db.updateBook(record)
            logger.info("Book edits saved successfully")
            onSave(record)
            dismiss()
        } catch {
            logger.error("Failed to save book: \(error)")
        }
    }
}

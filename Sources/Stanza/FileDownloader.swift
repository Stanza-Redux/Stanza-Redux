// Copyright 2025 The App Fair Project
// SPDX-License-Identifier: GPL-2.0-or-later

import Foundation
import SwiftUI
import OSLog
import Observation

let downloadLogger = Logger(subsystem: "FileDownloader", category: "FileDownloader")

// MARK: - FileDownloader

/// The current state of a file download.
public enum FileDownloadState {
    /// No download has started.
    case idle
    /// The download is in progress.
    case downloading
    /// The download completed successfully.
    case completed
    /// The download failed with an error message.
    case failed(String)
    /// The download was cancelled.
    case cancelled
}

/// An observable model that manages downloading a file from a URL to a local destination.
///
/// On iOS, uses `URLSessionDownloadTask` with a delegate for progress reporting.
/// On Android, uses `HttpURLConnection` with buffered I/O.
///
/// This is a generic utility with no app-specific dependencies.
@Observable public class FileDownloader {
    /// The current state of the download.
    public var state: FileDownloadState = .idle

    /// Download progress from 0.0 to 1.0. Negative if total size is unknown.
    public var progress: Double = 0.0

    /// Number of bytes received so far.
    public var bytesReceived: Int64 = 0

    /// Total expected bytes, or -1 if unknown.
    public var bytesTotal: Int64 = -1

    /// The source URL being downloaded.
    public let sourceURL: URL

    /// The local destination URL for the downloaded file.
    public let destinationURL: URL

    /// A display name for the file being downloaded.
    public let displayName: String

    #if !SKIP
    private var downloadTask: URLSessionDownloadTask?
    private var delegate: DownloadDelegate?
    #endif

    /// Creates a new file downloader.
    /// - Parameters:
    ///   - sourceURL: The remote URL to download.
    ///   - destinationURL: The local file URL where the download will be saved.
    ///   - displayName: A human-readable name for the download (e.g. the filename).
    public init(sourceURL: URL, destinationURL: URL, displayName: String = "") {
        self.sourceURL = sourceURL
        self.destinationURL = destinationURL
        self.displayName = displayName.isEmpty ? (sourceURL.lastPathComponent) : displayName
    }

    /// Starts the download. Only starts from idle state; call `reset()` first to retry.
    public func start() {
        guard case .idle = state else { return }
        startDownload()
    }

    private func startDownload() {
        downloadLogger.info("Starting download: \(self.sourceURL.absoluteString) -> \(self.destinationURL.path)")
        state = .downloading
        progress = 0.0
        bytesReceived = 0
        bytesTotal = -1

        let dir = destinationURL.deletingLastPathComponent()
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)

        #if !SKIP
        startIOSDownload()
        #else
        startAndroidDownload()
        #endif
    }

    /// Cancels the current download.
    public func cancel() {
        downloadLogger.info("Cancelling download: \(self.displayName)")
        #if !SKIP
        downloadTask?.cancel()
        downloadTask = nil
        delegate = nil
        #endif
        state = .cancelled
    }

    /// Resets the downloader to idle state so it can be started again.
    public func reset() {
        cancel()
        state = .idle
        progress = 0.0
        bytesReceived = 0
        bytesTotal = -1
    }

    // MARK: - iOS Implementation

    #if !SKIP
    private func startIOSDownload() {
        let del = DownloadDelegate(downloader: self)
        self.delegate = del
        let session = URLSession(configuration: .default, delegate: del, delegateQueue: .main)
        let task = session.downloadTask(with: sourceURL)
        self.downloadTask = task
        task.resume()
    }

    fileprivate func handleProgress(bytesWritten: Int64, totalBytesWritten: Int64, totalBytesExpectedToWrite: Int64) {
        self.bytesReceived = totalBytesWritten
        self.bytesTotal = totalBytesExpectedToWrite > 0 ? totalBytesExpectedToWrite : -1
        if totalBytesExpectedToWrite > 0 {
            self.progress = Double(totalBytesWritten) / Double(totalBytesExpectedToWrite)
        }
    }

    fileprivate func handleCompletion(tempURL: URL?, error: Error?) {
        if let error = error {
            if (error as NSError).code == NSURLErrorCancelled {
                downloadLogger.info("Download cancelled: \(self.displayName)")
                state = .cancelled
            } else {
                downloadLogger.error("Download failed: \(error.localizedDescription)")
                state = .failed(error.localizedDescription)
            }
            return
        }
        guard let tempURL = tempURL else {
            downloadLogger.error("Download completed but no file URL")
            state = .failed("Download completed but no file was received")
            return
        }
        do {
            if FileManager.default.fileExists(atPath: destinationURL.path) {
                try FileManager.default.removeItem(at: destinationURL)
            }
            try FileManager.default.moveItem(at: tempURL, to: destinationURL)
            progress = 1.0
            downloadLogger.info("Download complete: \(self.bytesReceived) bytes -> \(self.destinationURL.path)")
            state = .completed
        } catch {
            downloadLogger.error("Failed to move downloaded file: \(error)")
            state = .failed("Failed to save file: \(error.localizedDescription)")
        }
        downloadTask = nil
        delegate = nil
    }
    #endif

    // MARK: - Android Implementation

    #if SKIP
    private func startAndroidDownload() {
        Task {
            do {
                let javaUrl = java.net.URL(sourceURL.absoluteString)
                let connection = javaUrl.openConnection() as! java.net.HttpURLConnection
                connection.requestMethod = "GET"
                connection.setRequestProperty("User-Agent", "Mozilla/5.0")
                let totalBytes = Int64(connection.contentLength)
                self.bytesTotal = totalBytes > 0 ? totalBytes : -1

                let inputStream = connection.inputStream
                let outputStream = java.io.FileOutputStream(destinationURL.path)
                let buffer = kotlin.ByteArray(8192)
                var received: Int64 = 0
                var bytesRead = inputStream.read(buffer)
                while bytesRead != -1 {
                    if case .cancelled = state {
                        inputStream.close()
                        outputStream.close()
                        connection.disconnect()
                        downloadLogger.info("Download cancelled: \(self.displayName)")
                        return
                    }
                    outputStream.write(buffer, 0, bytesRead)
                    received = received + Int64(bytesRead)
                    self.bytesReceived = received
                    if totalBytes > 0 {
                        self.progress = Double(received) / Double(totalBytes)
                    }
                    bytesRead = inputStream.read(buffer)
                }
                outputStream.close()
                inputStream.close()
                connection.disconnect()
                self.progress = 1.0
                downloadLogger.info("Download complete: \(received) bytes -> \(self.destinationURL.path)")
                self.state = .completed
            } catch {
                downloadLogger.error("Download failed: \(error)")
                self.state = .failed(error.localizedDescription)
            }
        }
    }
    #endif
}

// MARK: - iOS URLSession Delegate

#if !SKIP
private class DownloadDelegate: NSObject, URLSessionDownloadDelegate {
    weak var downloader: FileDownloader?

    init(downloader: FileDownloader) {
        self.downloader = downloader
    }

    func urlSession(_ session: URLSession, downloadTask: URLSessionDownloadTask, didFinishDownloadingTo location: URL) {
        downloader?.handleCompletion(tempURL: location, error: nil)
        session.finishTasksAndInvalidate()
    }

    func urlSession(_ session: URLSession, downloadTask: URLSessionDownloadTask, didWriteData bytesWritten: Int64, totalBytesWritten: Int64, totalBytesExpectedToWrite: Int64) {
        downloader?.handleProgress(bytesWritten: bytesWritten, totalBytesWritten: totalBytesWritten, totalBytesExpectedToWrite: totalBytesExpectedToWrite)
    }

    func urlSession(_ session: URLSession, task: URLSessionTask, didCompleteWithError error: Error?) {
        if let error = error {
            downloader?.handleCompletion(tempURL: nil, error: error)
            session.finishTasksAndInvalidate()
        }
    }
}
#endif

// MARK: - FileDownloadView

/// A reusable SwiftUI view that displays download progress with a cancel button and byte counts.
///
/// Displays different content based on the download state:
/// - **idle**: Shows the start button
/// - **downloading**: Shows progress bar, byte counts, and cancel button
/// - **completed**: Shows a completion message
/// - **failed**: Shows the error message with a retry option
/// - **cancelled**: Shows a retry option
public struct FileDownloadView: View {
    @Bindable var downloader: FileDownloader

    /// Label for the download button. Defaults to "Download".
    var downloadLabel: String

    /// Label for the cancel button. Defaults to "Cancel".
    var cancelLabel: String

    /// Label shown on completion. Defaults to "Download Complete".
    var completedLabel: String

    /// Optional action to run when the download completes.
    var onCompleted: (() -> Void)?

    /// Creates a file download view.
    /// - Parameters:
    ///   - downloader: The `FileDownloader` instance to observe.
    ///   - downloadLabel: Text for the download button.
    ///   - cancelLabel: Text for the cancel button.
    ///   - completedLabel: Text shown when download finishes.
    ///   - onCompleted: Optional callback when download completes.
    public init(
        downloader: FileDownloader,
        downloadLabel: String = "Download",
        cancelLabel: String = "Cancel",
        completedLabel: String = "Download Complete",
        onCompleted: (() -> Void)? = nil
    ) {
        self.downloader = downloader
        self.downloadLabel = downloadLabel
        self.cancelLabel = cancelLabel
        self.completedLabel = completedLabel
        self.onCompleted = onCompleted
    }

    public var body: some View {
        VStack(spacing: 8) {
            switch downloader.state {
            case .idle:
                idleView
            case .downloading:
                downloadingView
            case .completed:
                completedView
            case .failed(let message):
                failedView(message: message)
            case .cancelled:
                cancelledView
            }
        }
    }

    @ViewBuilder private var idleView: some View {
        Button {
            downloader.start()
        } label: {
            Label(downloadLabel, systemImage: "arrow.down.circle")
                .frame(maxWidth: .infinity)
        }
        .buttonStyle(.borderedProminent)
    }

    @ViewBuilder private var downloadingView: some View {
        VStack(spacing: 6) {
            ProgressView(value: downloader.progress >= 0.0 ? downloader.progress : nil)

            HStack {
                Text(downloader.displayName)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
                Spacer()
                Text(progressText)
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }

            Button(cancelLabel) {
                downloader.cancel()
            }
            .foregroundStyle(.red)
            .font(.caption)
        }
    }

    @ViewBuilder private var completedView: some View {
        Label(completedLabel, systemImage: "checkmark.circle.fill")
            .foregroundStyle(.green)
            .font(.headline)
            .onAppear {
                onCompleted?()
            }
    }

    @ViewBuilder private func failedView(message: String) -> some View {
        VStack(spacing: 4) {
            Label(message, systemImage: "exclamationmark.triangle")
                .foregroundStyle(.red)
                .font(.caption)
            Button("Retry") {
                downloader.reset()
                downloader.start()
            }
            .font(.caption)
        }
    }

    @ViewBuilder private var cancelledView: some View {
        VStack(spacing: 4) {
            Text("Download cancelled")
                .font(.caption)
                .foregroundStyle(.secondary)
            Button("Retry") {
                downloader.reset()
                downloader.start()
            }
            .font(.caption)
        }
    }

    /// Formatted progress text showing bytes received / total.
    private var progressText: String {
        let received = FileDownloadView.formatBytes(downloader.bytesReceived)
        if downloader.bytesTotal > 0 {
            let total = FileDownloadView.formatBytes(downloader.bytesTotal)
            let pct = Int(downloader.progress * 100)
            return "\(received) / \(total) (\(pct)%)"
        }
        return received
    }

    /// Formats a byte count into a human-readable string (KB, MB, GB).
    public static func formatBytes(_ bytes: Int64) -> String {
        if bytes < 1024 {
            return "\(bytes) B"
        } else if bytes < 1024 * 1024 {
            let kb = Double(bytes) / 1024.0
            return String(format: "%.0f KB", kb)
        } else if bytes < 1024 * 1024 * 1024 {
            let mb = Double(bytes) / (1024.0 * 1024.0)
            return String(format: "%.1f MB", mb)
        } else {
            let gb = Double(bytes) / (1024.0 * 1024.0 * 1024.0)
            return String(format: "%.2f GB", gb)
        }
    }
}

/// A compact inline download view suitable for use in a List row.
public struct FileDownloadRowView: View {
    @Bindable var downloader: FileDownloader

    /// Optional action to run when the download completes.
    var onCompleted: (() -> Void)?

    public init(downloader: FileDownloader, onCompleted: (() -> Void)? = nil) {
        self.downloader = downloader
        self.onCompleted = onCompleted
    }

    public var body: some View {
        HStack {
            VStack(alignment: .leading, spacing: 4) {
                Text(downloader.displayName)
                    .font(.subheadline)
                    .lineLimit(1)
                statusText
            }
            Spacer()
            actionButton
        }
        .padding(.vertical, 4)
    }

    @ViewBuilder private var statusText: some View {
        switch downloader.state {
        case .idle:
            Text("Ready to download")
                .font(.caption)
                .foregroundStyle(.secondary)
        case .downloading:
            HStack(spacing: 8) {
                ProgressView(value: downloader.progress >= 0.0 ? downloader.progress : nil)
                    .frame(maxWidth: 120)
                Text(compactProgress)
                    .font(.caption2)
                    .foregroundStyle(.secondary)
            }
        case .completed:
            Text("Downloaded")
                .font(.caption)
                .foregroundStyle(.green)
                .onAppear {
                    onCompleted?()
                }
        case .failed(let msg):
            Text(msg)
                .font(.caption)
                .foregroundStyle(.red)
                .lineLimit(1)
        case .cancelled:
            Text("Cancelled")
                .font(.caption)
                .foregroundStyle(.secondary)
        }
    }

    @ViewBuilder private var actionButton: some View {
        switch downloader.state {
        case .idle:
            Button {
                downloader.start()
            } label: {
                Image(systemName: "arrow.down.circle")
            }
        case .downloading:
            Button {
                downloader.cancel()
            } label: {
                Image(systemName: "xmark.circle")
                    .foregroundStyle(.red)
            }
        case .completed:
            Image(systemName: "checkmark.circle.fill")
                .foregroundStyle(.green)
        case .failed, .cancelled:
            Button {
                downloader.reset()
                downloader.start()
            } label: {
                Image(systemName: "arrow.clockwise.circle")
            }
        }
    }

    private var compactProgress: String {
        let received = FileDownloadView.formatBytes(downloader.bytesReceived)
        if downloader.bytesTotal > 0 {
            let total = FileDownloadView.formatBytes(downloader.bytesTotal)
            return "\(received)/\(total)"
        }
        return received
    }
}

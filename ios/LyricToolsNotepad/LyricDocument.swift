import Foundation
import Observation

/// One open lyric file.
///
/// Saves automatically a moment after you stop typing. If the file changes on
/// another device (say, the PC) while it's open here:
/// - no unsaved edits here -> the new version just loads;
/// - unsaved edits here -> `changedElsewhere` is set and the editor asks
///   which version to keep, instead of silently overwriting either one.
@MainActor
@Observable
final class LyricDocument {
    let url: URL
    private(set) var text = ""
    private(set) var isDirty = false
    private(set) var isLoaded = false
    var changedElsewhere = false
    var errorMessage: String?

    /// Called with the new text when the file was reloaded from disk.
    @ObservationIgnored var onReload: ((String) -> Void)?

    @ObservationIgnored private var newline = "\n"
    @ObservationIgnored private var bom = false
    @ObservationIgnored private var diskDate: Date?
    @ObservationIgnored private var saveTask: Task<Void, Never>?
    @ObservationIgnored private var watcher: FileWatcher?

    private static let autosaveDelay: Duration = .seconds(1.2)
    /// Synced folders can round modification times a little.
    private static let dateSlack: TimeInterval = 0.5

    var title: String { url.deletingPathExtension().lastPathComponent }

    init(url: URL) {
        self.url = url
    }

    func load() async {
        do {
            let url = url
            let data = try await Task.detached { try FileIO.read(url) }.value
            apply(decodeFile(data: data))
            isLoaded = true
            watcher = FileWatcher(url: url) { [weak self] in
                Task { @MainActor in self?.checkForExternalChange() }
            }
        } catch {
            errorMessage = "Couldn't open \(url.lastPathComponent): \(error.localizedDescription)"
        }
    }

    private func apply(_ file: DecodedFile) {
        text = file.text
        newline = file.newline
        bom = file.bom
        diskDate = FileIO.modificationDate(url)
        isDirty = false
    }

    /// The editor reports every edit here.
    func userEdited(_ newText: String) {
        guard newText != text else { return }
        text = newText
        isDirty = true
        saveTask?.cancel()
        saveTask = Task { [weak self] in
            try? await Task.sleep(for: LyricDocument.autosaveDelay)
            guard !Task.isCancelled else { return }
            self?.save()
        }
    }

    private var changedOnDisk: Bool {
        guard let disk = FileIO.modificationDate(url), let known = diskDate else { return false }
        return disk.timeIntervalSince(known) > LyricDocument.dateSlack
    }

    /// Save now (unless the file changed elsewhere since we loaded it).
    func save() {
        saveTask?.cancel()
        guard isDirty, isLoaded, !changedElsewhere else { return }
        if changedOnDisk {
            changedElsewhere = true
            return
        }
        write()
    }

    private func write() {
        do {
            try FileIO.write(encodeFile(text: text, newline: newline, bom: bom), to: url, presenter: watcher)
            diskDate = FileIO.modificationDate(url)
            isDirty = false
        } catch {
            errorMessage = "Couldn't save: \(error.localizedDescription)"
        }
    }

    func checkForExternalChange() {
        guard isLoaded, changedOnDisk else { return }
        if isDirty {
            changedElsewhere = true
        } else {
            reloadFromDisk()
        }
    }

    /// "Keep mine": overwrite the other device's version.
    func keepMine() {
        changedElsewhere = false
        write()
    }

    /// "Use theirs": drop local edits and load the other device's version.
    func useTheirs() {
        changedElsewhere = false
        reloadFromDisk()
    }

    private func reloadFromDisk() {
        guard let data = try? FileIO.read(url) else { return }
        apply(decodeFile(data: data))
        onReload?(text)
    }

    func close() {
        save()
        watcher?.stop()
        watcher = nil
    }
}

import Foundation
import Observation

/// One lyric file as the Home list shows it.
struct LyricItem: Identifiable, Hashable, Sendable {
    /// The real file URL (never an iCloud ".icloud" placeholder).
    let url: URL
    var title: String
    var sections: [String]
    var preview: [String]
    var modified: Date?
    /// False while iCloud still has to download it to this device.
    var isDownloaded: Bool

    var id: URL { url }
    var fileName: String { url.lastPathComponent }
}

/// The lyrics folder: which folder it is, what's in it, creating new lyrics.
///
/// By default lyrics live in the app's own folder ("On My iPhone"). To sync
/// with the PC, the user picks a folder in iCloud Drive (or OneDrive,
/// Dropbox...) and the Windows app points at the same folder.
@MainActor
@Observable
final class LyricLibrary {
    private(set) var folderURL: URL
    private(set) var usingCustomFolder: Bool
    private(set) var items: [LyricItem] = []
    private(set) var isScanning = false
    var errorMessage: String?

    private static let bookmarkKey = "lyricsFolderBookmark"

    static var localFolder: URL {
        FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0]
    }

    init() {
        if let data = UserDefaults.standard.data(forKey: LyricLibrary.bookmarkKey),
           let url = LyricLibrary.resolve(bookmark: data)
        {
            folderURL = url
            usingCustomFolder = true
        } else {
            folderURL = LyricLibrary.localFolder
            usingCustomFolder = false
        }
    }

    private static func resolve(bookmark: Data) -> URL? {
        var stale = false
        guard let url = try? URL(resolvingBookmarkData: bookmark, bookmarkDataIsStale: &stale),
              url.startAccessingSecurityScopedResource()
        else { return nil }
        if stale, let fresh = try? url.bookmarkData() {
            UserDefaults.standard.set(fresh, forKey: bookmarkKey)
        }
        return url
    }

    /// A short name for where lyrics are kept, e.g. "iCloud Drive › Lyrics".
    var folderDisplayName: String {
        guard usingCustomFolder else { return "On My iPhone › LyricToolsNotepad" }
        let path = folderURL.path
        if let range = path.range(of: "com~apple~CloudDocs") {
            let rest = path[range.upperBound...].split(separator: "/").joined(separator: " › ")
            return rest.isEmpty ? "iCloud Drive" : "iCloud Drive › \(rest)"
        }
        return folderURL.lastPathComponent
    }

    /// Use a folder the user picked in the Files picker.
    func setFolder(_ url: URL) {
        guard url.startAccessingSecurityScopedResource() else {
            errorMessage = "Couldn't open that folder."
            return
        }
        do {
            let bookmark = try url.bookmarkData()
            UserDefaults.standard.set(bookmark, forKey: LyricLibrary.bookmarkKey)
        } catch {
            url.stopAccessingSecurityScopedResource()
            errorMessage = "Couldn't remember that folder: \(error.localizedDescription)"
            return
        }
        if usingCustomFolder { folderURL.stopAccessingSecurityScopedResource() }
        folderURL = url
        usingCustomFolder = true
        items = []
    }

    func useLocalFolder() {
        if usingCustomFolder { folderURL.stopAccessingSecurityScopedResource() }
        UserDefaults.standard.removeObject(forKey: LyricLibrary.bookmarkKey)
        folderURL = LyricLibrary.localFolder
        usingCustomFolder = false
        items = []
    }

    func refresh() async {
        let folder = folderURL
        isScanning = true
        let found = await Task.detached(priority: .userInitiated) { LibraryScanner.scan(folder) }.value
        if folder == folderURL { items = found }
        isScanning = false
    }

    /// Create `<folder>/<title>.txt` from the template; returns its URL.
    func createLyric(title: String) throws -> URL {
        let url = LibraryScanner.uniqueURL(in: folderURL, name: fileNameFor(title: title))
        let data = encodeFile(text: newLyricTemplate(title: title), newline: "\n", bom: false)
        try FileIO.write(data, to: url)
        return url
    }
}

/// Folder scanning, off the main thread.
enum LibraryScanner {
    static let extensions: Set<String> = ["txt", "lyrics", "md"]
    private static let maxDepth = 3

    /// iCloud keeps not-yet-downloaded files as ".Name.txt.icloud".
    static func realName(forPlaceholder name: String) -> String? {
        guard name.hasPrefix("."), name.hasSuffix(".icloud"), name.count > ".icloud".count + 1 else { return nil }
        return String(name.dropFirst().dropLast(".icloud".count))
    }

    static func isLyricFile(_ name: String) -> Bool {
        extensions.contains((name as NSString).pathExtension.lowercased())
    }

    static func scan(_ folder: URL) -> [LyricItem] {
        var items: [LyricItem] = []
        collect(folder, depth: 0, into: &items)
        return items.sorted {
            ($0.modified ?? .distantPast, $1.title.lowercased()) > ($1.modified ?? .distantPast, $0.title.lowercased())
        }
    }

    private static func collect(_ dir: URL, depth: Int, into items: inout [LyricItem]) {
        let fm = FileManager.default
        guard let names = try? fm.contentsOfDirectory(atPath: dir.path) else { return }
        for name in names {
            let url = dir.appendingPathComponent(name)
            if let real = realName(forPlaceholder: name) {
                guard isLyricFile(real) else { continue }
                let realURL = dir.appendingPathComponent(real)
                try? fm.startDownloadingUbiquitousItem(at: realURL)
                let title = (real as NSString).deletingPathExtension
                items.append(LyricItem(url: realURL, title: title, sections: [], preview: [],
                                       modified: FileIO.modificationDate(url), isDownloaded: false))
                continue
            }
            if name.hasPrefix(".") { continue }
            var isDir: ObjCBool = false
            guard fm.fileExists(atPath: url.path, isDirectory: &isDir) else { continue }
            if isDir.boolValue {
                if depth + 1 < maxDepth { collect(url, depth: depth + 1, into: &items) }
            } else if isLyricFile(name) {
                items.append(card(for: url))
            }
        }
    }

    static func card(for url: URL) -> LyricItem {
        let stem = url.deletingPathExtension().lastPathComponent
        let text = (try? FileIO.read(url)).map { decodeFile(data: $0).text } ?? ""
        let summary = summarize(text: text, fallbackTitle: stem)
        return LyricItem(url: url, title: summary.title, sections: summary.sections, preview: summary.preview,
                         modified: FileIO.modificationDate(url), isDownloaded: true)
    }

    /// `dir/name.txt`, or `dir/name (2).txt` etc. if taken.
    static func uniqueURL(in dir: URL, name: String) -> URL {
        let first = dir.appendingPathComponent("\(name).txt")
        if !FileIO.exists(first) { return first }
        var n = 2
        while FileIO.exists(dir.appendingPathComponent("\(name) (\(n)).txt")) { n += 1 }
        return dir.appendingPathComponent("\(name) (\(n)).txt")
    }
}

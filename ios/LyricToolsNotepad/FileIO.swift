import Foundation

/// File access that plays well with iCloud Drive and other synced folders:
/// every read and write goes through NSFileCoordinator, so the sync service
/// never sees a half-written file and we never read one mid-download.
enum FileIO {
    struct CoordinationError: Error {}

    static func read(_ url: URL) throws -> Data {
        var coordinationError: NSError?
        var result: Result<Data, Error> = .failure(CoordinationError())
        NSFileCoordinator().coordinate(readingItemAt: url, options: [], error: &coordinationError) { readURL in
            result = Result { try Data(contentsOf: readURL) }
        }
        if let coordinationError { throw coordinationError }
        return try result.get()
    }

    static func write(_ data: Data, to url: URL, presenter: NSFilePresenter? = nil) throws {
        var coordinationError: NSError?
        var result: Result<Void, Error> = .failure(CoordinationError())
        NSFileCoordinator(filePresenter: presenter).coordinate(
            writingItemAt: url, options: .forReplacing, error: &coordinationError
        ) { writeURL in
            result = Result { try data.write(to: writeURL, options: .atomic) }
        }
        if let coordinationError { throw coordinationError }
        try result.get()
    }

    /// Fresh from disk every time (URL resource values are cached).
    static func modificationDate(_ url: URL) -> Date? {
        (try? FileManager.default.attributesOfItem(atPath: url.path))?[.modificationDate] as? Date
    }

    static func exists(_ url: URL) -> Bool {
        FileManager.default.fileExists(atPath: url.path)
    }
}

/// Watches one file for changes made elsewhere (e.g. synced from the PC).
final class FileWatcher: NSObject, NSFilePresenter {
    let presentedItemURL: URL?
    let presentedItemOperationQueue = OperationQueue.main
    private let onChange: () -> Void

    init(url: URL, onChange: @escaping () -> Void) {
        presentedItemURL = url
        self.onChange = onChange
        super.init()
        NSFileCoordinator.addFilePresenter(self)
    }

    func stop() {
        NSFileCoordinator.removeFilePresenter(self)
    }

    func presentedItemDidChange() {
        onChange()
    }
}

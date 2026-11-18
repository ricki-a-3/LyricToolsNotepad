import SwiftUI

@main
struct LyricToolsNotepadApp: App {
    @State private var library = LyricLibrary()
    @State private var settings = AppSettings()

    init() {
        // Load the pronunciation dictionary and build the rhyme indexes in
        // the background so the first highlighted word gets rhymes instantly.
        Task.detached(priority: .utility) { warmUp() }
    }

    var body: some Scene {
        WindowGroup {
            HomeView()
                .environment(library)
                .environment(settings)
        }
    }
}

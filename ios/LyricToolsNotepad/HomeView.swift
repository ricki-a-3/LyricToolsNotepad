import SwiftUI
import UIKit

/// Every lyric in the lyrics folder, newest first, with search.
struct HomeView: View {
    @Environment(LyricLibrary.self) private var library
    @Environment(\.scenePhase) private var scenePhase
    @State private var path: [URL] = []
    @State private var search = ""
    @State private var askTitle = false
    @State private var newTitle = ""
    @State private var showSettings = false

    private var shown: [LyricItem] {
        let query = search.trimmingCharacters(in: .whitespaces).lowercased()
        guard !query.isEmpty else { return library.items }
        return library.items.filter { item in
            item.title.lowercased().contains(query)
                || item.fileName.lowercased().contains(query)
                || item.sections.contains { $0.lowercased().contains(query) }
                || item.preview.contains { $0.lowercased().contains(query) }
        }
    }

    var body: some View {
        NavigationStack(path: $path) {
            ScrollView {
                LazyVStack(spacing: 12) {
                    if !library.usingCustomFolder {
                        SyncTip { showSettings = true }
                    }
                    if library.items.isEmpty, !library.isScanning {
                        emptyState
                    }
                    ForEach(shown) { item in
                        Button {
                            open(item)
                        } label: {
                            LyricCard(item: item)
                        }
                        .buttonStyle(.plain)
                    }
                }
                .padding(.horizontal, 16)
                .padding(.vertical, 8)
            }
            .background(Color(.systemGroupedBackground))
            .navigationTitle("Lyrics")
            .searchable(text: $search, prompt: "Titles, sections, lines")
            .refreshable { await library.refresh() }
            .toolbar {
                ToolbarItem(placement: .topBarLeading) {
                    Button("Settings", systemImage: "gearshape") { showSettings = true }
                }
                ToolbarItem(placement: .topBarTrailing) {
                    Button("New Lyric", systemImage: "square.and.pencil") { askTitle = true }
                }
            }
            .navigationDestination(for: URL.self) { url in
                EditorScreen(url: url)
            }
            .alert("New Lyric", isPresented: $askTitle) {
                TextField("Song title", text: $newTitle)
                Button("Create") { createLyric() }
                Button("Cancel", role: .cancel) { newTitle = "" }
            } message: {
                Text("Saved in \(library.folderDisplayName)")
            }
            .alert("Something went wrong", isPresented: .constant(library.errorMessage != nil)) {
                Button("OK") { library.errorMessage = nil }
            } message: {
                Text(library.errorMessage ?? "")
            }
            .sheet(isPresented: $showSettings) {
                SettingsView()
            }
            .task(id: library.folderURL) {
                // Pick up lyrics added or changed on other devices.
                while !Task.isCancelled {
                    if path.isEmpty { await library.refresh() }
                    try? await Task.sleep(for: .seconds(5))
                }
            }
            .onChange(of: scenePhase) { _, phase in
                if phase == .active { Task { await library.refresh() } }
            }
            .onOpenURL { url in
                // A .txt shared/opened from the Files app.
                _ = url.startAccessingSecurityScopedResource()
                path = [url]
            }
        }
    }

    private var emptyState: some View {
        VStack(spacing: 10) {
            Image(systemName: "music.note.list").font(.system(size: 40)).foregroundStyle(.secondary)
            Text("No lyrics here yet").font(.headline)
            Text("Lyric files (.txt) in \(library.folderDisplayName) show up here.")
                .font(.subheadline)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
            Button("Write your first lyric") { askTitle = true }
                .buttonStyle(.borderedProminent)
                .padding(.top, 4)
        }
        .padding(.vertical, 40)
    }

    private func open(_ item: LyricItem) {
        if item.isDownloaded {
            path.append(item.url)
        } else {
            try? FileManager.default.startDownloadingUbiquitousItem(at: item.url)
            Task { await library.refresh() }
        }
    }

    private func createLyric() {
        let title = newTitle.trimmingCharacters(in: .whitespaces)
        newTitle = ""
        guard !title.isEmpty else { return }
        do {
            let url = try library.createLyric(title: title)
            path.append(url)
            Task { await library.refresh() }
        } catch {
            library.errorMessage = "Couldn't create \(title): \(error.localizedDescription)"
        }
    }
}

struct LyricCard: View {
    let item: LyricItem

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(item.title).font(.headline).lineLimit(1)
            if !item.isDownloaded {
                Label("Downloading from iCloud\u{2026} tap to retry", systemImage: "icloud.and.arrow.down")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            } else {
                Text(item.sections.isEmpty ? "No sections yet" : item.sections.joined(separator: " · "))
                    .font(.caption)
                    .foregroundStyle(Color(uiColor: LyricColors.section))
                    .lineLimit(1)
                ForEach(item.preview, id: \.self) { line in
                    Text(line).font(.subheadline).italic().foregroundStyle(.secondary).lineLimit(1)
                }
            }
            HStack {
                Text(item.fileName)
                if let modified = item.modified {
                    Text("·")
                    Text(modified, format: .relative(presentation: .named))
                }
            }
            .font(.caption2)
            .foregroundStyle(.tertiary)
            .padding(.top, 2)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(14)
        .background(Color(.secondarySystemGroupedBackground), in: RoundedRectangle(cornerRadius: 14))
        .contentShape(RoundedRectangle(cornerRadius: 14))
    }
}

/// Shown until a synced folder is chosen.
struct SyncTip: View {
    let onSetUp: () -> Void

    var body: some View {
        Button(action: onSetUp) {
            HStack(alignment: .top, spacing: 12) {
                Image(systemName: "arrow.triangle.2.circlepath.icloud").font(.title2)
                VStack(alignment: .leading, spacing: 2) {
                    Text("Sync with your PC").font(.subheadline.weight(.semibold))
                    Text("Lyrics are only on this iPhone right now. Choose your iCloud Drive lyrics folder to share them with LyricToolsNotepad on Windows.")
                        .font(.footnote)
                        .foregroundStyle(.secondary)
                        .multilineTextAlignment(.leading)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(14)
            .background(Color.accentColor.opacity(0.12), in: RoundedRectangle(cornerRadius: 14))
        }
        .buttonStyle(.plain)
    }
}

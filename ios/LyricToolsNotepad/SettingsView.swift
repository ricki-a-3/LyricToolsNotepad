import SwiftUI
import UniformTypeIdentifiers

struct SettingsView: View {
    @Environment(LyricLibrary.self) private var library
    @Environment(AppSettings.self) private var settings
    @Environment(\.dismiss) private var dismiss
    @State private var pickingFolder = false

    var body: some View {
        @Bindable var settings = settings
        NavigationStack {
            Form {
                Section {
                    LabeledContent("Folder", value: library.folderDisplayName)
                    Button("Choose Folder\u{2026}") { pickingFolder = true }
                    if library.usingCustomFolder {
                        Button("Use On My iPhone Instead", role: .destructive) { library.useLocalFolder() }
                    }
                } header: {
                    Text("Lyrics folder")
                } footer: {
                    Text("New lyrics are saved here and the Lyrics list shows every .txt in it.")
                }

                Section("Sync with your PC") {
                    VStack(alignment: .leading, spacing: 8) {
                        step(1, "On your PC, install iCloud for Windows (Microsoft Store), sign in with your Apple ID and turn on iCloud Drive.")
                        step(2, "In LyricToolsNotepad on Windows, open Settings \u{2192} Lyrics folder \u{2192} Change, and pick a folder inside iCloud Drive (e.g. iCloud Drive\\LyricToolsNotepad).")
                        step(3, "Here, tap Choose Folder\u{2026} and pick that same folder under iCloud Drive.")
                        Text("Edits then show up on both devices automatically. OneDrive or Dropbox folders work the same way.")
                            .font(.footnote)
                            .foregroundStyle(.secondary)
                    }
                    .padding(.vertical, 4)
                }

                Section("Rhymes") {
                    Picker("Highlight a word for", selection: $settings.rhymeMode) {
                        ForEach(RhymeMode.all, id: \.self) { Text($0.title).tag($0) }
                    }
                }

                Section("Text") {
                    Stepper(value: $settings.fontSize, in: 12 ... 32, step: 1) {
                        LabeledContent("Text size", value: "\(Int(settings.fontSize))")
                    }
                }

                Section("About") {
                    LabeledContent("Version", value: Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "")
                    Text("Rhymes and syllable counts work offline (CMU Pronouncing Dictionary). Synonyms and Deep Search use the free Datamuse API.")
                        .font(.footnote)
                        .foregroundStyle(.secondary)
                }
            }
            .navigationTitle("Settings")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .confirmationAction) { Button("Done") { dismiss() } }
            }
            .sheet(isPresented: $pickingFolder) {
                FolderPicker { url in
                    library.setFolder(url)
                    Task { await library.refresh() }
                }
                .ignoresSafeArea()
            }
        }
    }

    private func step(_ n: Int, _ text: String) -> some View {
        HStack(alignment: .top, spacing: 8) {
            Text("\(n)").font(.footnote.weight(.bold)).frame(width: 18, height: 18)
                .background(Circle().fill(Color.accentColor.opacity(0.2)))
            Text(text).font(.footnote)
        }
    }
}

/// The system Files folder picker (iCloud Drive, On My iPhone, OneDrive...).
struct FolderPicker: UIViewControllerRepresentable {
    let onPick: (URL) -> Void

    func makeUIViewController(context: Context) -> UIDocumentPickerViewController {
        let picker = UIDocumentPickerViewController(forOpeningContentTypes: [.folder])
        picker.delegate = context.coordinator
        picker.allowsMultipleSelection = false
        return picker
    }

    func updateUIViewController(_ controller: UIDocumentPickerViewController, context: Context) {}

    func makeCoordinator() -> Coordinator { Coordinator(onPick: onPick) }

    final class Coordinator: NSObject, UIDocumentPickerDelegate {
        let onPick: (URL) -> Void

        init(onPick: @escaping (URL) -> Void) {
            self.onPick = onPick
        }

        func documentPicker(_ controller: UIDocumentPickerViewController, didPickDocumentsAt urls: [URL]) {
            if let url = urls.first { onPick(url) }
        }
    }
}

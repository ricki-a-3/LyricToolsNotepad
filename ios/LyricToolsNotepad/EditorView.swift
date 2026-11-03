import SwiftUI
import UIKit

/// Lets SwiftUI (toolbar, alerts, sheet) drive the UIKit editor.
@MainActor
final class EditorController {
    weak var coordinator: LyricEditor.Coordinator?
    var onSectionPrompt: (() -> Void)?

    func perform(_ action: EditorAction) { coordinator?.perform(action) }
    func pick(_ word: String) { coordinator?.pick(word) }
    func insertSection(_ title: String) { coordinator?.insertSection(title) }
    func refreshSuggestions() { coordinator?.refreshSuggestions() }
}

/// The editor screen for one lyric file.
struct EditorScreen: View {
    @Environment(AppSettings.self) private var settings
    @Environment(\.scenePhase) private var scenePhase
    @State private var document: LyricDocument
    @State private var bar = SuggestionBarModel()
    @State private var controller = EditorController()
    @State private var askSection = false
    @State private var sectionTitle = ""

    init(url: URL) {
        _document = State(initialValue: LyricDocument(url: url))
    }

    var body: some View {
        @Bindable var settings = settings
        Group {
            if document.isLoaded {
                LyricEditor(
                    document: document, fontSize: settings.fontSize, rhymeMode: settings.rhymeMode,
                    bar: bar, controller: controller
                )
                .ignoresSafeArea(.container, edges: .bottom)
            } else if let error = document.errorMessage {
                ContentUnavailableView("Couldn't open this lyric", systemImage: "exclamationmark.triangle",
                                       description: Text(error))
            } else {
                ProgressView()
            }
        }
        .navigationTitle(document.title)
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                Menu {
                    Picker("Rhymes", selection: $settings.rhymeMode) {
                        ForEach(RhymeMode.all, id: \.self) { Text($0.title).tag($0) }
                    }
                    Divider()
                    Button("Synonyms", systemImage: "text.book.closed") { controller.perform(.synonyms) }
                    Button("Deep Search", systemImage: "sparkle.magnifyingglass") { controller.perform(.deepSearch) }
                    Divider()
                    Button("Fold All Sections", systemImage: "rectangle.compress.vertical") { controller.perform(.foldAll) }
                    Button("Unfold All", systemImage: "rectangle.expand.vertical") { controller.perform(.unfoldAll) }
                } label: {
                    Label("Rhymes & tools", systemImage: "music.quarternote.3")
                }
            }
        }
        .safeAreaInset(edge: .top) {
            if document.changedElsewhere {
                ChangedElsewhereBanner(onKeepMine: { document.keepMine() }, onUseTheirs: { document.useTheirs() })
            }
        }
        .alert("New Section", isPresented: $askSection) {
            TextField("Chorus", text: $sectionTitle)
            Button("Add") {
                let title = sectionTitle.trimmingCharacters(in: .whitespaces)
                if !title.isEmpty { controller.insertSection(title) }
                sectionTitle = ""
            }
            Button("Cancel", role: .cancel) { sectionTitle = "" }
        } message: {
            Text("A titled, foldable part of the song, like Chorus or Verse 2.")
        }
        .sheet(isPresented: $bar.showingAll) {
            SuggestionList(model: bar) { controller.pick($0) }
                .presentationDetents([.medium, .large])
        }
        .task {
            controller.onSectionPrompt = { askSection = true }
            await document.load()
            // Safety net for sync services that don't notify file presenters.
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(3))
                document.checkForExternalChange()
            }
        }
        .onChange(of: scenePhase) { _, phase in
            if phase != .active { document.save() }
        }
        .onChange(of: settings.rhymeMode) { controller.refreshSuggestions() }
        .onDisappear { document.close() }
    }
}

struct ChangedElsewhereBanner: View {
    let onKeepMine: () -> Void
    let onUseTheirs: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Label("This lyric changed on another device", systemImage: "arrow.triangle.2.circlepath")
                .font(.subheadline.weight(.semibold))
            Text("You have edits here that aren't saved yet. Which version should win?")
                .font(.footnote)
                .foregroundStyle(.secondary)
            HStack {
                Button("Use the other version", action: onUseTheirs)
                Spacer()
                Button("Keep mine", action: onKeepMine).buttonStyle(.borderedProminent)
            }
            .font(.footnote.weight(.medium))
        }
        .padding(12)
        .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 12))
        .padding(.horizontal, 12)
    }
}

/// Bridges the UIKit `LyricTextView` into SwiftUI.
struct LyricEditor: UIViewRepresentable {
    let document: LyricDocument
    let fontSize: Double
    let rhymeMode: RhymeMode
    let bar: SuggestionBarModel
    let controller: EditorController

    func makeCoordinator() -> Coordinator { Coordinator(self) }

    func makeUIView(context: Context) -> LyricTextView {
        let textView = LyricTextView()
        let coordinator = context.coordinator
        coordinator.textView = textView
        textView.delegate = coordinator
        textView.fontSize = CGFloat(fontSize)
        textView.text = document.text
        textView.restyle()
        textView.inputAccessoryView = coordinator.makeAccessory()
        controller.coordinator = coordinator
        document.onReload = { [weak textView] text in
            guard let textView, textView.text != text else { return }
            let selection = textView.selectedRange
            let offset = textView.contentOffset
            textView.text = text
            textView.restyle()
            let length = (text as NSString).length
            textView.selectedRange = NSRange(location: min(selection.location, length), length: 0)
            textView.setContentOffset(offset, animated: false)
        }
        return textView
    }

    func updateUIView(_ textView: LyricTextView, context: Context) {
        context.coordinator.parent = self
        textView.fontSize = CGFloat(fontSize)
    }

    @MainActor
    final class Coordinator: NSObject, UITextViewDelegate {
        var parent: LyricEditor
        weak var textView: LyricTextView?
        private var hosting: UIHostingController<SuggestionBar>?
        private var selectionTask: Task<Void, Never>?
        private var generation = 0
        /// After a formatting command, don't pop up rhymes for the word it
        /// just selected.
        private var skipNextLookup = false

        init(_ parent: LyricEditor) {
            self.parent = parent
        }

        private var bar: SuggestionBarModel { parent.bar }

        func makeAccessory() -> UIView {
            let barView = SuggestionBar(
                model: parent.bar,
                onPick: { [weak self] in self?.pick($0) },
                onAction: { [weak self] in self?.perform($0) }
            )
            let host = UIHostingController(rootView: barView)
            host.view.backgroundColor = .clear
            host.view.translatesAutoresizingMaskIntoConstraints = false
            let container = UIInputView(
                frame: CGRect(x: 0, y: 0, width: 320, height: 44), inputViewStyle: .keyboard
            )
            container.autoresizingMask = [.flexibleWidth]
            container.addSubview(host.view)
            NSLayoutConstraint.activate([
                host.view.leadingAnchor.constraint(equalTo: container.leadingAnchor),
                host.view.trailingAnchor.constraint(equalTo: container.trailingAnchor),
                host.view.topAnchor.constraint(equalTo: container.topAnchor),
                host.view.bottomAnchor.constraint(equalTo: container.bottomAnchor),
            ])
            hosting = host
            return container
        }

        // MARK: UITextViewDelegate

        func textViewDidChange(_ textView: UITextView) {
            guard let textView = textView as? LyricTextView else { return }
            textView.restyle()
            parent.document.userEdited(textView.text)
        }

        func textViewDidChangeSelection(_ textView: UITextView) {
            guard let textView = textView as? LyricTextView else { return }
            textView.unfoldAroundCaret()
            selectionTask?.cancel()
            selectionTask = Task { [weak self] in
                try? await Task.sleep(for: .milliseconds(220))
                guard !Task.isCancelled else { return }
                self?.selectionSettled()
            }
        }

        // MARK: Rhymes for the selected word

        /// The selection, if it is exactly one whole word on one line.
        private func selectedWord() -> (String, NSRange)? {
            guard let textView, let text = textView.text else { return nil }
            let selection = textView.selectedRange
            guard selection.length > 0 else { return nil }
            let ns = text as NSString
            let lineRange = ns.lineRange(for: NSRange(location: selection.location, length: 0))
            var line = ns.substring(with: lineRange)
            while line.hasSuffix("\n") || line.hasSuffix("\r") { line.removeLast() }
            let start = selection.location - lineRange.location
            let end = start + selection.length
            guard end <= (line as NSString).length,
                  isWholeWord(line: line, start: UInt32(start), end: UInt32(end))
            else { return nil }
            return (ns.substring(with: selection), selection)
        }

        private func selectionSettled() {
            if skipNextLookup {
                skipNextLookup = false
                return
            }
            guard let picked = selectedWord() else {
                // Not on a whole word: go back to the formatting buttons. Keep
                // online results (Synonyms / Deep Search) for the word at the
                // cursor until you move away from it.
                if bar.showingAll { return }
                if bar.kind.isOnline, let range = bar.range, let caret = textView?.selectedRange,
                   NSLocationInRange(caret.location, NSRange(location: range.location, length: range.length + 1))
                {
                    return
                }
                bar.clear()
                return
            }
            let (word, range) = picked
            let kind = LookupKind.rhymes(parent.rhymeMode)
            if bar.word == word, bar.range == range, bar.kind == kind { return }
            lookup(word: word, range: range, kind: kind)
        }

        func refreshSuggestions() {
            guard let picked = selectedWord() else { return }
            lookup(word: picked.0, range: picked.1, kind: .rhymes(parent.rhymeMode))
        }

        private func lookup(word: String, range: NSRange, kind: LookupKind) {
            generation += 1
            let current = generation
            bar.begin(word: word, range: range, kind: kind)
            Task {
                // The engine is synchronous (and Synonyms/Deep Search hit the
                // network), so run it off the main thread.
                let result = await Task.detached(priority: .userInitiated) { () -> Result<[SuggestionGroup], Error> in
                    switch kind {
                    case .rhymes(let mode):
                        return .success(rhymeGroups(word: word, mode: mode))
                    case .synonyms:
                        return Result { [SuggestionGroup(heading: "Synonyms", words: try synonyms(word: word))] }
                    case .deepSearch:
                        return .success([SuggestionGroup(heading: "Deep Search", words: deepSearch(word: word))])
                    }
                }.value
                guard current == generation else { return } // superseded
                bar.finish(result)
                if kind.isOnline, !bar.flat.isEmpty { bar.showingAll = true }
            }
        }

        /// The selected word, else the word at the cursor.
        private func wordForLookup() -> (String, NSRange)? {
            guard let textView, let text = textView.text else { return nil }
            let selection = textView.selectedRange
            let ns = text as NSString
            if selection.length > 0 {
                let word = ns.substring(with: selection).trimmingCharacters(in: CharacterSet(charactersIn: " *_"))
                return word.isEmpty || word.contains("\n") ? nil : (word, selection)
            }
            guard let caret = textView.selectedTextRange?.start,
                  let range = textView.tokenizer.rangeEnclosingPosition(caret, with: .word, inDirection: .storage(.backward))
                      ?? textView.tokenizer.rangeEnclosingPosition(caret, with: .word, inDirection: .storage(.forward))
            else { return nil }
            let location = textView.offset(from: textView.beginningOfDocument, to: range.start)
            let length = textView.offset(from: range.start, to: range.end)
            let nsRange = NSRange(location: location, length: length)
            return (ns.substring(with: nsRange), nsRange)
        }

        func pick(_ choice: String) {
            guard let textView, let range = bar.range, let original = bar.word, let text = textView.text else { return }
            let ns = text as NSString
            guard NSMaxRange(range) <= ns.length, ns.substring(with: range) == original else { return }
            let replacement = matchCase(original: original, replacement: choice)
            textView.replace(range, with: replacement)
            textView.selectedRange = NSRange(location: range.location + (replacement as NSString).length, length: 0)
            bar.clear()
            bar.showingAll = false
        }

        // MARK: Commands

        func perform(_ action: EditorAction) {
            guard let textView, let text = textView.text else { return }
            let selection = textView.selectedRange
            switch action {
            case .bold, .italic, .underline:
                let marker = action == .bold ? "**" : action == .italic ? "*" : "__"
                skipNextLookup = true
                textView.apply(toggleWrap(
                    text: text, selectionStart: UInt32(selection.location),
                    selectionEnd: UInt32(NSMaxRange(selection)), marker: marker
                ))
            case .heading:
                textView.apply(cycleHeading(text: text, caret: UInt32(selection.location)))
            case .section:
                parent.controller.onSectionPrompt?()
            case .synonyms, .deepSearch:
                guard let picked = wordForLookup() else {
                    bar.show(message: "Select a word first")
                    return
                }
                lookup(word: picked.0, range: picked.1, kind: action == .synonyms ? .synonyms : .deepSearch)
            case .foldAll:
                textView.setAllFolded(true)
            case .unfoldAll:
                textView.setAllFolded(false)
            case .dismissKeyboard:
                textView.resignFirstResponder()
            }
        }

        func insertSection(_ title: String) {
            guard let textView, let text = textView.text else { return }
            textView.apply(LyricToolsNotepad.insertSection(
                text: text, caret: UInt32(textView.selectedRange.location), title: title
            ))
            textView.becomeFirstResponder()
        }
    }
}

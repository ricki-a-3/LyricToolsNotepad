import SwiftUI

enum LookupKind: Equatable {
    case rhymes(RhymeMode)
    case synonyms
    case deepSearch

    var heading: String {
        switch self {
        case .rhymes(let mode): mode == .nearAndPerfect ? "Rhymes" : "\(mode.title) Rhymes"
        case .synonyms: "Synonyms"
        case .deepSearch: "Deep Search"
        }
    }

    /// Synonyms and Deep Search go to the internet; rhymes don't.
    var isOnline: Bool {
        if case .rhymes = self { return false }
        return true
    }
}

enum EditorAction {
    case bold, italic, underline, heading, section
    case synonyms, deepSearch
    case foldAll, unfoldAll
    case dismissKeyboard
}

/// What the bar above the keyboard is showing.
@MainActor
@Observable
final class SuggestionBarModel {
    /// The word being looked up and where it sits (UTF-16), or nil when the
    /// bar shows the formatting buttons instead.
    private(set) var word: String?
    private(set) var range: NSRange?
    private(set) var kind: LookupKind = .rhymes(.nearAndPerfect)
    private(set) var groups: [SuggestionGroup] = []
    private(set) var loading = false
    private(set) var message: String?
    var showingAll = false

    var flat: [Suggestion] { groups.flatMap(\.words) }

    func begin(word: String, range: NSRange, kind: LookupKind) {
        self.word = word
        self.range = range
        self.kind = kind
        groups = []
        message = nil
        loading = true
    }

    func finish(_ result: Result<[SuggestionGroup], Error>) {
        loading = false
        switch result {
        case .success(let groups):
            self.groups = groups
            if groups.isEmpty, let word { message = "No \(kind.heading.lowercased()) for \u{201C}\(word)\u{201D}" }
        case .failure(let error):
            message = "\(kind.heading) failed: \(error.localizedDescription)"
        }
    }

    func show(message: String) {
        word = ""
        range = nil
        groups = []
        loading = false
        self.message = message
    }

    func clear() {
        word = nil
        range = nil
        groups = []
        loading = false
        message = nil
    }
}

/// The bar above the keyboard: rhyme chips for a selected word, otherwise
/// formatting buttons.
struct SuggestionBar: View {
    var model: SuggestionBarModel
    let onPick: (String) -> Void
    let onAction: (EditorAction) -> Void

    var body: some View {
        Group {
            if model.word != nil {
                suggestions
            } else {
                formatting
            }
        }
        .frame(maxWidth: .infinity, minHeight: 44, maxHeight: 44)
        .background(.bar)
    }

    private var suggestions: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 6) {
                if model.loading {
                    ProgressView().padding(.horizontal, 10)
                    Text(model.kind.heading + "\u{2026}").font(.footnote).foregroundStyle(.secondary)
                } else if let message = model.message {
                    Text(message).font(.footnote).foregroundStyle(.secondary).padding(.horizontal, 8)
                }
                ForEach(model.flat.prefix(30), id: \.word) { suggestion in
                    Button { onPick(suggestion.word) } label: { SuggestionChip(suggestion: suggestion) }
                        .buttonStyle(.plain)
                }
                if !model.flat.isEmpty {
                    Button("All \(model.flat.count)") { model.showingAll = true }
                        .font(.footnote.weight(.semibold))
                        .padding(.horizontal, 8)
                }
            }
            .padding(.horizontal, 8)
        }
    }

    private var formatting: some View {
        HStack(spacing: 2) {
            barButton("bold", "Bold", .bold)
            barButton("italic", "Italic", .italic)
            barButton("underline", "Underline", .underline)
            barButton("textformat.size", "Heading", .heading)
            Button { onAction(.section) } label: {
                Label("Section", systemImage: "plus.rectangle.on.rectangle").font(.footnote.weight(.medium))
            }
            .padding(.horizontal, 8)
            Spacer()
            barButton("keyboard.chevron.compact.down", "Hide keyboard", .dismissKeyboard)
        }
        .padding(.horizontal, 6)
    }

    private func barButton(_ symbol: String, _ label: String, _ action: EditorAction) -> some View {
        Button { onAction(action) } label: {
            Image(systemName: symbol).frame(width: 40, height: 36)
        }
        .accessibilityLabel(label)
    }
}

struct SuggestionChip: View {
    let suggestion: Suggestion

    var body: some View {
        HStack(spacing: 5) {
            Text(suggestion.word).font(.callout)
            Text("\(suggestion.syllables)")
                .font(.caption.weight(.bold))
                .foregroundStyle(Color(syllables: suggestion.syllables))
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 6)
        .background(Capsule().fill(Color(.tertiarySystemFill)))
    }
}

/// Every suggestion, grouped, in a sheet.
struct SuggestionList: View {
    var model: SuggestionBarModel
    let onPick: (String) -> Void
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            List {
                ForEach(model.groups, id: \.heading) { group in
                    Section(group.heading) {
                        ForEach(group.words, id: \.word) { suggestion in
                            Button {
                                onPick(suggestion.word)
                                dismiss()
                            } label: {
                                HStack {
                                    Text(suggestion.word).foregroundStyle(.primary)
                                    Spacer()
                                    Text("\(suggestion.syllables)")
                                        .font(.callout.weight(.bold))
                                        .foregroundStyle(Color(syllables: suggestion.syllables))
                                }
                            }
                        }
                    }
                }
            }
            .navigationTitle("\(model.kind.heading): \(model.word ?? "")")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .confirmationAction) { Button("Done") { dismiss() } }
            }
        }
    }
}

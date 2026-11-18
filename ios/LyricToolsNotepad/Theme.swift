import SwiftUI
import UIKit

/// JetBrains Mono (bundled, same as the Windows app) in four styles.
enum LyricFont {
    static func font(size: CGFloat, bold: Bool = false, italic: Bool = false) -> UIFont {
        let name: String
        switch (bold, italic) {
        case (false, false): name = "JetBrainsMono-Regular"
        case (true, false): name = "JetBrainsMono-Bold"
        case (false, true): name = "JetBrainsMono-Italic"
        case (true, true): name = "JetBrainsMono-BoldItalic"
        }
        return UIFont(name: name, size: size)
            ?? .monospacedSystemFont(ofSize: size, weight: bold ? .bold : .regular)
    }
}

enum LyricColors {
    /// `[Verse 1]` / `Chorus:` section headers.
    static let section = UIColor { traits in
        traits.userInterfaceStyle == .dark
            ? UIColor(red: 0x8a / 255, green: 0xb4 / 255, blue: 0xf8 / 255, alpha: 1)
            : UIColor(red: 0x1f / 255, green: 0x5f / 255, blue: 0xc4 / 255, alpha: 1)
    }
    /// The `**`, `*`, `__`, `#` characters.
    static let marker = UIColor.tertiaryLabel
    static let gutterBackground = UIColor.secondarySystemBackground
    static let foldArrow = UIColor.secondaryLabel

    /// The syllable-count color shared with the Windows app.
    static func syllables(_ count: UInt32) -> UIColor? {
        colorForCount(count: count).map {
            UIColor(red: CGFloat($0.r) / 255, green: CGFloat($0.g) / 255, blue: CGFloat($0.b) / 255, alpha: 1)
        }
    }
}

extension RhymeMode {
    static let all: [RhymeMode] = [.perfect, .near, .nearAndPerfect]

    var title: String {
        switch self {
        case .perfect: "Perfect"
        case .near: "Near"
        case .nearAndPerfect: "Near + Perfect"
        }
    }

    var key: String {
        switch self {
        case .perfect: "perfect"
        case .near: "near"
        case .nearAndPerfect: "nearAndPerfect"
        }
    }

    init?(key: String) {
        guard let mode = RhymeMode.all.first(where: { $0.key == key }) else { return nil }
        self = mode
    }
}

extension Color {
    init(syllables count: UInt32) {
        self = LyricColors.syllables(count).map { Color(uiColor: $0) } ?? .secondary
    }
}

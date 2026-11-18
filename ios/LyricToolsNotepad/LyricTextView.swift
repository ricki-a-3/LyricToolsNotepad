import UIKit

/// The lyric editor: a UITextView with a syllable gutter, styled markup
/// (bold / italic / underline / headings / sections) and foldable sections.
///
/// The text itself stays plain (markers like `**` remain in it) so files
/// round-trip with the Windows app; only the display is styled.
final class LyricTextView: UITextView {
    static let gutterWidth: CGFloat = 50

    var fontSize: CGFloat = 17 {
        didSet { if fontSize != oldValue { restyle() } }
    }

    /// Folded sections, keyed "title#occurrence" (stable across edits).
    private(set) var folded = Set<String>()
    /// Syllables per line (0 for headers / blank lines).
    private(set) var lineCounts: [UInt32] = []
    private(set) var sectionList: [LyricSection] = []
    /// UTF-16 offset where each line starts.
    private(set) var lineStarts: [Int] = [0]

    private let storage: NSTextStorage
    private let gutter = GutterView()

    init() {
        // TextKit 1, so we can ask where each line was laid out (for the gutter).
        storage = NSTextStorage()
        let layout = NSLayoutManager()
        storage.addLayoutManager(layout)
        let container = NSTextContainer(size: CGSize(width: 0, height: CGFloat.greatestFiniteMagnitude))
        container.widthTracksTextView = true
        layout.addTextContainer(container)
        super.init(frame: .zero, textContainer: container)

        alwaysBounceVertical = true
        keyboardDismissMode = .interactive
        autocapitalizationType = .sentences
        smartQuotesType = .no // keep ' as typed: "droppin'" must stay one word
        smartDashesType = .no
        backgroundColor = .systemBackground
        textContainerInset = UIEdgeInsets(top: 12, left: LyricTextView.gutterWidth + 6, bottom: 60, right: 12)

        gutter.textView = self
        gutter.backgroundColor = LyricColors.gutterBackground
        gutter.contentMode = .redraw
        addSubview(gutter)
        gutter.addGestureRecognizer(UITapGestureRecognizer(target: self, action: #selector(gutterTapped(_:))))
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used") }

    override func layoutSubviews() {
        super.layoutSubviews()
        // The gutter scrolls with the text; it extends one screen above and
        // below the content so overscroll never shows a gap.
        let extra = bounds.height
        let frame = CGRect(
            x: 0, y: -extra, width: LyricTextView.gutterWidth,
            height: max(contentSize.height, bounds.height) + 2 * extra
        )
        if gutter.frame != frame {
            gutter.frame = frame
            gutter.originOffset = extra
            gutter.setNeedsDisplay()
        }
        sendSubviewToBack(gutter)
    }

    // MARK: Styling

    private var baseAttributes: [NSAttributedString.Key: Any] {
        [.font: LyricFont.font(size: fontSize), .foregroundColor: UIColor.label]
    }

    private var hiddenAttributes: [NSAttributedString.Key: Any] {
        let paragraph = NSMutableParagraphStyle()
        paragraph.maximumLineHeight = 0.01
        paragraph.minimumLineHeight = 0.01
        return [.font: LyricFont.font(size: 0.01), .foregroundColor: UIColor.clear, .paragraphStyle: paragraph]
    }

    private func headingScale(_ level: UInt8) -> CGFloat {
        switch level {
        case 1: 1.55
        case 2: 1.3
        default: 1.12
        }
    }

    /// Hidden line indexes (bodies of folded sections).
    private func hiddenLines() -> Set<Int> {
        var hidden = Set<Int>()
        for section in sectionList where folded.contains(foldKey(section)) {
            hidden.formUnion(Int(section.headerLine) + 1 ..< Int(section.endLine))
        }
        return hidden
    }

    func foldKey(_ section: LyricSection) -> String {
        "\(section.title)#\(section.occurrence)"
    }

    /// Re-derive counts/sections and re-apply all styling. Cheap for lyric-sized text.
    func restyle() {
        // Never touch attributes mid-composition (e.g. a Japanese keyboard).
        guard markedTextRange == nil else { return }
        let text = self.text ?? ""
        let lines = text.components(separatedBy: "\n")
        lineCounts = lineSyllableCounts(text: text)
        sectionList = sections(text: text)
        var starts: [Int] = []
        var location = 0
        for line in lines {
            starts.append(location)
            location += (line as NSString).length + 1
        }
        lineStarts = starts
        let hidden = hiddenLines()

        storage.beginEditing()
        storage.setAttributes(baseAttributes, range: NSRange(location: 0, length: storage.length))
        for (i, line) in lines.enumerated() {
            let start = starts[i]
            let length = (line as NSString).length
            if hidden.contains(i) {
                let withBreak = NSRange(location: start, length: min(length + 1, storage.length - start))
                storage.setAttributes(hiddenAttributes, range: withBreak)
                continue
            }
            style(line: line, start: start, length: length)
        }
        storage.endEditing()
        typingAttributes = baseAttributes
        gutter.setNeedsDisplay()
    }

    private func style(line: String, start: Int, length: Int) {
        guard length > 0 else { return }
        let whole = NSRange(location: start, length: length)
        switch lineKind(line: line) {
        case .heading(let level):
            let size = fontSize * headingScale(level)
            storage.addAttributes([.font: LyricFont.font(size: size, bold: true)], range: whole)
            let marks = min(Int(level) + 1, length)
            storage.addAttribute(.foregroundColor, value: LyricColors.marker, range: NSRange(location: start, length: marks))
        case .section:
            storage.addAttributes(
                [.font: LyricFont.font(size: fontSize * 1.05, bold: true), .foregroundColor: LyricColors.section],
                range: whole
            )
        case .plain:
            for run in inlineRuns(line: line) where run.bold || run.italic || run.underline || run.marker {
                let range = NSRange(location: start + Int(run.start), length: Int(run.length))
                var attrs: [NSAttributedString.Key: Any] = [
                    .font: LyricFont.font(size: fontSize, bold: run.bold, italic: run.italic),
                ]
                if run.marker { attrs[.foregroundColor] = LyricColors.marker }
                if run.underline { attrs[.underlineStyle] = NSUnderlineStyle.single.rawValue }
                storage.addAttributes(attrs, range: range)
            }
        }
    }

    // MARK: Lines & folding

    /// Line index containing a UTF-16 offset.
    func lineIndex(at offset: Int) -> Int {
        var low = 0
        var high = lineStarts.count - 1
        while low < high {
            let mid = (low + high + 1) / 2
            if lineStarts[mid] <= offset { low = mid } else { high = mid - 1 }
        }
        return low
    }

    /// Where line `i` was laid out, in text-view content coordinates.
    func lineRect(_ i: Int) -> CGRect? {
        guard i < lineStarts.count else { return nil }
        let lm = layoutManager
        lm.ensureLayout(for: textContainer)
        let charIndex = lineStarts[i]
        var rect: CGRect
        if charIndex >= storage.length {
            rect = lm.extraLineFragmentRect
            if rect.isEmpty {
                guard storage.length > 0 else {
                    return CGRect(x: 0, y: textContainerInset.top, width: 1, height: font?.lineHeight ?? fontSize)
                }
                let last = lm.glyphIndexForCharacter(at: storage.length - 1)
                rect = lm.lineFragmentRect(forGlyphAt: last, effectiveRange: nil)
            }
        } else {
            rect = lm.lineFragmentRect(forGlyphAt: lm.glyphIndexForCharacter(at: charIndex), effectiveRange: nil)
        }
        return rect.offsetBy(dx: textContainerInset.left, dy: textContainerInset.top)
    }

    func hasBody(_ section: LyricSection) -> Bool {
        section.endLine > section.headerLine + 1
    }

    func isFolded(_ section: LyricSection) -> Bool {
        folded.contains(foldKey(section))
    }

    func toggleFold(_ section: LyricSection) {
        let key = foldKey(section)
        if folded.remove(key) == nil { folded.insert(key) }
        restyle()
    }

    func setAllFolded(_ fold: Bool) {
        folded = fold ? Set(sectionList.filter(hasBody).map(foldKey)) : []
        restyle()
    }

    /// Typing or moving into a folded section opens it.
    func unfoldAroundCaret() {
        let line = lineIndex(at: selectedRange.location)
        for section in sectionList where isFolded(section)
            && (Int(section.headerLine) + 1 ..< Int(section.endLine)).contains(line)
        {
            folded.remove(foldKey(section))
            restyle()
            return
        }
    }

    @objc private func gutterTapped(_ tap: UITapGestureRecognizer) {
        let y = tap.location(in: gutter).y - gutter.originOffset
        for section in sectionList where hasBody(section) {
            guard let rect = lineRect(Int(section.headerLine)) else { continue }
            if y >= rect.minY - 6, y <= rect.maxY + 6 {
                toggleFold(section)
                return
            }
        }
    }

    // MARK: Edits (all go through UITextInput so undo works)

    /// Replace a UTF-16 range, then tell the delegate like a typed edit would.
    func replace(_ range: NSRange, with string: String) {
        guard let start = position(from: beginningOfDocument, offset: range.location),
              let end = position(from: start, offset: range.length),
              let textRange = textRange(from: start, to: end)
        else { return }
        replace(textRange, withText: string)
        delegate?.textViewDidChange?(self)
    }

    /// Apply a whole-text edit (from a formatting command) as one undo step,
    /// replacing only the part that actually changed.
    func apply(_ result: EditResult) {
        let old = Array((text ?? "").utf16)
        let new = Array(result.text.utf16)
        var prefix = 0
        while prefix < old.count, prefix < new.count, old[prefix] == new[prefix] { prefix += 1 }
        var suffix = 0
        while suffix < old.count - prefix, suffix < new.count - prefix,
              old[old.count - 1 - suffix] == new[new.count - 1 - suffix] { suffix += 1 }
        let middle = String(utf16CodeUnits: Array(new[prefix ..< new.count - suffix]), count: new.count - suffix - prefix)
        replace(NSRange(location: prefix, length: old.count - suffix - prefix), with: middle)
        selectedRange = NSRange(
            location: Int(result.selectionStart),
            length: Int(result.selectionEnd) - Int(result.selectionStart)
        )
    }
}

/// Draws the syllable counts and fold arrows to the left of the text.
final class GutterView: UIView {
    weak var textView: LyricTextView?
    /// The gutter extends above the content (for overscroll); this is how far.
    var originOffset: CGFloat = 0

    override func draw(_ rect: CGRect) {
        guard let tv = textView else { return }
        let size = tv.fontSize * 0.85
        let font = LyricFont.font(size: size, bold: true)
        let right = LyricTextView.gutterWidth - 8
        let hidden = Set(tv.sectionList.filter(tv.isFolded).flatMap { Int($0.headerLine) + 1 ..< Int($0.endLine) })

        for (i, count) in tv.lineCounts.enumerated() where !hidden.contains(i) {
            guard let lineRect = tv.lineRect(i) else { continue }
            let midY = lineRect.midY + originOffset
            guard midY > rect.minY - 40, midY < rect.maxY + 40 else { continue }
            if let color = LyricColors.syllables(count) {
                let label = "\(count)" as NSString
                let attrs: [NSAttributedString.Key: Any] = [.font: font, .foregroundColor: color]
                let width = label.size(withAttributes: attrs).width
                label.draw(at: CGPoint(x: right - width, y: midY - font.lineHeight / 2), withAttributes: attrs)
            }
        }

        LyricColors.foldArrow.setFill()
        for section in tv.sectionList where tv.hasBody(section) {
            guard let lineRect = tv.lineRect(Int(section.headerLine)) else { continue }
            let c = CGPoint(x: 10, y: lineRect.midY + originOffset)
            let s: CGFloat = 5
            let path = UIBezierPath()
            if tv.isFolded(section) {
                path.move(to: CGPoint(x: c.x - s * 0.6, y: c.y - s))
                path.addLine(to: CGPoint(x: c.x + s * 0.9, y: c.y))
                path.addLine(to: CGPoint(x: c.x - s * 0.6, y: c.y + s))
                // How many lines are tucked away, where the count would go.
                let n = Int(section.endLine) - Int(section.headerLine) - 1
                let note = "+\(n)" as NSString
                let noteAttrs: [NSAttributedString.Key: Any] = [
                    .font: UIFont.systemFont(ofSize: size * 0.8, weight: .medium),
                    .foregroundColor: LyricColors.foldArrow,
                ]
                let noteSize = note.size(withAttributes: noteAttrs)
                note.draw(at: CGPoint(x: right - noteSize.width, y: c.y - noteSize.height / 2), withAttributes: noteAttrs)
            } else {
                path.move(to: CGPoint(x: c.x - s, y: c.y - s * 0.6))
                path.addLine(to: CGPoint(x: c.x + s, y: c.y - s * 0.6))
                path.addLine(to: CGPoint(x: c.x, y: c.y + s * 0.9))
            }
            path.close()
            path.fill()
        }
    }
}

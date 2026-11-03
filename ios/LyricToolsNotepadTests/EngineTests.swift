import XCTest
@testable import LyricToolsNotepad

/// The Rust engine as Swift sees it, plus the app's file helpers.
final class EngineTests: XCTestCase {
    func testSyllableCountsSkipHeaders() {
        XCTAssertEqual(lineSyllableCounts(text: "[Verse 1]\nMy bitch'll pop a cap\n"), [0, 6, 0])
    }

    func testRhymesForNight() {
        let groups = rhymeGroups(word: "night", mode: .nearAndPerfect)
        XCTAssertEqual(groups.first?.heading, "Perfect")
        XCTAssertTrue(groups.first?.words.contains { $0.word == "light" } ?? false)
    }

    func testWholeWordUsesUTF16Offsets() {
        // "café" has a precomposed é (1 UTF-16 unit); "night" starts at 5.
        XCTAssertTrue(isWholeWord(line: "café night", start: 5, end: 10))
        XCTAssertFalse(isWholeWord(line: "café night", start: 5, end: 8))
    }

    func testBoldToggleReturnsUTF16Selection() {
        let result = toggleWrap(text: "make it big", selectionStart: 8, selectionEnd: 11, marker: "**")
        XCTAssertEqual(result.text, "make it **big**")
        XCTAssertEqual(result.selectionStart, 10)
        XCTAssertEqual(result.selectionEnd, 13)
    }

    func testWindowsLineEndingsSurviveARoundTrip() {
        let original = Data("[Hook]\r\nline\r\n".utf8)
        let decoded = decodeFile(data: original)
        XCTAssertEqual(decoded.text, "[Hook]\nline\n")
        XCTAssertEqual(encodeFile(text: decoded.text, newline: decoded.newline, bom: decoded.bom), original)
    }

    func testICloudPlaceholderNames() {
        XCTAssertEqual(LibraryScanner.realName(forPlaceholder: ".Late Night.txt.icloud"), "Late Night.txt")
        XCTAssertNil(LibraryScanner.realName(forPlaceholder: "Late Night.txt"))
        XCTAssertNil(LibraryScanner.realName(forPlaceholder: ".icloud"))
    }

    func testScanFindsLyricsAndReadsTitles() throws {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        try Data("# Late Night\n[Verse 1]\nI been droppin' bars".utf8).write(to: dir.appendingPathComponent("a.txt"))
        try Data("not lyrics".utf8).write(to: dir.appendingPathComponent("b.pdf"))
        let items = LibraryScanner.scan(dir)
        XCTAssertEqual(items.map(\.title), ["Late Night"])
        XCTAssertEqual(items.first?.sections, ["Verse 1"])
        XCTAssertEqual(LibraryScanner.uniqueURL(in: dir, name: "a").lastPathComponent, "a (2).txt")
    }
}

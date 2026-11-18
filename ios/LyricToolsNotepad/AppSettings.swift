import Foundation
import Observation

/// Small user preferences, kept in UserDefaults.
@Observable
final class AppSettings {
    var rhymeMode: RhymeMode {
        didSet { UserDefaults.standard.set(rhymeMode.key, forKey: "rhymeMode") }
    }

    var fontSize: Double {
        didSet { UserDefaults.standard.set(fontSize, forKey: "fontSize") }
    }

    static let defaultFontSize = 17.0

    init() {
        let defaults = UserDefaults.standard
        rhymeMode = defaults.string(forKey: "rhymeMode").flatMap(RhymeMode.init(key:)) ?? .nearAndPerfect
        let size = defaults.double(forKey: "fontSize")
        fontSize = size > 0 ? size : AppSettings.defaultFontSize
    }
}

"""Dark (default) and light themes: Fusion style + palette + a little QSS."""
from PySide6.QtGui import QColor, QPalette

THEMES = {
    "dark": {
        "window": "#202329", "base": "#16181d", "alt": "#1d2026", "text": "#e6e6e6",
        "muted": "#7a808c", "button": "#2a2e36", "highlight": "#3d5a99",
        "highlight_text": "#ffffff", "border": "#2f333b",
        "gutter_bg": "#1b1d23", "gutter_fg": "#6b7280", "current_line": "#ffffff0d",
    },
    "light": {
        "window": "#f3f3f3", "base": "#ffffff", "alt": "#f7f7f7", "text": "#1e1e1e",
        "muted": "#80868f", "button": "#e8e8e8", "highlight": "#b5cdf5",
        "highlight_text": "#000000", "border": "#d6d6d6",
        "gutter_bg": "#f0f0f0", "gutter_fg": "#9aa0a6", "current_line": "#0000000a",
    },
}


def _argb(hex_rgba):
    """'#rrggbbaa' -> QColor (Qt's own parser expects #aarrggbb)."""
    if len(hex_rgba) == 9:
        color = QColor(hex_rgba[:7])
        color.setAlpha(int(hex_rgba[7:], 16))
        return color
    return QColor(hex_rgba)


def apply_theme(app, editor, name):
    t = THEMES.get(name, THEMES["dark"])
    app.setStyle("Fusion")
    pal = QPalette()
    roles = QPalette.ColorRole
    pal.setColor(roles.Window, QColor(t["window"]))
    pal.setColor(roles.WindowText, QColor(t["text"]))
    pal.setColor(roles.Base, QColor(t["base"]))
    pal.setColor(roles.AlternateBase, QColor(t["alt"]))
    pal.setColor(roles.Text, QColor(t["text"]))
    pal.setColor(roles.Button, QColor(t["button"]))
    pal.setColor(roles.ButtonText, QColor(t["text"]))
    pal.setColor(roles.ToolTipBase, QColor(t["button"]))
    pal.setColor(roles.ToolTipText, QColor(t["text"]))
    pal.setColor(roles.Highlight, QColor(t["highlight"]))
    pal.setColor(roles.HighlightedText, QColor(t["highlight_text"]))
    pal.setColor(roles.PlaceholderText, QColor(t["muted"]))
    pal.setColor(roles.Link, QColor(t["highlight"]))
    for role in (roles.Text, roles.WindowText, roles.ButtonText):
        pal.setColor(QPalette.ColorGroup.Disabled, role, QColor(t["muted"]))
    app.setPalette(pal)
    app.setStyleSheet(
        "QPlainTextEdit { border: none; }"
        "QToolBar { border: none; spacing: 2px; padding: 2px; }"
        "QStatusBar { color: %(muted)s; }"
        "#SuggestionPopup { border: 1px solid %(border)s; background: %(alt)s; }"
        "#SuggestionPopup::item { padding: 2px 8px; }" % t
    )
    editor.set_colors(QColor(t["gutter_bg"]), QColor(t["gutter_fg"]), _argb(t["current_line"]))

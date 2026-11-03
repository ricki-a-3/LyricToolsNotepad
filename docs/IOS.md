# LyricToolsNotepad on iPhone

The iPhone app has the same syllable counts, rhymes, formatting and foldable
sections as the Windows app. It reads and writes the same plain `.txt` files,
so a shared cloud folder keeps your lyrics in sync between the two.

## 1. Install it (from a Windows PC, no Mac needed)

The app isn't on the App Store, so you install ("sideload") it yourself with
a free Apple ID. Apps installed this way **expire after 7 days**; just
re-install (or let AltStore refresh it) — your lyrics are not affected.

1. **Get the app file.** On GitHub, open **Actions → iOS**, click the newest
   green run and download **LyricToolsNotepad-ios** (a zip containing
   `LyricToolsNotepad-<version>-ios-unsigned.ipa`). Releases also have the
   `.ipa` attached.
2. **Install Sideloadly** from <https://sideloadly.io> on your PC. It asks
   for iTunes and iCloud from Apple's website (not the Microsoft Store
   versions) — install those if prompted.
3. Plug your iPhone into the PC with a cable, unlock it, and tap **Trust**.
4. In Sideloadly, drag in the `.ipa`, type your Apple ID email, click
   **Start**, and enter your Apple ID password when asked.
5. On the iPhone, turn on **Settings → Privacy & Security → Developer Mode**
   (the phone restarts), then **Settings → General → VPN & Device
   Management** → your Apple ID → **Trust**.
6. Open LyricToolsNotepad. 🎉

> Prefer automatic refreshes? **AltStore** (<https://altstore.io>) does the
> same job and re-signs the app over Wi-Fi every few days while AltServer is
> running on your PC.

## 2. Sync with your PC

1. **PC:** install **iCloud for Windows** (Microsoft Store), sign in with the
   same Apple ID, and turn on **iCloud Drive**. A folder
   `C:\Users\<you>\iCloudDrive` appears.
2. **PC:** in LyricToolsNotepad → **Settings → Lyrics folder → Change…**,
   pick (or create) `iCloudDrive\LyricToolsNotepad`. Move your existing
   lyrics into it if you like.
3. **iPhone:** LyricToolsNotepad → **Settings → Choose Folder…** → **iCloud
   Drive → LyricToolsNotepad** → **Open**.

That's it — edits on either device show up on the other within seconds
(as fast as iCloud syncs). If a lyric changes on the other device while you
have unsaved edits, the app asks which version to keep instead of silently
overwriting one.

OneDrive or Dropbox work the same way: pick that folder on both devices.

## Using it

- **Rhymes:** select a word (double-tap) — rhymes appear above the keyboard
  with their syllable counts; tap one to swap it in, or **All** for the full
  list. Pick Perfect / Near / Near + Perfect from the ♫ menu.
- **Formatting:** with nothing selected, the bar above the keyboard has
  **B**, *I*, U, Heading and **Section** (adds a titled `[Chorus]`-style part).
- **Folding:** tap the arrow next to a section in the gutter.
- **Synonyms / Deep Search:** ♫ menu (needs internet).

## Building it yourself

Needs a Mac with Xcode 16+, Rust (`rustup`) and XcodeGen (`brew install xcodegen`):

```
ios/build-rust.sh
cd ios && xcodegen && open LyricToolsNotepad.xcodeproj
```

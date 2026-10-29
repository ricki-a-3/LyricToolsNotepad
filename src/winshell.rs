//! "Open with LyricTools++" on the right-click menu of .txt files.
//!
//! Per-user (HKCU), so no admin rights are needed. The installer writes the
//! same key, so the Settings toggle and the installer agree with each other.

pub const MENU_TEXT: &str = "Open with LyricTools++";
#[cfg(windows)]
const KEY: &str = r"Software\Classes\SystemFileAssociations\.txt\shell\LyricToolsPP";

#[cfg(windows)]
pub fn is_registered() -> bool {
    use winreg::{RegKey, enums::HKEY_CURRENT_USER};
    RegKey::predef(HKEY_CURRENT_USER).open_subkey(format!(r"{KEY}\command")).is_ok()
}

#[cfg(windows)]
pub fn register() -> std::io::Result<()> {
    use winreg::{RegKey, enums::HKEY_CURRENT_USER};
    let exe = std::env::current_exe()?;
    let exe = exe.to_string_lossy();
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu.create_subkey(KEY)?;
    key.set_value("", &MENU_TEXT)?;
    key.set_value("Icon", &format!("\"{exe}\",0"))?;
    let (command, _) = hkcu.create_subkey(format!(r"{KEY}\command"))?;
    command.set_value("", &format!("\"{exe}\" \"%1\""))?;
    Ok(())
}

#[cfg(windows)]
pub fn unregister() -> std::io::Result<()> {
    use winreg::{RegKey, enums::HKEY_CURRENT_USER};
    match RegKey::predef(HKEY_CURRENT_USER).delete_subkey_all(KEY) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

#[cfg(not(windows))]
pub fn is_registered() -> bool {
    false
}

#[cfg(not(windows))]
pub fn register() -> std::io::Result<()> {
    Err(std::io::Error::other("only available on Windows"))
}

#[cfg(not(windows))]
pub fn unregister() -> std::io::Result<()> {
    Ok(())
}

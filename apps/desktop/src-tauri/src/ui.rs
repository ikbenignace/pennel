use std::sync::Mutex;
use std::sync::atomic::{AtomicU8, Ordering};

use pennel_core::InterfaceLanguage;
use tauri::Manager;
use tauri::menu::{Menu, MenuItem, MenuItemKind};

static LANGUAGE: AtomicU8 = AtomicU8::new(0);

pub fn initialize(language: InterfaceLanguage) {
    LANGUAGE.store(
        u8::from(language == InterfaceLanguage::Dutch),
        Ordering::SeqCst,
    );
}

pub fn language() -> InterfaceLanguage {
    if LANGUAGE.load(Ordering::SeqCst) == 1 {
        InterfaceLanguage::Dutch
    } else {
        InterfaceLanguage::English
    }
}

pub fn message(source: impl AsRef<str>) -> String {
    pennel_core::localize_ui_text(language(), source.as_ref())
}

/// The source labels and native item handles survive relabeling; localized captions are never IDs.
#[derive(Default)]
struct NativeUi {
    #[cfg(target_os = "macos")]
    stock: Mutex<Vec<(MenuItemKind<tauri::Wry>, String)>>,
    tray: Mutex<Vec<(MenuItem<tauri::Wry>, &'static str)>>,
}

pub fn install_menu(app: &tauri::AppHandle) -> tauri::Result<()> {
    if app.try_state::<NativeUi>().is_some() {
        return Ok(());
    }
    app.manage(NativeUi::default());
    #[cfg(target_os = "macos")]
    {
        // Keep Tauri's stock objects, selectors, shortcuts and OS-managed Services submenu.
        let existing = app.menu();
        let menu = match &existing {
            Some(menu) => menu.clone(),
            None => Menu::default(app)?,
        };
        let mut app_names = vec![app.package_info().name.as_str()];
        if let Some(name) = app.config().product_name.as_deref() {
            app_names.push(name);
        }
        let native = app.state::<NativeUi>();
        let mut sources = native.stock.lock().unwrap();
        remember_menu_items(menu.items()?, &app_names, &mut sources)?;
        crate::desktop_commands::install(app, &menu)?;
        for (item, original) in sources.iter() {
            if let Some(label) = stock_menu_text(language(), original, &app_names) {
                set_stock_text(item, label)?;
            }
        }
        if existing.is_none() {
            app.set_menu(menu)?;
        }
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn remember_menu_items(
    items: Vec<MenuItemKind<tauri::Wry>>,
    app_names: &[&str],
    sources: &mut Vec<(MenuItemKind<tauri::Wry>, String)>,
) -> tauri::Result<()> {
    for item in items {
        match &item {
            MenuItemKind::Submenu(submenu) => {
                let original = submenu.text()?;
                if stock_menu_text(InterfaceLanguage::English, &original, app_names).is_some() {
                    sources.push((item.clone(), original.clone()));
                }
                if original != "Services" {
                    remember_menu_items(submenu.items()?, app_names, sources)?;
                }
            }
            MenuItemKind::Predefined(predefined) => {
                let original = predefined.text()?;
                if stock_menu_text(InterfaceLanguage::English, &original, app_names).is_some() {
                    sources.push((item.clone(), original));
                }
            }
            _ => {}
        }
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn set_stock_text(item: &MenuItemKind<tauri::Wry>, label: String) -> tauri::Result<()> {
    let label = label.replace('&', "&&");
    match item {
        MenuItemKind::Submenu(item) => item.set_text(label),
        MenuItemKind::Predefined(item) => item.set_text(label),
        _ => Ok(()),
    }
}

pub fn register_tray(app: &tauri::AppHandle, menu: &Menu<tauri::Wry>) -> tauri::Result<()> {
    install_menu(app)?;
    let native = app.state::<NativeUi>();
    let mut items = native.tray.lock().unwrap();
    for item in menu.items()? {
        if let MenuItemKind::MenuItem(item) = item {
            let source = match item.id().as_ref() {
                "open" => "Open Pennel",
                "dictation" => "Start/stop dictation",
                "pad" => "Dictation…",
                "history" => "History…",
                "settings" => "Settings…",
                "log" => "Open log",
                "quit" => "Quit Pennel",
                "meeting" if !cfg!(target_os = "macos") => "Record meeting…",
                _ => continue,
            };
            items.push((item, source));
        }
    }
    Ok(())
}

pub fn apply(app: &tauri::AppHandle, shared: &crate::state::Shared) -> tauri::Result<()> {
    let next = shared.interface_language();
    if next == language() {
        return Ok(());
    }
    initialize(next);
    #[cfg(target_os = "macos")]
    let _ = crate::macos::localize_style_application_picker();
    for (label, window) in app.webview_windows() {
        if let Some(spec) = crate::workspace::label_spec(&label) {
            window.set_title(&message(spec.title))?;
        }
    }
    if let Some(native) = app.try_state::<NativeUi>() {
        #[cfg(target_os = "macos")]
        {
            let mut names = vec![app.package_info().name.as_str()];
            if let Some(name) = app.config().product_name.as_deref() {
                names.push(name);
            }
            for (item, original) in native.stock.lock().unwrap().iter() {
                if let Some(label) = stock_menu_text(next, original, &names) {
                    set_stock_text(item, label)?;
                }
            }
        }
        for (item, original) in native.tray.lock().unwrap().iter() {
            item.set_text(message(original))?;
        }
    }
    if let Some(hint) = app.try_state::<crate::TrayHint>() {
        hint.0.set_text(crate::hotkey_hint(shared))?;
    }
    #[cfg(target_os = "macos")]
    crate::macos_tray::changed();
    crate::desktop_commands::refresh(app)?;
    Ok(())
}

/// Read once at launch. A saved System preference follows the next launch's OS language list.
pub fn system_languages() -> Vec<String> {
    #[cfg(target_os = "macos")]
    return crate::macos::preferred_languages();
    #[cfg(windows)]
    return windows_languages();
    #[cfg(target_os = "linux")]
    {
        linux_languages(
            std::env::var("LANGUAGE").ok().as_deref(),
            std::env::var("LC_ALL").ok().as_deref(),
            std::env::var("LC_MESSAGES").ok().as_deref(),
            std::env::var("LANG").ok().as_deref(),
        )
    }
}

#[cfg(windows)]
fn windows_languages() -> Vec<String> {
    use windows::Win32::Globalization::{GetUserPreferredUILanguages, MUI_LANGUAGE_NAME};
    use windows::core::PWSTR;
    let mut count = 0;
    let mut length = 0;
    unsafe {
        if GetUserPreferredUILanguages(MUI_LANGUAGE_NAME, &mut count, None, &mut length).is_err()
            || length == 0
        {
            return Vec::new();
        }
        let mut buffer = vec![0u16; length as usize];
        if GetUserPreferredUILanguages(
            MUI_LANGUAGE_NAME,
            &mut count,
            Some(PWSTR(buffer.as_mut_ptr())),
            &mut length,
        )
        .is_err()
        {
            return Vec::new();
        }
        buffer
            .split(|unit| *unit == 0)
            .filter(|part| !part.is_empty())
            .map(String::from_utf16_lossy)
            .collect()
    }
}

#[cfg(any(target_os = "linux", test))]
fn linux_languages(
    language: Option<&str>,
    all: Option<&str>,
    messages: Option<&str>,
    lang: Option<&str>,
) -> Vec<String> {
    fn usable(value: Option<&str>) -> Option<&str> {
        value.map(str::trim).filter(|value| !value.is_empty())
    }
    let locale = usable(all)
        .or_else(|| usable(messages))
        .or_else(|| usable(lang))
        .unwrap_or("C");
    let base = locale.split(['.', '@']).next().unwrap_or(locale);
    if matches!(base, "C" | "POSIX") {
        return vec!["en".into()];
    }
    usable(language)
        .map(|languages| {
            languages
                .split(':')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_else(|| vec![locale.to_owned()])
}

#[cfg(any(target_os = "macos", test))]
fn stock_menu_text(
    language: InterfaceLanguage,
    source: &str,
    app_names: &[&str],
) -> Option<String> {
    for name in app_names {
        if source == *name || source == name.replace('&', "&&") {
            return None;
        }
        for (verb, key) in [
            ("About", "backend.menu.about"),
            ("Hide", "backend.menu.hide"),
            ("Quit", "backend.menu.quit"),
        ] {
            let named = format!("{verb} {name}");
            if source == named || source == named.replace('&', "&&") {
                return Some(pennel_core::ui_text(language, key, &[("app", name)]));
            }
        }
    }
    let key = match source {
        "File" => "file",
        "Edit" => "edit",
        "View" => "view",
        "Window" => "window",
        "Help" => "help",
        "Undo" => "undo",
        "Redo" => "redo",
        "Cut" | "&Cut" => "cut",
        "Copy" | "&Copy" => "copy",
        "Paste" | "&Paste" => "paste",
        "Paste and Match Style" => "paste_match_style",
        "Delete" | "&Delete" => "delete",
        "Select All" | "Select &All" => "select_all",
        "Minimize" | "&Minimize" => "minimize",
        "Zoom" => "zoom",
        "Close Window" | "C&lose Window" => "close_window",
        "Toggle Full Screen" => "full_screen",
        "Hide Others" => "hide_others",
        "Show All" => "show_all",
        "Bring All to Front" => "bring_all_front",
        "Services" => "services",
        "About" | "&About" => "about_bare",
        "Hide" | "&Hide" => "hide_bare",
        "Quit" | "&Quit" => "quit_bare",
        _ => return None,
    };
    Some(pennel_core::ui_text(
        language,
        &format!("backend.menu.{key}"),
        &[],
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_platform_and_provider_errors_have_both_interface_translations() {
        for (source, english) in [
            (
                "De provider weigert toegang. Controleer je API-sleutel en toegang tot het model.",
                "The provider denied access. Check your API key and access to the model.",
            ),
            (
                "Geef Pennel toegang tot de microfoon in Systeeminstellingen → Privacy en beveiliging → Microfoon en probeer opnieuw.",
                "Give Pennel microphone access in System Settings → Privacy & Security → Microphone, then try again.",
            ),
            (
                "Sta microfoontoegang toe in de melding van macOS en probeer opnieuw.",
                "Allow microphone access in the macOS prompt, then try again.",
            ),
            (
                "De microfoon kon niet starten. Kies een ander invoerapparaat of probeer opnieuw.",
                "The microphone could not start. Choose another input device or try again.",
            ),
            (
                "Browsercalls herkennen vereist xprop en toegang tot de X11-sessie (pakket x11-utils).",
                "Detecting browser calls requires xprop and access to the X11 session (package x11-utils).",
            ),
            (
                "Echo-onderdrukking kon niet worden gecontroleerd",
                "Echo cancellation could not be checked",
            ),
            (
                "Deze microfoon biedt geen bevestigde echo-onderdrukking",
                "This microphone does not provide confirmed echo cancellation",
            ),
            (
                "Microfoon met echo-onderdrukking kon niet starten",
                "The microphone with echo cancellation could not start",
            ),
            (
                "Het gekozen taalmodel heeft de voorbeeldtekst verwerkt.",
                "The selected language model processed the sample text.",
            ),
            (
                "Het gekozen spraakmodel heeft het audiovoorbeeld verwerkt.",
                "The selected speech model processed the audio sample.",
            ),
        ] {
            assert_eq!(
                pennel_core::localize_ui_text(InterfaceLanguage::English, source),
                english
            );
            assert_eq!(
                pennel_core::localize_ui_text(InterfaceLanguage::Dutch, source),
                source
            );
        }
    }

    #[test]
    fn linux_message_locale_respects_precedence_and_language_priority() {
        assert_eq!(
            linux_languages(Some("fr: nl-BE :en"), None, None, Some("fr_BE.UTF-8")),
            ["fr", "nl-BE", "en"]
        );
        assert_eq!(
            linux_languages(None, Some("nl_NL.UTF-8"), Some("en_US"), Some("en_GB")),
            ["nl_NL.UTF-8"]
        );
        assert_eq!(
            linux_languages(None, Some(""), Some("nl_BE"), Some("en_GB")),
            ["nl_BE"]
        );
        assert_eq!(
            linux_languages(None, None, None, Some("nl_BE.UTF-8")),
            ["nl_BE.UTF-8"]
        );
        for locale in ["C", "POSIX", "C.UTF-8"] {
            assert_eq!(
                linux_languages(Some("nl"), Some(locale), None, Some("nl_BE")),
                ["en"]
            );
        }
        assert_eq!(linux_languages(None, None, None, None), ["en"]);
        assert_eq!(
            linux_languages(Some("  "), None, None, Some("en_US")),
            ["en_US"]
        );
    }

    #[test]
    fn original_menu_sources_support_repeated_language_switches() {
        let source = "&Copy";
        for (language, label) in [
            (InterfaceLanguage::English, "Copy"),
            (InterfaceLanguage::Dutch, "Kopieer"),
            (InterfaceLanguage::English, "Copy"),
        ] {
            assert_eq!(
                stock_menu_text(language, source, &["Pennel"]).as_deref(),
                Some(label)
            );
        }
    }

    #[test]
    fn stock_menu_labels_follow_interface_language_including_mnemonic_sources() {
        for (source, english, dutch) in [
            ("File", "File", "Bestand"),
            ("Edit", "Edit", "Bewerk"),
            ("View", "View", "Weergave"),
            ("Window", "Window", "Venster"),
            ("Help", "Help", "Help"),
            ("&Copy", "Copy", "Kopieer"),
            ("Select &All", "Select All", "Selecteer alles"),
            ("C&lose Window", "Close Window", "Sluit venster"),
            ("Services", "Services", "Voorzieningen"),
        ] {
            assert_eq!(
                stock_menu_text(InterfaceLanguage::English, source, &["Pennel"]).as_deref(),
                Some(english)
            );
            assert_eq!(
                stock_menu_text(InterfaceLanguage::Dutch, source, &["Pennel"]).as_deref(),
                Some(dutch)
            );
        }
    }

    #[test]
    fn app_menu_preserves_brand_names_and_ignores_unknown_or_os_managed_items() {
        let names = &["Pennel", "Pennel Preview", "Pennel & Notes"];
        assert_eq!(
            stock_menu_text(InterfaceLanguage::Dutch, "About Pennel Preview", names).as_deref(),
            Some("Over Pennel Preview")
        );
        assert_eq!(
            stock_menu_text(InterfaceLanguage::Dutch, "Hide Pennel & Notes", names).as_deref(),
            Some("Verberg Pennel & Notes")
        );
        assert_eq!(
            stock_menu_text(InterfaceLanguage::Dutch, "Quit Pennel && Notes", names).as_deref(),
            Some("Stop Pennel & Notes")
        );
        for source in [
            "Pennel",
            "Pennel Preview",
            "Pennel & Notes",
            "A custom service",
            "Meeting notes",
            "About Another App",
        ] {
            assert_eq!(
                stock_menu_text(InterfaceLanguage::Dutch, source, names),
                None
            );
        }
    }
}

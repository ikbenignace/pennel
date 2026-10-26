use serde::{Deserialize, Serialize};
use tauri::AppHandle;

#[cfg(not(target_os = "macos"))]
use tauri_plugin_dialog::DialogExt;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StyleApplication {
    pub id: String,
    pub name: String,
}

pub(crate) fn picker_labels(language: pennel_core::InterfaceLanguage) -> (String, String) {
    (
        pennel_core::ui_text(language, "backend.add_app", &[]),
        pennel_core::ui_text(language, "backend.picker.add", &[]),
    )
}

#[tauri::command]
pub async fn pick_style_application(app: AppHandle) -> Result<Option<StyleApplication>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        #[cfg(target_os = "macos")]
        {
            let _ = app;
            crate::macos::pick_style_application()
        }
        #[cfg(not(target_os = "macos"))]
        {
            let dialog = app
                .dialog()
                .file()
                .set_title(picker_labels(crate::ui::language()).0);
            #[cfg(windows)]
            let dialog = dialog.add_filter(crate::ui::message("Apps"), &["exe"]);
            #[cfg(target_os = "linux")]
            let dialog = dialog
                .set_directory("/usr/share/applications")
                .add_filter(crate::ui::message("Apps"), &["desktop"])
                .add_filter(crate::ui::message("Alle bestanden"), &["*"]);
            resolve_selection(dialog.blocking_pick_file(), |selection| {
                let path = selection.into_path().map_err(|error| error.to_string())?;
                selected_application(&path)
            })
        }
    })
    .await
    .map_err(|_| "De appkiezer is onderbroken.".to_owned())?
}

#[cfg(any(not(target_os = "macos"), test))]
fn resolve_selection<T>(
    selection: Option<T>,
    resolve: impl FnOnce(T) -> Result<StyleApplication, String>,
) -> Result<Option<StyleApplication>, String> {
    selection.map(resolve).transpose()
}

#[cfg(any(not(target_os = "macos"), test))]
fn selected_application(path: &std::path::Path) -> Result<StyleApplication, String> {
    let metadata =
        std::fs::metadata(path).map_err(|_| "De gekozen app is niet meer beschikbaar.")?;
    if !metadata.is_file() {
        return Err("Kies een appbestand.".into());
    }
    let filename = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("De appnaam bevat ongeldige tekst.")?;
    #[cfg(windows)]
    {
        if !filename.to_ascii_lowercase().ends_with(".exe") {
            return Err("Kies een Windows-app (.exe).".into());
        }
        executable_application(filename)
    }
    #[cfg(not(windows))]
    {
        use std::io::Read;
        use std::os::unix::fs::PermissionsExt;

        if path
            .extension()
            .is_some_and(|extension| extension == "desktop")
        {
            let mut text = String::new();
            std::fs::File::open(path)
                .and_then(|file| file.take(65_537).read_to_string(&mut text))
                .map_err(|_| "Het appbestand kan niet worden gelezen.")?;
            if text.len() > 65_536 {
                return Err("Het appbestand is te groot.".into());
            }
            desktop_application(&text)
        } else if metadata.permissions().mode() & 0o111 != 0 {
            executable_application(filename)
        } else {
            Err("Kies een .desktop-app of een uitvoerbaar appbestand.".into())
        }
    }
}

#[cfg(any(not(target_os = "macos"), test))]
fn executable_application(filename: &str) -> Result<StyleApplication, String> {
    if filename.is_empty()
        || filename.len() > 255
        || filename.starts_with('-')
        || filename.trim() != filename
        || !filename
            .chars()
            .all(|character| character.is_alphanumeric() || "._+- ".contains(character))
        || matches!(filename, "." | "..")
    {
        return Err("Deze app-ID is niet bruikbaar. Vul de app-ID in bij Geavanceerd.".into());
    }
    let lowercase = filename.to_lowercase();
    let name = if lowercase.ends_with(".exe") {
        &filename[..filename.len() - 4]
    } else {
        filename
    };
    if name.is_empty() {
        return Err("Kies een app met een geldige naam.".into());
    }
    Ok(StyleApplication {
        id: crate::foreground::style_id(&lowercase),
        name: name.to_owned(),
    })
}

#[cfg(any(target_os = "linux", test))]
fn desktop_application(text: &str) -> Result<StyleApplication, String> {
    let (mut in_entry, mut app_type, mut name, mut exec) = (false, None, None, None);
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
        } else if in_entry
            && !line.starts_with('#')
            && let Some((key, value)) = line.split_once('=')
        {
            match key.trim() {
                "Type" => app_type = Some(value),
                "Name" => name = Some(value),
                "Exec" => exec = Some(value),
                _ => {}
            }
        }
    }
    if app_type != Some("Application") {
        return Err("Dit bestand is geen app.".into());
    }
    let name = name.unwrap_or("").trim();
    if name.is_empty() || name.len() > 255 || name.chars().any(char::is_control) {
        return Err("De app heeft geen bruikbare naam.".into());
    }
    let mut words = desktop_exec_words(exec.unwrap_or(""))?.into_iter();
    let mut command = words.next().ok_or("De app heeft geen startopdracht.")?;
    if command.rsplit('/').next() == Some("env") {
        command = words
            .find(|word| !word.contains('=') && word != "--")
            .ok_or("De app heeft geen uitvoerbaar bestand.")?;
    }
    let filename = command.rsplit('/').next().unwrap_or("");
    if matches!(
        filename,
        "flatpak" | "snap" | "sh" | "bash" | "dash" | "zsh"
    ) {
        return Err(
            "Deze app gebruikt een startprogramma. Vul de app-ID in bij Geavanceerd.".into(),
        );
    }
    let mut application = executable_application(filename)?;
    application.name = name.to_owned();
    Ok(application)
}

#[cfg(any(target_os = "linux", test))]
fn desktop_exec_words(exec: &str) -> Result<Vec<String>, String> {
    let (mut words, mut word, mut quoted, mut escaped) = (Vec::new(), String::new(), false, false);
    for character in exec.chars() {
        if escaped {
            word.push(character);
            escaped = false;
        } else {
            match character {
                '\\' => escaped = true,
                '"' => quoted = !quoted,
                character if character.is_whitespace() && !quoted => {
                    if !word.is_empty() {
                        words.push(std::mem::take(&mut word));
                    }
                }
                character => word.push(character),
            }
        }
    }
    if quoted || escaped {
        return Err("De startopdracht van de app is ongeldig.".into());
    }
    if !word.is_empty() {
        words.push(word);
    }
    Ok(words)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_picker_uses_interface_labels_in_both_languages() {
        use pennel_core::InterfaceLanguage::{Dutch, English};
        assert_eq!(picker_labels(English), ("Add app…".into(), "Add".into()));
        assert_eq!(
            picker_labels(Dutch),
            ("App toevoegen…".into(), "Toevoegen".into())
        );
        assert_eq!(picker_labels(English), ("Add app…".into(), "Add".into()));
    }

    #[test]
    fn cancellation_never_resolves_or_changes_a_rule() {
        let selected: Result<Option<StyleApplication>, String> =
            resolve_selection(None::<()>, |_| {
                panic!("an annulled selection must not inspect a file")
            });
        assert_eq!(selected, Ok(None));
        assert_eq!(
            resolve_selection(Some(()), |_| Err("invalid".into())),
            Err("invalid".into())
        );
    }

    #[test]
    fn executable_selection_uses_the_same_identity_as_foreground_detection() {
        assert_eq!(
            executable_application("Slack.EXE").unwrap(),
            StyleApplication {
                id: crate::foreground::style_id("slack.exe"),
                name: "Slack".into(),
            }
        );
        assert_eq!(executable_application("notepad.exe").unwrap().id, "notepad");
        assert_eq!(
            executable_application("My.AppImage").unwrap().id,
            "my.appimage"
        );
        assert_eq!(
            executable_application("Docker Desktop.exe").unwrap().id,
            "docker desktop"
        );
        for invalid in [
            "",
            ".",
            "..",
            ".exe",
            "../code",
            "-u",
            "code\n",
            "file;action",
        ] {
            assert!(executable_application(invalid).is_err());
        }
    }

    #[test]
    fn desktop_entry_uses_app_name_and_command_identity_without_executing_it() {
        let app = desktop_application("[Desktop Entry]\nType=Application\nName=Visual Studio Code\nExec=\"/opt/Visual Studio Code/code\" --reuse-window %F\n[Desktop Action New]\nExec=wrong\n").unwrap();
        assert_eq!(app.id, "com.microsoft.VSCode");
        assert_eq!(app.name, "Visual Studio Code");
        let app = desktop_application("[Desktop Entry]\nType=Application\nName=Slack\nExec=env LANG=nl_BE.UTF-8 /opt/slack/slack %U\n").unwrap();
        assert_eq!(app.id, "com.tinyspeck.slackmacgap");
    }

    #[test]
    fn desktop_entries_reject_links_launchers_and_invalid_commands() {
        for entry in [
            "[Desktop Entry]\nType=Link\nName=Code\nExec=code",
            "[Desktop Entry]\nType=Application\nName=Code\nExec=\"code",
            "[Desktop Entry]\nType=Application\nName=Code\nExec=flatpak run com.code",
            "[Desktop Entry]\nType=Application\nName=Code\nExec=sh -c code",
            "[Desktop Entry]\nType=Application\nName=Code\nExec=env",
            "[Desktop Entry]\nType=Application\nName=Code\nExec=%U",
        ] {
            assert!(desktop_application(entry).is_err(), "accepted {entry:?}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn selected_desktop_file_is_read_only_and_bounded() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("code.desktop");
        std::fs::write(
            &path,
            "[Desktop Entry]\nType=Application\nName=Code\nExec=code %F\n",
        )
        .unwrap();
        assert_eq!(
            selected_application(&path).unwrap().id,
            "com.microsoft.VSCode"
        );
        std::fs::write(&path, "x".repeat(65_537)).unwrap();
        assert!(
            selected_application(&path)
                .unwrap_err()
                .contains("te groot")
        );
        assert!(selected_application(directory.path()).is_err());
        assert!(selected_application(&directory.path().join("missing.desktop")).is_err());
    }
}

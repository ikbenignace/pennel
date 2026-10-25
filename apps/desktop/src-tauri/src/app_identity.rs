use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppIdentity {
    pub name: String,
    /// PNG data URL of the application's own icon.
    pub icon: String,
}

const ICON_POINTS: i32 = 20;

fn cache() -> &'static Mutex<HashMap<String, Option<AppIdentity>>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Option<AppIdentity>>>> = OnceLock::new();
    CACHE.get_or_init(Default::default)
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 255
        && !id
            .chars()
            .any(|c| c.is_control() || matches!(c, '/' | '\\' | ':'))
}

fn cached(id: &str, lookup: impl FnOnce(&str) -> Option<AppIdentity>) -> Option<AppIdentity> {
    if !valid_id(id) {
        return None;
    }
    if let Some(hit) = cache().lock().unwrap().get(id) {
        return hit.clone();
    }
    let found = lookup(id);
    cache().lock().unwrap().insert(id.to_owned(), found.clone());
    found
}

#[cfg(target_os = "macos")]
fn lookup(id: &str) -> Option<AppIdentity> {
    crate::macos::app_identity(id, ICON_POINTS)
}

/// Windows and Linux show the shared generic app glyph until a light native lookup is worth adding.
#[cfg(not(target_os = "macos"))]
fn lookup(_id: &str) -> Option<AppIdentity> {
    let _ = ICON_POINTS;
    None
}

#[tauri::command]
pub async fn app_identity(id: String) -> Option<AppIdentity> {
    tauri::async_runtime::spawn_blocking(move || cached(&id, lookup))
        .await
        .ok()
        .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn rejects_paths_and_empty_ids_without_lookup() {
        for id in [
            "",
            "../Applications/Mail.app",
            "C:\\Windows\\notepad.exe",
            "a\nb",
        ] {
            assert_eq!(cached(id, |_| panic!("unexpected lookup")), None);
        }
    }

    #[test]
    fn caches_hits_and_misses() {
        static CALLS: AtomicUsize = AtomicUsize::new(0);
        let found = |_: &str| {
            CALLS.fetch_add(1, Ordering::SeqCst);
            Some(AppIdentity {
                name: "Voorbeeld".into(),
                icon: "data:image/png;base64,AA==".into(),
            })
        };
        let first = cached("dev.pennel.test.cache", found);
        let second = cached("dev.pennel.test.cache", found);
        assert_eq!(first, second);
        assert_eq!(CALLS.load(Ordering::SeqCst), 1);
        let missing = |_: &str| {
            CALLS.fetch_add(1, Ordering::SeqCst);
            None
        };
        assert_eq!(cached("dev.pennel.test.missing", missing), None);
        assert_eq!(cached("dev.pennel.test.missing", missing), None);
        assert_eq!(CALLS.load(Ordering::SeqCst), 2);
    }
}

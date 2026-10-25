//! Port of `PennelKit/PasteboardGuard.swift` plus the copy-based selection read of `SelectionReader.swift`.

use std::time::{Duration, Instant};

/// Apps read the clipboard whenever they handle Ctrl+V; busy Electron apps can take several hundred ms.
pub const RESTORE_DELAY: Duration = Duration::from_millis(700);
const COPY_WAIT: Duration = Duration::from_millis(300);
pub const LATE_COPY_WATCH: Duration = Duration::from_secs(2);
const POLL: Duration = Duration::from_millis(20);
const SENTINEL_PREFIX: &str = "\u{2063}pennel-selection-";

pub trait Clipboard {
    type Snapshot: Clone;

    /// Changes whenever any app writes the clipboard.
    fn fingerprint(&mut self) -> u64;
    fn snapshot(&mut self) -> Self::Snapshot;
    fn restore(&mut self, snapshot: &Self::Snapshot);
    /// Writes text marked so clipboard managers and Windows clipboard history skip it.
    fn set_transient_text(&mut self, text: &str);
    fn text(&mut self) -> Option<String>;
}

/// Puts a secret on the clipboard marked so clipboard managers and Windows clipboard history skip it.
pub fn copy_secret<C: Clipboard>(clipboard: &mut C, text: &str) {
    clipboard.set_transient_text(text);
}

pub struct Token<S> {
    saved: S,
    fingerprint_after_set: u64,
}

pub fn begin<C: Clipboard>(clipboard: &mut C, text: &str) -> Token<C::Snapshot> {
    let saved = clipboard.snapshot();
    clipboard.set_transient_text(text);
    Token {
        saved,
        fingerprint_after_set: clipboard.fingerprint(),
    }
}

/// Restores the user's clipboard unless they copied something new in the meantime.
pub fn end<C: Clipboard>(clipboard: &mut C, token: Token<C::Snapshot>) {
    if clipboard.fingerprint() == token.fingerprint_after_set {
        clipboard.restore(&token.saved);
    }
}

/// A copy that arrived after we gave up waiting; the user's clipboard goes back once it lands.
pub struct LateCopyWatch<S> {
    saved: S,
    fingerprint: u64,
    until: Instant,
}

impl<S> LateCopyWatch<S> {
    pub fn expired(&self, now: Instant) -> bool {
        now >= self.until
    }

    pub fn check<C: Clipboard<Snapshot = S>>(&self, clipboard: &mut C) -> bool {
        if clipboard.fingerprint() == self.fingerprint {
            return false;
        }
        clipboard.restore(&self.saved);
        true
    }
}

pub struct SelectionRead<S> {
    pub text: Option<String>,
    pub late_watch: Option<LateCopyWatch<S>>,
}

/// Copies the selection with `send_copy` and puts the user's clipboard back.
/// A unique sentinel goes on the clipboard first, so an unchanged clipboard reliably means nothing was selected.
pub fn read_selection<C: Clipboard>(
    clipboard: &mut C,
    send_copy: impl FnOnce(),
    nonce: u64,
) -> SelectionRead<C::Snapshot> {
    let saved = clipboard.snapshot();
    let sentinel = format!("{SENTINEL_PREFIX}{nonce}");
    clipboard.set_transient_text(&sentinel);
    let before = clipboard.fingerprint();
    send_copy();
    let deadline = Instant::now() + COPY_WAIT;
    while clipboard.fingerprint() == before && Instant::now() < deadline {
        std::thread::sleep(POLL);
    }
    if clipboard.fingerprint() == before {
        clipboard.restore(&saved);
        let fingerprint = clipboard.fingerprint();
        return SelectionRead {
            text: None,
            late_watch: Some(LateCopyWatch {
                saved,
                fingerprint,
                until: Instant::now() + LATE_COPY_WATCH,
            }),
        };
    }
    let text = clipboard.text();
    clipboard.restore(&saved);
    SelectionRead {
        text: text.filter(|t| !t.is_empty() && !t.starts_with(SENTINEL_PREFIX)),
        late_watch: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    /// `pending_copy` is what the focused app writes when it receives Ctrl+C.
    #[derive(Default)]
    struct FakeClipboard {
        contents: Option<String>,
        writes: u64,
        transient_writes: u64,
        pending_copy: Rc<RefCell<Option<String>>>,
    }

    impl FakeClipboard {
        fn user_copies(&mut self, text: &str) {
            self.contents = Some(text.into());
            self.writes += 1;
        }
    }

    impl Clipboard for FakeClipboard {
        type Snapshot = Option<String>;

        fn fingerprint(&mut self) -> u64 {
            if let Some(copied) = self.pending_copy.borrow_mut().take() {
                self.contents = Some(copied);
                self.writes += 1;
            }
            self.writes
        }

        fn snapshot(&mut self) -> Option<String> {
            self.contents.clone()
        }

        fn restore(&mut self, snapshot: &Option<String>) {
            self.contents = snapshot.clone();
            self.writes += 1;
        }

        fn set_transient_text(&mut self, text: &str) {
            self.transient_writes += 1;
            self.contents = Some(text.into());
            self.writes += 1;
        }

        fn text(&mut self) -> Option<String> {
            self.contents.clone()
        }
    }

    fn copying(c: &FakeClipboard, selection: &str) -> impl FnOnce() + use<> {
        let pending = c.pending_copy.clone();
        let selection = selection.to_owned();
        move || *pending.borrow_mut() = Some(selection)
    }

    #[test]
    fn secrets_are_written_through_the_history_excluding_path() {
        let mut c = FakeClipboard::default();
        copy_secret(&mut c, "ABCD-EFGH");
        assert_eq!(c.transient_writes, 1);
        assert_eq!(c.contents.as_deref(), Some("ABCD-EFGH"));
    }

    #[test]
    fn restores_previous_contents() {
        let mut c = FakeClipboard::default();
        c.user_copies("mine");
        let token = begin(&mut c, "dictated");
        assert_eq!(c.contents.as_deref(), Some("dictated"));
        end(&mut c, token);
        assert_eq!(c.contents.as_deref(), Some("mine"));
    }

    #[test]
    fn keeps_what_the_user_copied_in_between() {
        let mut c = FakeClipboard::default();
        c.user_copies("mine");
        let token = begin(&mut c, "dictated");
        c.user_copies("newer");
        end(&mut c, token);
        assert_eq!(c.contents.as_deref(), Some("newer"));
    }

    #[test]
    fn empty_clipboard_is_restored_empty() {
        let mut c = FakeClipboard::default();
        let token = begin(&mut c, "dictated");
        end(&mut c, token);
        assert_eq!(c.contents, None);
    }

    #[test]
    fn selection_is_read_and_clipboard_restored() {
        let mut c = FakeClipboard::default();
        c.user_copies("mine");
        let copy = copying(&c, "selected words");
        let read = read_selection(&mut c, copy, 1);
        assert_eq!(read.text.as_deref(), Some("selected words"));
        assert!(read.late_watch.is_none());
        assert_eq!(c.contents.as_deref(), Some("mine"));
    }

    #[test]
    fn selection_equal_to_clipboard_is_still_detected() {
        let mut c = FakeClipboard::default();
        c.user_copies("same");
        let copy = copying(&c, "same");
        let read = read_selection(&mut c, copy, 2);
        assert_eq!(read.text.as_deref(), Some("same"));
        assert_eq!(c.contents.as_deref(), Some("same"));
    }

    #[test]
    fn nothing_selected_restores_and_watches_for_a_late_copy() {
        let mut c = FakeClipboard::default();
        c.user_copies("mine");
        let read = read_selection(&mut c, || {}, 3);
        assert_eq!(read.text, None);
        assert_eq!(c.contents.as_deref(), Some("mine"));
        let watch = read.late_watch.unwrap();
        assert!(!watch.check(&mut c));
        c.user_copies("late selection");
        assert!(watch.check(&mut c));
        assert_eq!(c.contents.as_deref(), Some("mine"));
    }
}

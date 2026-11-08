//! Preserve successful manual-save intent across windows in this process.
//! Validation stays outside this lock; settings retain their cross-process lock.
use std::{
    collections::BTreeMap,
    sync::{atomic::{AtomicU64, Ordering}, Mutex},
};

#[derive(Clone, Copy)]
pub(super) struct Request(u64);

struct Saves {
    next: AtomicU64,
    committed: Mutex<BTreeMap<String, u64>>,
}

impl Saves {
    const fn new() -> Self {
        Self { next: AtomicU64::new(0), committed: Mutex::new(BTreeMap::new()) }
    }

    fn begin(&self) -> Result<Request, String> {
        self.next.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
            .map(Request)
            .map_err(|_| "Too many profile saves; restart Matrix Play".into())
    }

    fn commit<T>(&self, request: Request, id: &str, write: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
        let mut committed = self.committed.lock()
            .map_err(|_| "Profile saving is unavailable; restart Matrix Play".to_string())?;
        if committed.get(id).is_some_and(|saved| *saved > request.0) {
            return Err("A newer emulator choice was already saved. This older save was not applied.".into());
        }
        let result = write()?;
        committed.insert(id.into(), request.0);
        Ok(result)
    }
}

static SAVES: Saves = Saves::new();

pub(super) fn begin() -> Result<Request, String> {
    SAVES.begin()
}

pub(super) fn commit<T>(request: Request, id: &str, write: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    SAVES.commit(request, id, write)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delayed_older_save_cannot_overwrite_a_newer_success() {
        let saves = Saves::new();
        let old = saves.begin().unwrap();
        let new = saves.begin().unwrap();
        let value = Mutex::new(String::new());
        let (ready, ready_rx) = std::sync::mpsc::channel();
        let (release, release_rx) = std::sync::mpsc::channel();
        std::thread::scope(|scope| {
            let saves = &saves;
            let value = &value;
            let delayed = scope.spawn(move || {
                // A's slow validation has started, but has not acquired the commit lock.
                ready.send(()).unwrap();
                release_rx.recv().unwrap();
                saves.commit(old, "ps2", || { *value.lock().unwrap() = "old".into(); Ok(()) })
            });
            ready_rx.recv().unwrap();
            saves.commit(new, "ps2", || { *value.lock().unwrap() = "new".into(); Ok(()) }).unwrap();
            release.send(()).unwrap();
            assert!(delayed.join().unwrap().unwrap_err().contains("newer emulator choice"));
        });
        assert_eq!(*value.lock().unwrap(), "new");
    }

    #[test]
    fn failed_newer_save_does_not_suppress_older_valid_choice() {
        let saves = Saves::new();
        let old = saves.begin().unwrap();
        let new = saves.begin().unwrap();
        let result: Result<(), String> = saves.commit(new, "ps2", || Err("write failed".into()));
        assert!(result.is_err());
        assert_eq!(saves.commit(old, "ps2", || Ok("old")).unwrap(), "old");
    }

    #[test]
    fn different_consoles_and_ordered_successes_remain_independent() {
        let saves = Saves::new();
        let old = saves.begin().unwrap();
        let new = saves.begin().unwrap();
        saves.commit(new, "n64", || Ok(())).unwrap();
        saves.commit(old, "ps2", || Ok(())).unwrap();
        saves.commit(new, "ps2", || Ok(())).unwrap();
    }
}

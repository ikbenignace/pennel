use std::collections::HashMap;

/// Releases results in the order their recordings started, even when they finish out of order.
#[derive(Debug)]
pub struct PasteQueue<T> {
    next_reservation: u64,
    next_release: u64,
    pending: HashMap<u64, Option<T>>,
}

impl<T> Default for PasteQueue<T> {
    fn default() -> Self {
        Self {
            next_reservation: 0,
            next_release: 0,
            pending: HashMap::new(),
        }
    }
}

impl<T> PasteQueue<T> {
    pub fn reserve(&mut self) -> u64 {
        let seq = self.next_reservation;
        self.next_reservation += 1;
        seq
    }

    pub fn fulfill(&mut self, seq: u64, value: Option<T>) -> Vec<T> {
        self.pending.insert(seq, value);
        let mut ready = Vec::new();
        while let Some(entry) = self.pending.remove(&self.next_release) {
            ready.extend(entry);
            self.next_release += 1;
        }
        ready
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_order_results_release_immediately() {
        let mut q = PasteQueue::default();
        let a = q.reserve();
        assert_eq!(q.fulfill(a, Some("a")), ["a"]);
    }

    #[test]
    fn later_result_waits_for_earlier_one() {
        let mut q = PasteQueue::default();
        let a = q.reserve();
        let b = q.reserve();
        assert!(q.fulfill(b, Some("b")).is_empty());
        assert_eq!(q.fulfill(a, Some("a")), ["a", "b"]);
    }

    #[test]
    fn failed_result_unblocks_without_pasting() {
        let mut q = PasteQueue::default();
        let a = q.reserve();
        let b = q.reserve();
        assert!(q.fulfill(b, Some("b")).is_empty());
        assert_eq!(q.fulfill(a, None), ["b"]);
    }
}

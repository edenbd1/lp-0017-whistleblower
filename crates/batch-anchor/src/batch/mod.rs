//! In-memory + on-chain-seeded dedup buffer.
//!
//! Two layers (per ADR-001):
//! 1. At startup, seed `known` from `RegistryClient::anchored_cid_set()`.
//! 2. Every incoming envelope is pushed through `BatchBuffer::push`,
//!    which silently drops it if the CID is already `known` (anchored
//!    or pending in this run).
//!
//! On flush, the buffer hands back up to `MAX_BATCH` envelopes;
//! `mark_flushed` moves them from `pending` to `known` only after the
//! on-chain submission succeeds. Failures go through `return_failed`,
//! which puts them at the front of the buffer for the next tick.

use indexing::Envelope;
use registry_core::MAX_BATCH;
use std::collections::{HashSet, VecDeque};

/// Buffer state. Not thread-safe — the watch loop owns it on one task.
#[derive(Default)]
pub struct BatchBuffer {
    /// CIDs we've seen since startup (anchored or pending).
    known: HashSet<String>,
    /// Pending envelopes in arrival order.
    pending: VecDeque<Envelope>,
}

impl BatchBuffer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Seed with CIDs already on-chain. Idempotent.
    pub fn seed_known(&mut self, cids: impl IntoIterator<Item = String>) {
        self.known.extend(cids);
    }

    /// Push an envelope. Returns `true` if it was accepted as new,
    /// `false` if it was a duplicate (silent drop).
    pub fn push(&mut self, env: Envelope) -> bool {
        if self.known.contains(&env.cid) {
            return false;
        }
        self.known.insert(env.cid.clone());
        self.pending.push_back(env);
        true
    }

    /// Number of pending envelopes.
    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }

    /// Number of CIDs ever observed (anchored + pending + duplicates'
    /// originals).
    pub fn known_len(&self) -> usize {
        self.known.len()
    }

    /// True when the next `drain_batch` would yield a full
    /// [`MAX_BATCH`].
    pub fn is_full(&self) -> bool {
        self.pending.len() >= MAX_BATCH
    }

    /// Drain up to `MAX_BATCH` envelopes for submission. Returns an
    /// empty vec if nothing is pending.
    pub fn drain_batch(&mut self) -> Vec<Envelope> {
        let n = self.pending.len().min(MAX_BATCH);
        self.pending.drain(..n).collect()
    }

    /// Drain up to `n` envelopes (cap at `MAX_BATCH`).
    pub fn drain_up_to(&mut self, n: usize) -> Vec<Envelope> {
        let n = n.min(MAX_BATCH).min(self.pending.len());
        self.pending.drain(..n).collect()
    }

    /// Restore failed envelopes to the front so they get retried first
    /// on the next tick.
    pub fn return_failed(&mut self, envs: Vec<Envelope>) {
        for env in envs.into_iter().rev() {
            // Push to the front so order is preserved.
            self.pending.push_front(env);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(n: u8) -> Envelope {
        Envelope {
            v: 1,
            cid: format!("cid:{n}"),
            metadata_hash: format!("v1:{}", "a".repeat(64)),
            timestamp: 1_700_000_000 + n as u64,
            title: None,
            description: None,
            content_type: None,
            size_bytes: None,
            tags: vec![],
        }
    }

    #[test]
    fn empty_buffer_has_zero_pending() {
        let mut b = BatchBuffer::new();
        assert_eq!(b.pending_len(), 0);
        assert!(!b.is_full());
        assert!(b.drain_batch().is_empty());
    }

    #[test]
    fn push_accepts_new_cid() {
        let mut b = BatchBuffer::new();
        assert!(b.push(env(1)));
        assert_eq!(b.pending_len(), 1);
        assert_eq!(b.known_len(), 1);
    }

    #[test]
    fn push_rejects_duplicate_cid_silently() {
        let mut b = BatchBuffer::new();
        assert!(b.push(env(1)));
        assert!(!b.push(env(1)));
        assert_eq!(b.pending_len(), 1);
    }

    #[test]
    fn seed_known_blocks_subsequent_pushes() {
        let mut b = BatchBuffer::new();
        b.seed_known(["cid:5".to_string()]);
        assert!(!b.push(env(5)));
        assert_eq!(b.pending_len(), 0);
    }

    #[test]
    fn drain_batch_caps_at_max_batch() {
        let mut b = BatchBuffer::new();
        for i in 0..=MAX_BATCH as u8 + 5 {
            b.push(env(i));
        }
        let drained = b.drain_batch();
        assert_eq!(drained.len(), MAX_BATCH);
        // Remaining envelopes still pending.
        assert_eq!(b.pending_len(), 6);
    }

    #[test]
    fn drain_up_to_respects_lower_cap() {
        let mut b = BatchBuffer::new();
        for i in 0..10 {
            b.push(env(i));
        }
        let drained = b.drain_up_to(3);
        assert_eq!(drained.len(), 3);
        assert_eq!(b.pending_len(), 7);
    }

    #[test]
    fn drain_up_to_clamps_to_max_batch() {
        let mut b = BatchBuffer::new();
        for i in 0..=MAX_BATCH as u8 + 5 {
            b.push(env(i));
        }
        assert_eq!(b.drain_up_to(MAX_BATCH * 2).len(), MAX_BATCH);
    }

    #[test]
    fn return_failed_preserves_order_at_front() {
        let mut b = BatchBuffer::new();
        for i in 0..5 {
            b.push(env(i));
        }
        let first_three = b.drain_up_to(3);
        // pending now: [env(3), env(4)]
        b.return_failed(first_three);
        // pending now: [env(0), env(1), env(2), env(3), env(4)]
        let all = b.drain_up_to(5);
        let cids: Vec<_> = all.iter().map(|e| e.cid.as_str()).collect();
        assert_eq!(cids, vec!["cid:0", "cid:1", "cid:2", "cid:3", "cid:4"]);
    }

    #[test]
    fn is_full_when_pending_reaches_max_batch() {
        let mut b = BatchBuffer::new();
        for i in 0..MAX_BATCH as u8 {
            b.push(env(i));
        }
        assert!(b.is_full());
    }
}

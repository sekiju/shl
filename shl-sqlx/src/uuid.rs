use chrono::{DateTime, Utc};
use once_cell::sync::Lazy;
use std::sync::atomic::{AtomicU32, Ordering};
use uuid::{ClockSequence, Timestamp, Uuid};

pub struct UuidV7Context(AtomicU32);

static CONTEXT: Lazy<UuidV7Context> = Lazy::new(UuidV7Context::default);

impl UuidV7Context {
    pub fn new() -> Self {
        Self(AtomicU32::new(0))
    }

    pub fn with_initial_counter(initial: u32) -> Self {
        Self(AtomicU32::new(initial & 0xFFF))
    }
}

impl Default for UuidV7Context {
    fn default() -> Self {
        Self::new()
    }
}

impl ClockSequence for UuidV7Context {
    type Output = u32;

    fn generate_sequence(&self, _seconds: u64, _subsec_nanos: u32) -> Self::Output {
        self.0.fetch_add(1, Ordering::Relaxed) & 0xFFF
    }

    fn usable_bits(&self) -> usize {
        12
    }
}

pub fn uuidv7() -> Uuid {
    let now = Utc::now();
    let ts = Timestamp::from_unix(&*CONTEXT, now.timestamp() as u64, now.timestamp_subsec_nanos());
    Uuid::new_v7(ts)
}

pub fn uuidv7_and_created_at() -> (Uuid, DateTime<Utc>) {
    let now = Utc::now();
    let ts = Timestamp::from_unix(&*CONTEXT, now.timestamp() as u64, now.timestamp_subsec_nanos());
    (Uuid::new_v7(ts), now)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uuid7_different() {
        let (id1, _) = uuidv7_and_created_at();
        let (id2, _) = uuidv7_and_created_at();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_uuid7_monotonic() {
        let (id1, _) = uuidv7_and_created_at();
        let (id2, _) = uuidv7_and_created_at();
        assert!(id1.as_bytes() < id2.as_bytes());
    }
}

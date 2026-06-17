//! 时钟抽象：注入时钟以保证存档/检查点时间戳可测且确定。
//!
//! 生产用 [`SystemClock`]（chrono UTC），测试用 [`FakeClock`]（单调递增）。

use std::sync::atomic::{AtomicU64, Ordering};

/// 时间源。
pub trait Clock: Send + Sync {
    /// ISO 8601 时间戳（存档/检查点），如 `2026-06-16T12:00:00Z`。
    fn now_iso(&self) -> String;
    /// 文件名安全的时间戳（损坏文件另存用），如 `20260616T120000Z`。
    fn now_stamp(&self) -> String;
}

/// 系统时钟（UTC）。
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_iso(&self) -> String {
        chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
    }
    fn now_stamp(&self) -> String {
        chrono::Utc::now().format("%Y%m%dT%H%M%SZ").to_string()
    }
}

/// 测试用假时钟：每次调用单调递增，保证唯一与确定性。
#[derive(Debug, Default)]
pub struct FakeClock {
    base: AtomicU64,
}

impl FakeClock {
    pub fn new() -> Self {
        Self::default()
    }
    fn next(&self) -> u64 {
        self.base.fetch_add(1, Ordering::SeqCst)
    }
}

impl Clock for FakeClock {
    fn now_iso(&self) -> String {
        let n = self.next();
        format!("2026-01-01T00:{:02}:{:02}Z", (n / 60) % 60, n % 60)
    }
    fn now_stamp(&self) -> String {
        let n = self.next();
        format!("20260101T0000{n:02}Z")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_clock_increments() {
        let c = FakeClock::new();
        let a = c.now_iso();
        let b = c.now_iso();
        assert_ne!(a, b);
        assert!(a.starts_with("2026-01-01T00:"));
    }

    #[test]
    fn system_clock_iso_format() {
        let s = SystemClock.now_iso();
        assert!(s.ends_with('Z'));
        assert_eq!(s.len(), "2026-06-16T12:00:00Z".len());
    }
}

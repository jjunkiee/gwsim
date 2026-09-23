//! Time, behind a trait, so the crawl's waiting can be tested instantly.
//!
//! The extractor's politeness is almost entirely about waiting: 3 s between
//! requests (EXT-3) and 30, 60 and 120 s of back-off (EXT-6). Tests that
//! really slept would take minutes, so every wait goes through [`Clock`], and
//! [`FakeClock`] advances its own time instead of sleeping.

use std::cell::{Cell, RefCell};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// A source of the current time that can also wait.
pub trait Clock {
    /// Milliseconds since the Unix epoch.
    fn now_ms(&self) -> u64;
    /// Waits for a duration.
    fn sleep(&self, duration: Duration);
}

/// The real clock.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_ms(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|elapsed| elapsed.as_millis() as u64)
            .unwrap_or(0)
    }

    fn sleep(&self, duration: Duration) {
        std::thread::sleep(duration);
    }
}

/// A clock that moves only when told to, or when something sleeps on it.
///
/// Every sleep is recorded, so a test can assert the exact back-off sequence
/// rather than only the total time.
#[derive(Debug)]
pub struct FakeClock {
    now: Cell<u64>,
    sleeps: RefCell<Vec<Duration>>,
}

impl FakeClock {
    /// A clock starting at a given time.
    pub fn at(now_ms: u64) -> Self {
        FakeClock {
            now: Cell::new(now_ms),
            sleeps: RefCell::new(Vec::new()),
        }
    }

    /// Moves time forward without it counting as a sleep.
    pub fn advance(&self, duration: Duration) {
        self.now.set(self.now.get() + duration.as_millis() as u64);
    }

    /// Every sleep so far, in order.
    pub fn sleeps(&self) -> Vec<Duration> {
        self.sleeps.borrow().clone()
    }
}

impl Default for FakeClock {
    /// 2026-09-23T00:00:00Z, so dates in test output are stable and plausible.
    fn default() -> Self {
        FakeClock::at(1_790_121_600_000)
    }
}

impl Clock for FakeClock {
    fn now_ms(&self) -> u64 {
        self.now.get()
    }

    fn sleep(&self, duration: Duration) {
        self.sleeps.borrow_mut().push(duration);
        self.advance(duration);
    }
}

/// A timestamp as `YYYY-MM-DDTHH:MM:SSZ`.
pub fn iso_datetime(ms: u64) -> String {
    let seconds = ms / 1000;
    let (year, month, day) = civil_from_days((seconds / 86_400) as i64);
    let rest = seconds % 86_400;
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rest / 3600,
        (rest % 3600) / 60,
        rest % 60
    )
}

/// A timestamp's date as `YYYY-MM-DD`.
pub fn iso_date(ms: u64) -> String {
    iso_datetime(ms)[..10].to_owned()
}

/// Parses `YYYY-MM-DDTHH:MM:SSZ` back into milliseconds.
pub fn parse_iso_datetime(text: &str) -> Option<u64> {
    let bytes = text.as_bytes();
    if bytes.len() != 20 || bytes[4] != b'-' || bytes[10] != b'T' || bytes[19] != b'Z' {
        return None;
    }
    let number = |range: std::ops::Range<usize>| text.get(range)?.parse::<u64>().ok();
    let year = number(0..4)? as i64;
    let month = number(5..7)? as u32;
    let day = number(8..10)? as u32;
    let hour = number(11..13)?;
    let minute = number(14..16)?;
    let second = number(17..19)?;
    let days = days_from_civil(year, month, day);
    let total = u64::try_from(days).ok()? * 86_400 + hour * 3600 + minute * 60 + second;
    Some(total * 1000)
}

/// Howard Hinnant's `civil_from_days`: days since 1970-01-01 to a date.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

/// The inverse of [`civil_from_days`].
fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let yoe = year.rem_euclid(400);
    let month = i64::from(month);
    let mp = if month > 2 { month - 3 } else { month + 9 };
    let doy = (153 * mp + 2) / 5 + i64::from(day) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fake_clock_records_sleeps_and_moves_forward() {
        let clock = FakeClock::at(1000);
        clock.sleep(Duration::from_secs(3));
        clock.sleep(Duration::from_millis(500));
        assert_eq!(clock.now_ms(), 4500);
        assert_eq!(
            clock.sleeps(),
            vec![Duration::from_secs(3), Duration::from_millis(500)]
        );
    }

    #[test]
    fn timestamps_format_as_utc() {
        assert_eq!(iso_datetime(0), "1970-01-01T00:00:00Z");
        assert_eq!(iso_datetime(1_790_121_600_000), "2026-09-23T00:00:00Z");
        assert_eq!(iso_date(1_790_121_600_000 + 86_399_000), "2026-09-23");
        // A leap day, which is where hand-rolled date code usually breaks.
        assert_eq!(iso_date(1_709_164_800_000), "2024-02-29");
    }

    #[test]
    fn timestamps_round_trip() {
        for ms in [0, 1_709_164_800_000, 1_790_121_612_000, 4_102_444_800_000] {
            assert_eq!(parse_iso_datetime(&iso_datetime(ms)), Some(ms));
        }
        assert_eq!(parse_iso_datetime("2026-09-23"), None);
    }
}

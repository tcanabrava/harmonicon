// SPDX-License-Identifier: MIT

//! Today's date, as a whole day number — the one wall-clock reading the
//! game makes, for spacing practice out over days.
//!
//! Everything else measures time on the game's own clocks. This reads the
//! wall clock through `web_time`, which is `std::time` natively and the
//! browser's clock on wasm, where `std::time::SystemTime::now()` panics.
//!
//! Days are counted in UTC from the Unix epoch. A day boundary that falls
//! at some local hour other than midnight only shifts when a review comes
//! due by a few hours, which spacing on the scale of days does not notice.

use web_time::{SystemTime, UNIX_EPOCH};

const SECS_PER_DAY: u64 = 86_400;

/// Days since 1970-01-01 (UTC) at `secs` seconds past the epoch.
pub fn day_of(secs: u64) -> u32 {
    (secs / SECS_PER_DAY) as u32
}

/// Today, as days since 1970-01-01 (UTC). A clock set before 1970 reads as
/// day 0 rather than failing: the worst it can do is make a review due.
pub fn today() -> u32 {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |elapsed| elapsed.as_secs());
    day_of(secs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_day_runs_from_midnight_to_midnight_utc() {
        assert_eq!(day_of(0), 0);
        assert_eq!(day_of(SECS_PER_DAY - 1), 0);
        assert_eq!(day_of(SECS_PER_DAY), 1);
        // 2026-09-25 00:00:00 UTC.
        assert_eq!(day_of(1_790_294_400), 20_721);
    }

    #[test]
    fn today_is_after_this_code_was_written() {
        assert!(today() >= 20_721);
    }
}

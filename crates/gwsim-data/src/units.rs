//! Units that data files are written in.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A duration, written in a data file as seconds and held as milliseconds.
///
/// Data files say `activation: 0.75` because that is how the wiki states it.
/// The engine works in integer milliseconds, because floating-point time
/// accumulates error and ENG determinism (§10.13) depends on two runs of the
/// same seed producing the same schedule.
///
/// Conversion happens once, at load, and a value that is not a whole number of
/// milliseconds is **rejected** rather than rounded. Silently rounding would
/// make a data file and the engine disagree about what the file says.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Seconds(u32);

impl Seconds {
    /// Zero.
    pub const ZERO: Seconds = Seconds(0);

    /// Builds a duration from whole milliseconds.
    pub const fn from_ms(milliseconds: u32) -> Self {
        Seconds(milliseconds)
    }

    /// The duration in whole milliseconds.
    pub const fn ms(self) -> u32 {
        self.0
    }

    /// The duration in seconds.
    pub fn as_secs_f64(self) -> f64 {
        f64::from(self.0) / 1000.0
    }

    /// Builds a duration from seconds, rejecting anything that is not a whole
    /// number of milliseconds.
    pub fn from_secs_f64(seconds: f64) -> Result<Self, SecondsError> {
        if !seconds.is_finite() {
            return Err(SecondsError {
                input: seconds,
                reason: "it is not a finite number",
            });
        }
        if seconds < 0.0 {
            return Err(SecondsError {
                input: seconds,
                reason: "it is negative",
            });
        }

        let milliseconds = seconds * 1000.0;
        let rounded = milliseconds.round();
        // A tolerance is needed because 0.1 * 1000.0 is 100.00000000000001 in
        // binary floating point. It is far tighter than half a millisecond, so
        // a genuinely sub-millisecond value still fails.
        if (milliseconds - rounded).abs() > 1e-6 {
            return Err(SecondsError {
                input: seconds,
                reason: "it is not a whole number of milliseconds",
            });
        }
        if rounded > f64::from(u32::MAX) {
            return Err(SecondsError {
                input: seconds,
                reason: "it is too large",
            });
        }

        Ok(Seconds(rounded as u32))
    }
}

impl fmt::Display for Seconds {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}s", self.as_secs_f64())
    }
}

/// Why a duration was rejected.
#[derive(Debug, Clone, PartialEq)]
pub struct SecondsError {
    input: f64,
    reason: &'static str,
}

impl fmt::Display for SecondsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} is not a usable duration: {}. Durations are written in seconds \
             and must land on a whole millisecond, as in 0.75 or 2.0",
            self.input, self.reason
        )
    }
}

impl std::error::Error for SecondsError {}

impl Serialize for Seconds {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_f64(self.as_secs_f64())
    }
}

impl<'de> Deserialize<'de> for Seconds {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let seconds = f64::deserialize(deserializer)?;
        Seconds::from_secs_f64(seconds).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whole_milliseconds_are_accepted() {
        for (seconds, milliseconds) in [
            (0.0, 0u32),
            (0.25, 250),
            (0.75, 750),
            (1.0, 1000),
            (2.0, 2000),
            (0.001, 1),
            (1.5, 1500),
            (30.0, 30_000),
        ] {
            let value = Seconds::from_secs_f64(seconds)
                .unwrap_or_else(|error| panic!("{seconds} should be usable: {error}"));
            assert_eq!(value.ms(), milliseconds, "{seconds} seconds");
        }
    }

    #[test]
    fn binary_floating_point_noise_does_not_cause_a_rejection() {
        // 0.1 * 1000.0 is 100.00000000000001, not 100. Without a tolerance
        // this would reject a perfectly ordinary recharge time.
        assert_eq!(Seconds::from_secs_f64(0.1).unwrap().ms(), 100);
        assert_eq!(Seconds::from_secs_f64(0.3).unwrap().ms(), 300);
        assert_eq!(Seconds::from_secs_f64(2.9).unwrap().ms(), 2900);
    }

    #[test]
    fn sub_millisecond_values_are_rejected_rather_than_rounded() {
        // The tolerance must not be so loose that real precision is lost.
        for seconds in [0.0001, 0.00075, 1.00025] {
            assert!(
                Seconds::from_secs_f64(seconds).is_err(),
                "{seconds} should be rejected"
            );
        }
    }

    #[test]
    fn negative_and_non_finite_values_are_rejected() {
        for seconds in [-1.0, -0.001, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(
                Seconds::from_secs_f64(seconds).is_err(),
                "{seconds} should be rejected"
            );
        }
    }

    #[test]
    fn round_trips_through_ron() {
        for milliseconds in [0u32, 1, 250, 750, 1000, 2500, 30_000] {
            let original = Seconds::from_ms(milliseconds);
            let text = ron::to_string(&original).unwrap();
            let back: Seconds = ron::from_str(&text).unwrap();
            assert_eq!(back, original, "{milliseconds} ms serialised as {text}");
        }
    }

    #[test]
    fn reads_the_way_a_data_file_writes_it() {
        #[derive(serde::Deserialize)]
        struct Holder {
            activation: Seconds,
            recharge: Seconds,
        }

        let holder: Holder = ron::from_str("(activation: 0.75, recharge: 20.0)").unwrap();
        assert_eq!(holder.activation.ms(), 750);
        assert_eq!(holder.recharge.ms(), 20_000);
    }

    #[test]
    fn an_unusable_duration_says_what_to_write_instead() {
        let error = ron::from_str::<Seconds>("0.00075").unwrap_err();
        let message = error.to_string();
        assert!(
            message.contains("whole millisecond"),
            "unhelpful message: {message}"
        );
    }

    #[test]
    fn ordering_is_by_duration() {
        assert!(Seconds::from_ms(250) < Seconds::from_ms(750));
        assert_eq!(Seconds::ZERO.ms(), 0);
    }
}

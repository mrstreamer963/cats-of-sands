/// Signed Q32.32 number. Its value is `raw / 2³²`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Fixed(i64);

impl Fixed {
    const FRACTION_BITS: u32 = 32;
    const SCALE: f64 = (1_u64 << Self::FRACTION_BITS) as f64;

    pub const fn from_int(value: i32) -> Self {
        Self((value as i64) << Self::FRACTION_BITS)
    }

    pub const fn from_raw(raw: i64) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> i64 {
        self.0
    }

    /// Quantizes to the nearest representable value, with ties away from zero.
    pub fn try_from_f64(value: f64) -> Option<Self> {
        let raw = (value * Self::SCALE).round();
        let lower = i64::MIN as f64;
        (lower..-lower).contains(&raw).then(|| Self(raw as i64))
    }

    pub fn to_f64(self) -> f64 {
        self.0 as f64 / Self::SCALE
    }
}

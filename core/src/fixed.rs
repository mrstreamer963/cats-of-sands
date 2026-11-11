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

    pub(crate) fn div_int(self, divisor: i64) -> Self {
        Self(self.0 / divisor)
    }

    pub(crate) fn add_wrapped(self, offset: Self, period: Self) -> Self {
        let sum = i128::from(self.0) + i128::from(offset.0);
        Self(sum.rem_euclid(i128::from(period.0)) as i64)
    }

    /// Advances a point toward its destination using wide intermediates.
    pub(crate) fn step_towards_2d(
        (x, y): (Self, Self),
        (target_x, target_y): (Self, Self),
        step: Self,
    ) -> (Self, Self) {
        let step = i128::from(step.0);
        if step <= 0 {
            return (x, y);
        }
        let dx = i128::from(target_x.0) - i128::from(x.0);
        let dy = i128::from(target_y.0) - i128::from(y.0);
        let distance = wide_distance(dx, dy);
        if distance <= step {
            return (target_x, target_y);
        }

        let offset_x = dx * step / distance;
        let offset_y = dy * step / distance;
        // A tiny step on a diagonal can round both components to zero.
        let (offset_x, offset_y) = match (offset_x, offset_y) {
            (0, 0) if dx.abs() >= dy.abs() => (dx.signum(), 0),
            (0, 0) => (0, dy.signum()),
            offsets => offsets,
        };
        (
            Self((i128::from(x.0) + offset_x) as i64),
            Self((i128::from(y.0) + offset_y) as i64),
        )
    }
}

fn wide_distance(dx: i128, dy: i128) -> i128 {
    // Halve extreme deltas before squaring so the sum fits in u128.
    let scale = if dx.abs().max(dy.abs()) > i128::from(i64::MAX) {
        2
    } else {
        1
    };
    let x = (dx / scale).unsigned_abs();
    let y = (dy / scale).unsigned_abs();
    let squared = x * x + y * y;
    let root = squared.isqrt();
    (root + u128::from(root * root != squared)) as i128 * scale
}

#[cfg(test)]
mod tests {
    use super::Fixed;

    #[test]
    fn step_towards_handles_deltas_wider_than_fixed() {
        let low = Fixed::from_raw(i64::MIN);
        let high = Fixed::from_raw(i64::MAX);
        let (x, y) = Fixed::step_towards_2d((low, high), (high, low), Fixed::from_int(1));
        assert_eq!(x.raw() - low.raw(), 3_037_000_499);
        assert_eq!(high.raw() - y.raw(), 3_037_000_499);
    }
}

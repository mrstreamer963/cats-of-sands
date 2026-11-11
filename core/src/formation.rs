use crate::{Fixed, Position};

pub(crate) fn destinations(
    point: Position,
    count: usize,
    radius: Fixed,
    (width, height): (Fixed, Fixed),
) -> impl Iterator<Item = Position> {
    let columns = (count - 1).isqrt() + 1;
    let rows = count.div_ceil(columns);
    let radius = i128::from(radius.raw());
    let spacing = 2 * radius + i128::from(Fixed::from_int(8).raw());
    let center_x = fit_axis(point.x, width, radius + (columns - 1) as i128 * spacing / 2);
    let center_y = fit_axis(point.y, height, radius + (rows - 1) as i128 * spacing / 2);

    (0..count).map(move |index| {
        let row = index / columns;
        let column = index % columns;
        let row_len = (count - row * columns).min(columns);
        Position {
            x: axis_position(center_x, column, row_len, spacing),
            y: axis_position(center_y, row, rows, spacing),
        }
    })
}

fn fit_axis(point: Fixed, size: Fixed, half_extent: i128) -> i128 {
    let size = i128::from(size.raw());
    let margin = half_extent.min(size / 2);
    i128::from(point.raw()).clamp(margin, size - margin)
}

fn axis_position(center: i128, index: usize, count: usize, spacing: i128) -> Fixed {
    let offset = (2 * index as i128 - (count - 1) as i128) * spacing / 2;
    let raw = (center + offset).clamp(i128::from(i64::MIN), i128::from(i64::MAX));
    Fixed::from_raw(raw as i64)
}

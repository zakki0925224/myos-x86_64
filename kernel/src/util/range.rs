use crate::error::Result;
use core::ops::RangeInclusive;

pub fn map_value_range_inclusive(
    from: RangeInclusive<i64>,
    to: RangeInclusive<i64>,
    value: i64,
) -> Result<i64> {
    if !from.contains(&value) {
        return Err("Value out of range".into());
    }

    let from_left = (value - *from.start()) as i128;
    let from_width = (from.end() - from.start()) as i128;
    let to_width = (to.end() - to.start()) as i128;

    if from_width == 0 {
        Ok(*to.start())
    } else {
        let to_left = from_left * to_width / from_width;
        to_left
            .try_into()
            .or(Err("Failed to convert to_left to the result type".into()))
            .map(|to_left: i64| to.start() + to_left)
    }
}

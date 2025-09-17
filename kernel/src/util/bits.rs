use core::cmp::min;

pub fn extract_bits<T>(value: T, shift: usize, width: usize) -> T
where
    T: TryFrom<u64> + From<u8>,
    u64: TryInto<T> + From<T>,
{
    let mask = (1u64 << min(63, width)) - 1;
    let value = u64::from(value);
    let value = value.checked_shr(shift as u32).unwrap_or(0) & mask;
    TryInto::try_into(value).unwrap_or_else(|_| T::from(0u8))
}

pub fn extract_bits_from_le_bytes(bytes: &[u8], shift: usize, width: usize) -> Option<u64> {
    if width == 0 {
        return None;
    }

    let byte_range = (shift / 8)..((shift + width + 7) / 8);
    let mut value = 0u64;
    let bit_shift = shift - byte_range.start * 8;
    bytes.get(byte_range).map(|bytes_in_range| {
        for (i, v) in bytes_in_range.iter().enumerate() {
            let v = *v as u128;
            value |= ((v << (i * 8)) >> bit_shift) as u64;
        }

        extract_bits(value, 0, width)
    })
}

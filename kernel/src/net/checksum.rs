use core::net::Ipv4Addr;

pub(super) fn checksum_words(data: &[u8]) -> u32 {
    data.chunks(2).fold(0u32, |acc, chunk| {
        let word = match chunk {
            [h, l] => u16::from_be_bytes([*h, *l]),
            [h] => u16::from_be_bytes([*h, 0]),
            _ => 0,
        };
        acc.wrapping_add(word as u32)
    })
}

pub(super) fn fold_checksum(mut sum: u32) -> u16 {
    while (sum >> 16) > 0 {
        sum = (sum & 0xffff) + (sum >> 16);
    }
    !(sum as u16)
}

pub(super) fn pseudo_header_sum(
    src_addr: Ipv4Addr,
    dst_addr: Ipv4Addr,
    protocol: u32,
    len: usize,
) -> u32 {
    let src = src_addr.octets();
    let dst = dst_addr.octets();
    let mut sum: u32 = 0;
    sum += ((src[0] as u32) << 8) | (src[1] as u32);
    sum += ((src[2] as u32) << 8) | (src[3] as u32);
    sum += ((dst[0] as u32) << 8) | (dst[1] as u32);
    sum += ((dst[2] as u32) << 8) | (dst[3] as u32);
    sum += protocol;
    sum += len as u32;
    sum
}

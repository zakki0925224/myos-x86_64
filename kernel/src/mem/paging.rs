#[derive(Debug)]
pub enum PageError {
    PageNotMapped,
    AddressNotAlignedByPageSize(u64),
    AddressNotMapped(u64),
}

impl core::fmt::Display for PageError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::PageNotMapped => write!(f, "Page not mapped"),
            Self::AddressNotAlignedByPageSize(addr) => {
                write!(f, "Address not aligned by page size: {:#x}", *addr)
            }
            Self::AddressNotMapped(addr) => {
                write!(f, "Address not mapped: {:#x}", *addr)
            }
        }
    }
}

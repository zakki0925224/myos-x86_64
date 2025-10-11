use crate::{
    arch::{IoPortAddress, VirtualAddress},
    error::{Error, Result},
    kinfo,
};
use alloc::vec::Vec;
use core::{ptr::read_unaligned, slice};

static mut ACPI: Acpi = Acpi::new();

const RSDP_SIGNATURE: [u8; 8] = *b"RSD PTR ";
const XSDT_SIGNATURE: [u8; 4] = *b"XSDT";
const FADT_SIGNATURE: [u8; 4] = *b"FACP";

const PM_TIMER_FREQ: u32 = 3579545;

#[derive(Debug)]
#[repr(C, packed)]
struct RootSystemDescriptorPointer {
    sign: [u8; 8],
    checksum: u8,
    oem_id: [u8; 6],
    rev: u8,
    rsdt_addr: u32,
    len: u32,
    xsdt_addr: u64,
    ext_checksum: u8,
    reserved: [u8; 3],
}

impl RootSystemDescriptorPointer {
    fn is_valid(&self) -> bool {
        self.sign == RSDP_SIGNATURE
    }

    fn is_valid_checksum(&self) -> bool {
        let size = size_of::<Self>();

        let mut sum: u8 = 0;
        for i in 0..size {
            let byte = unsafe { read_unaligned((self as *const _ as *const u8).add(i)) };
            sum = sum.wrapping_add(byte);
        }

        sum == 0
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
struct DescriptionHeader {
    sign: [u8; 4],
    len: u32,
    rev: u8,
    checksum: u8,
    oem_id: [u8; 6],
    oem_table_id: [u8; 8],
    oem_rev: u32,
    creator_id: u32,
    creator_rev: u32,
}

impl DescriptionHeader {
    fn is_valid(&self, sign: [u8; 4]) -> bool {
        self.sign == sign
    }

    fn is_valid_checksum(&self) -> bool {
        let bytes: &[u8] =
            unsafe { slice::from_raw_parts(self as *const _ as *const u8, self.len as usize) };

        bytes.iter().fold(0u8, |acc, &b| acc.wrapping_add(b)) == 0
    }

    fn entries_count(&self) -> usize {
        (self.len as usize - size_of::<Self>()) / size_of::<u64>()
    }
}

#[derive(Debug)]
#[repr(C, packed)]
struct FixedAcpiDescriptionTable {
    header: DescriptionHeader,
    reserved0: [u8; 40],
    pm_timer_block: u32,
    reserved1: [u8; 32],
    flags: u32,
    reserved2: [u8; 160],
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum AcpiError {
    InvalidSignatureError,
    InvalidRevisionError(u8),
    InvalidChecksumError,
    FixedAcpiDescriptionTableWasNotFound,
}

struct Acpi {
    rsdp_virt_addr: Option<VirtualAddress>,
}

impl Acpi {
    const fn new() -> Self {
        Self {
            rsdp_virt_addr: None,
        }
    }

    fn init(&mut self, rsdp_virt_addr: VirtualAddress) -> Result<()> {
        let rsdp = unsafe { &*(rsdp_virt_addr.as_ptr() as *const RootSystemDescriptorPointer) };
        let rev = rsdp.rev;

        if !rsdp.is_valid() {
            return Err(AcpiError::InvalidSignatureError.into());
        }

        if rev != 2 {
            return Err(AcpiError::InvalidRevisionError(rev).into());
        }

        if !rsdp.is_valid_checksum() {
            return Err(AcpiError::InvalidChecksumError.into());
        }

        self.rsdp_virt_addr = Some(rsdp_virt_addr);
        Ok(())
    }

    fn rsdp(&self) -> Result<&RootSystemDescriptorPointer> {
        self.rsdp_virt_addr
            .map(|addr| unsafe { &*(addr.as_ptr() as *const RootSystemDescriptorPointer) })
            .ok_or(Error::NotInitialized)
    }

    // XSDT header, entries
    fn xsdt(&self) -> Result<(&DescriptionHeader, Vec<u64>)> {
        let rsdp = self.rsdp()?;
        let xsdt_virt_addr: VirtualAddress = rsdp.xsdt_addr.into();
        let xsdt = unsafe { &*(xsdt_virt_addr.as_ptr() as *const DescriptionHeader) };

        if !xsdt.is_valid(XSDT_SIGNATURE) {
            return Err(AcpiError::InvalidSignatureError.into());
        }

        if !xsdt.is_valid_checksum() {
            return Err(AcpiError::InvalidChecksumError.into());
        }

        // 4 bytes align
        let u32_entries: &[u32] = unsafe {
            slice::from_raw_parts(
                xsdt_virt_addr
                    .offset(size_of::<DescriptionHeader>())
                    .as_ptr(),
                xsdt.entries_count() * 2,
            )
        };

        let entries = u32_entries
            .chunks(2)
            .map(|c| (c[1] as u64) << 32 | (c[0] as u64))
            .collect();

        Ok((xsdt, entries))
    }

    fn fadt(&self) -> Result<Option<&FixedAcpiDescriptionTable>> {
        let (_, xsdt_entries) = self.xsdt()?;
        let mut fadt = None;

        for entry_addr in xsdt_entries {
            let entry_addr: VirtualAddress = entry_addr.into();
            let entry = unsafe { &*(entry_addr.as_ptr() as *const FixedAcpiDescriptionTable) };
            if entry.header.is_valid(FADT_SIGNATURE) {
                fadt = Some(entry);
                break;
            }
        }

        Ok(fadt)
    }

    // addr, bit width == 32
    fn pm_timer_io_addr(&self) -> Result<(IoPortAddress, bool)> {
        let fadt = self
            .fadt()?
            .ok_or(AcpiError::FixedAcpiDescriptionTableWasNotFound)?;
        Ok((fadt.pm_timer_block.into(), ((fadt.flags >> 8) & 1) != 0))
    }

    fn pm_timer_wait_ms(&self, ms: u32) -> Result<()> {
        let (io_addr, is_bit_width_32) = self.pm_timer_io_addr()?;
        let start = io_addr.in32();
        let mut end = start + (PM_TIMER_FREQ * ms / 1000);

        if !is_bit_width_32 {
            end &= 0x00ff_ffff;
        }

        if end < start {
            while io_addr.in32() >= start {}
        }

        while io_addr.in32() < end {}
        Ok(())
    }
}

pub fn init(rsdp_virt_addr: VirtualAddress) -> Result<()> {
    unsafe { ACPI.init(rsdp_virt_addr) }?;
    kinfo!("acpi: Initialized");

    Ok(())
}

pub fn pm_timer_wait_ms(ms: u32) -> Result<()> {
    unsafe { ACPI.pm_timer_wait_ms(ms) }
}

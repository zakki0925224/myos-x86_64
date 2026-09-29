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

pub const PM_TIMER_FREQ: u32 = 3579545;

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

#[derive(Debug, Clone, Copy)]
enum ResetRegister {
    Io(IoPortAddress),
    Memory(VirtualAddress),
}

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
struct GenericAddressStructure {
    addr_space_id: u8,
    reg_bit_width: u8,
    reg_bit_offset: u8,
    access_size: u8,
    addr: u64,
}

impl GenericAddressStructure {
    fn reset_reg(&self) -> Result<ResetRegister> {
        let reg = match self.addr_space_id {
            0 => ResetRegister::Memory(self.addr.into()),
            1 => ResetRegister::Io(IoPortAddress::new(self.addr as u32)),
            _ => return Err(AcpiError::ResetRegisterNotSupported.into()),
        };

        Ok(reg)
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
    reset_reg: GenericAddressStructure,
    reset_value: u8,
    reserved2: [u8; 147],
}

#[derive(Debug)]
pub enum AcpiError {
    InvalidSignature,
    InvalidRevision(u8),
    InvalidChecksum,
    FixedAcpiDescriptionTableWasNotFound,
    PmTimerNotSupported,
    ResetRegisterNotSupported,
}

impl core::fmt::Display for AcpiError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "ACPI error: ")?;

        match self {
            Self::InvalidSignature => write!(f, "Invalid signature"),
            Self::InvalidRevision(r) => write!(f, "Invalid revision: {:#x}", r),
            Self::InvalidChecksum => write!(f, "Invalid checksum"),
            Self::FixedAcpiDescriptionTableWasNotFound => {
                write!(f, "Fixed ACPI Description Table was not found")
            }
            Self::PmTimerNotSupported => write!(f, "PM timer is not supported"),
            Self::ResetRegisterNotSupported => write!(f, "Reset register is not supported"),
        }
    }
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
        let rsdp: &RootSystemDescriptorPointer = unsafe { &*rsdp_virt_addr.as_ptr() };
        let rev = rsdp.rev;

        if !rsdp.is_valid() {
            return Err(AcpiError::InvalidSignature.into());
        }

        if rev != 2 {
            return Err(AcpiError::InvalidRevision(rev).into());
        }

        if !rsdp.is_valid_checksum() {
            return Err(AcpiError::InvalidChecksum.into());
        }

        self.rsdp_virt_addr = Some(rsdp_virt_addr);
        Ok(())
    }

    fn rsdp(&self) -> Result<&RootSystemDescriptorPointer> {
        self.rsdp_virt_addr
            .map(|addr| unsafe { &*addr.as_ptr::<RootSystemDescriptorPointer>() })
            .ok_or(Error::NotInitialized.into())
    }

    // XSDT header, entries
    fn xsdt(&self) -> Result<(&DescriptionHeader, Vec<u64>)> {
        let rsdp = self.rsdp()?;
        let xsdt_virt_addr: VirtualAddress = rsdp.xsdt_addr.into();
        let xsdt: &DescriptionHeader = unsafe { &*xsdt_virt_addr.as_ptr() };

        if !xsdt.is_valid(XSDT_SIGNATURE) {
            return Err(AcpiError::InvalidSignature.into());
        }

        if !xsdt.is_valid_checksum() {
            return Err(AcpiError::InvalidChecksum.into());
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
            let entry: &FixedAcpiDescriptionTable = unsafe { &*entry_addr.as_ptr() };
            if entry.header.is_valid(FADT_SIGNATURE) {
                fadt = Some(entry);
                break;
            }
        }

        Ok(fadt)
    }

    fn pm_timer(&self) -> Result<PmTimer> {
        let fadt = self
            .fadt()?
            .ok_or(AcpiError::FixedAcpiDescriptionTableWasNotFound)?;

        if fadt.pm_timer_block == 0 {
            return Err(AcpiError::PmTimerNotSupported.into());
        }

        let mask = if ((fadt.flags >> 8) & 1) != 0 {
            0xffff_ffff
        } else {
            0x00ff_ffff
        };

        Ok(PmTimer {
            io_addr: fadt.pm_timer_block.into(),
            mask,
        })
    }

    fn reset(&self) -> Result<()> {
        let fadt = self
            .fadt()?
            .ok_or(AcpiError::FixedAcpiDescriptionTableWasNotFound)?;

        if fadt.header.len < 129 || (fadt.flags >> 10) & 1 == 0 {
            return Err(AcpiError::ResetRegisterNotSupported.into());
        }

        let gas = fadt.reset_reg;
        let reset_reg = gas.reset_reg()?;
        let reset_value = fadt.reset_value;

        match reset_reg {
            ResetRegister::Io(port) => port.out8(reset_value),
            ResetRegister::Memory(addr) => unsafe {
                addr.as_ptr_mut::<u8>().write_volatile(reset_value);
            },
        }

        Ok(())
    }
}

#[derive(Debug)]
pub struct PmTimer {
    io_addr: IoPortAddress,
    mask: u32,
}

impl PmTimer {
    pub fn read(&self) -> u32 {
        self.io_addr.in32() & self.mask
    }

    pub fn elapsed(&self, from: u32, to: u32) -> u32 {
        to.wrapping_sub(from) & self.mask
    }

    pub fn wait_ms(&self, ms: u32) {
        let start = self.read();
        let end = ms * (PM_TIMER_FREQ / 1000);
        while self.elapsed(start, self.read()) < end {
            core::hint::spin_loop();
        }
    }
}

pub fn init(rsdp_virt_addr: VirtualAddress) -> Result<()> {
    let acpi = &raw mut ACPI;
    unsafe { (*acpi).init(rsdp_virt_addr) }?;
    kinfo!("acpi: Initialized");

    Ok(())
}

pub fn pm_timer() -> Result<PmTimer> {
    let acpi = &raw const ACPI;
    unsafe { (*acpi).pm_timer() }
}

pub fn reset() -> Result<()> {
    let acpi = &raw const ACPI;
    unsafe { (*acpi).reset() }
}

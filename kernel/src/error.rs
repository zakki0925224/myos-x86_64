use crate::{
    arch::x86_64::acpi::AcpiError,
    device::usb::xhc::XhcDriverError,
    fs::vfs::VirtualFileSystemError,
    graphics::{draw::DrawError, multi_layer::LayerError, window_manager::WindowManagerError},
    mem::{allocator::AllocationError, bitmap::BitmapMemoryManagerError, paging::PageManagerError},
    util::fifo::FifoError,
};
use common::elf::Elf64Error;

#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    NotInitialized,
    Failed(&'static str),
    LayerError(LayerError),
    BitmapMemoryManagerError(BitmapMemoryManagerError),
    PageManagerError(PageManagerError),
    FifoError(FifoError),
    IndexOutOfBoundsError(usize),
    VirtualFileSystemError(VirtualFileSystemError),
    Elf64Error(Elf64Error),
    WindowManagerError(WindowManagerError),
    AcpiError(AcpiError),
    AllocationError(AllocationError),
    DrawError(DrawError),
    XhcDriverError(XhcDriverError),
}

impl From<&'static str> for Error {
    fn from(s: &'static str) -> Self {
        Self::Failed(s)
    }
}

macro_rules! impl_from_error {
    ($($variant:ident($error_type:ty)),* $(,)?) => {
        $(
            impl From<$error_type> for Error {
                fn from(err: $error_type) -> Self {
                    Self::$variant(err)
                }
            }
        )*
    };
}

impl_from_error! {
    LayerError(LayerError),
    BitmapMemoryManagerError(BitmapMemoryManagerError),
    PageManagerError(PageManagerError),
    FifoError(FifoError),
    VirtualFileSystemError(VirtualFileSystemError),
    Elf64Error(Elf64Error),
    WindowManagerError(WindowManagerError),
    AcpiError(AcpiError),
    AllocationError(AllocationError),
    DrawError(DrawError),
    XhcDriverError(XhcDriverError),
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            _ => write!(f, "{:?}", self),
        }
    }
}

impl core::error::Error for Error {}

pub type Result<T> = core::result::Result<T, Error>;

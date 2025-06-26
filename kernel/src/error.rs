use crate::{
    acpi::AcpiError,
    device::{tty::TtyError, xhc::XhcDriverError},
    fs::vfs::VirtualFileSystemError,
    graphics::{
        draw::DrawError, multi_layer::LayerError, simple_window_manager::SimpleWindowManagerError,
    },
    mem::{allocator::AllocationError, bitmap::BitmapMemoryManagerError, paging::PageManagerError},
    util::{fifo::FifoError, lifo::LifoError},
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
    LifoError(LifoError),
    IndexOutOfBoundsError(usize),
    VirtualFileSystemError(VirtualFileSystemError),
    Elf64Error(Elf64Error),
    SimpleWindowManagerError(SimpleWindowManagerError),
    AcpiError(AcpiError),
    AllocationError(AllocationError),
    DrawError(DrawError),
    TtyError(TtyError),
    XhcDriverError(XhcDriverError),
}

impl From<&'static str> for Error {
    fn from(s: &'static str) -> Self {
        Self::Failed(s)
    }
}

impl From<LayerError> for Error {
    fn from(err: LayerError) -> Self {
        Self::LayerError(err)
    }
}

impl From<BitmapMemoryManagerError> for Error {
    fn from(err: BitmapMemoryManagerError) -> Self {
        Self::BitmapMemoryManagerError(err)
    }
}

impl From<PageManagerError> for Error {
    fn from(err: PageManagerError) -> Self {
        Self::PageManagerError(err)
    }
}

impl From<FifoError> for Error {
    fn from(err: FifoError) -> Self {
        Self::FifoError(err)
    }
}

impl From<LifoError> for Error {
    fn from(err: LifoError) -> Self {
        Self::LifoError(err)
    }
}

impl From<VirtualFileSystemError> for Error {
    fn from(err: VirtualFileSystemError) -> Self {
        Self::VirtualFileSystemError(err)
    }
}

impl From<Elf64Error> for Error {
    fn from(err: Elf64Error) -> Self {
        Self::Elf64Error(err)
    }
}

impl From<SimpleWindowManagerError> for Error {
    fn from(err: SimpleWindowManagerError) -> Self {
        Self::SimpleWindowManagerError(err)
    }
}

impl From<AcpiError> for Error {
    fn from(err: AcpiError) -> Self {
        Self::AcpiError(err)
    }
}

impl From<AllocationError> for Error {
    fn from(err: AllocationError) -> Self {
        Self::AllocationError(err)
    }
}

impl From<DrawError> for Error {
    fn from(err: DrawError) -> Self {
        Self::DrawError(err)
    }
}

impl From<TtyError> for Error {
    fn from(err: TtyError) -> Self {
        Self::TtyError(err)
    }
}

impl From<XhcDriverError> for Error {
    fn from(err: XhcDriverError) -> Self {
        Self::XhcDriverError(err)
    }
}

pub type Result<T> = core::result::Result<T, Error>;

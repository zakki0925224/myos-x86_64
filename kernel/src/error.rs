use crate::{
    arch::x86_64::acpi::AcpiError,
    device::{pci_bus::PciError, usb::xhc::XhcDriverError},
    fs::vfs::VirtualFileSystemError,
    graphics::{draw::DrawError, multi_layer::LayerError, window_manager::WindowManagerError},
    mem::{allocator::AllocationError, bitmap::BitmapMemoryManagerError, paging::PageError},
};
use common::elf::Elf64Error;

macro_rules! impl_from_error {
    ($($variant:ident($error_type:ty)),* $(,)?) => {
        $(
            impl From<$error_type> for Error {
                fn from(err: $error_type) -> Self {
                    Self::$variant(err)
                }
            }

            impl From<$error_type> for Error_ {
                fn from(err: $error_type) -> Self {
                    Error::from(err).into()
                }
            }
        )*
    };
}

#[derive(Debug)]
pub enum Error {
    NotInitialized,
    Locked,
    IndexOutOfBounds {
        index: usize,
        len: Option<usize>,
    },
    OutOfRange {
        value: usize,
        min: usize,
        max: usize,
    },
    NotAligned {
        value: usize,
        align: usize,
    },
    InvalidBufferSize {
        required: usize,
        actual: usize,
    },
    BufferFull,
    BufferEmpty,
    AlreadyExists,
    Overflow,
    NotFound,
    InvalidData,
    NotSupported,
    Elf64Error(Elf64Error),
    AcpiError(AcpiError),
    VirtualFileSystemError(VirtualFileSystemError),
    PciError(PciError),
    XhcDriverError(XhcDriverError),
    DrawError(DrawError),
    LayerError(LayerError),
    WindowManagerError(WindowManagerError),
    AllocationError(AllocationError),
    BitmapMemoryManagerError(BitmapMemoryManagerError),
    PageError(PageError),
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NotInitialized => write!(f, "Not initialized"),
            Self::Locked => write!(f, "Locked"),
            Self::IndexOutOfBounds { index, len } => {
                write!(f, "Index out of bounds: the index is {}", index)?;

                if let Some(len) = len {
                    write!(f, " but the len is {}", len)?;
                }

                Ok(())
            }
            Self::OutOfRange { value, min, max } => {
                write!(
                    f,
                    "Out of range: {} is out of range from {} to {}",
                    value, min, max
                )
            }
            Self::NotAligned { value, align } => {
                write!(f, "Not aligned: {} is not aligned to {}", value, align)
            }
            Self::InvalidBufferSize { required, actual } => {
                write!(
                    f,
                    "Invalid buffer size: required {} but actual {}",
                    required, actual
                )
            }
            Self::BufferFull => write!(f, "Buffer is full"),
            Self::BufferEmpty => write!(f, "Buffer is empty"),
            Self::AlreadyExists => write!(f, "Already exists"),
            Self::Overflow => write!(f, "Overflow"),
            Self::NotFound => write!(f, "Not found"),
            Self::InvalidData => write!(f, "Invalid data"),
            Self::NotSupported => write!(f, "Not supported"),
            Self::Elf64Error(err) => write!(f, "{}", err),
            Self::AcpiError(err) => write!(f, "{}", err),
            Self::VirtualFileSystemError(err) => write!(f, "{}", err),
            Self::PciError(err) => write!(f, "{}", err),
            Self::XhcDriverError(err) => write!(f, "{}", err),
            Self::DrawError(err) => write!(f, "{}", err),
            Self::LayerError(err) => write!(f, "{}", err),
            Self::WindowManagerError(err) => write!(f, "{}", err),
            Self::AllocationError(err) => write!(f, "{}", err),
            Self::BitmapMemoryManagerError(err) => write!(f, "{}", err),
            Self::PageError(err) => write!(f, "{}", err),
        }
    }
}

impl_from_error! {
    Elf64Error(Elf64Error),
    AcpiError(AcpiError),
    VirtualFileSystemError(VirtualFileSystemError),
    PciError(PciError),
    XhcDriverError(XhcDriverError),
    DrawError(DrawError),
    LayerError(LayerError),
    WindowManagerError(WindowManagerError),
    AllocationError(AllocationError),
    BitmapMemoryManagerError(BitmapMemoryManagerError),
    PageError(PageError),
}

impl Error {
    pub fn with_context(self, context: &'static str) -> Error_ {
        let err: Error_ = self.into();
        err.with_context(context)
    }
}

#[derive(Debug)]
pub struct Error_ {
    kind: Error,
    context: Option<&'static str>,
}

impl core::error::Error for Error_ {}

impl core::fmt::Display for Error_ {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.kind)?;

        if let Some(c) = self.context {
            write!(f, " ({})", c)?;
        }

        Ok(())
    }
}

impl Error_ {
    pub fn should_retry(&self) -> bool {
        matches!(self.kind, Error::Locked)
    }

    pub fn with_context(mut self, context: &'static str) -> Self {
        self.context = Some(context);
        self
    }
}

impl From<Error> for Error_ {
    fn from(kind: Error) -> Self {
        Self {
            kind,
            context: None,
        }
    }
}

pub type Result<T> = core::result::Result<T, Error_>;

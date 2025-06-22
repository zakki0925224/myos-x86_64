use super::paging::{self, EntryMode, MappingInfo, PageWriteThroughLevel, ReadWrite, PAGE_SIZE};
use crate::{
    arch::addr::*,
    error::{Error, Result},
    util::mutex::Mutex,
};
use common::mem_desc::{MemoryDescriptor, UEFI_PAGE_SIZE};

static mut BMM: Mutex<BitmapMemoryManager> = Mutex::new(BitmapMemoryManager::new());

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MemoryFrameInfo {
    pub frame_start_phys_addr: PhysicalAddress,
    pub frame_size: usize, // must be 4096B align
    pub frame_index: usize,
    pub is_allocated: bool,
}

impl MemoryFrameInfo {
    pub fn set_permissions_to_supervisor(&self) -> Result<()> {
        self.set_permissions(
            ReadWrite::Write,
            EntryMode::Supervisor,
            PageWriteThroughLevel::WriteThrough,
            false,
        )
    }

    pub fn set_permissions_to_user(&self) -> Result<()> {
        self.set_permissions(
            ReadWrite::Write,
            EntryMode::User,
            PageWriteThroughLevel::WriteThrough,
            false,
        )
    }

    pub fn frame_start_virt_addr(&self) -> Result<VirtualAddress> {
        self.frame_start_phys_addr.get_virt_addr()
    }

    pub fn set_permissions(
        &self,
        rw: ReadWrite,
        us: EntryMode,
        pwt: PageWriteThroughLevel,
        pcd: bool,
    ) -> Result<()> {
        let page_len = self.frame_size / PAGE_SIZE;
        let mut start = self.frame_start_virt_addr()?;

        for _ in 0..page_len {
            paging::update_mapping(&MappingInfo {
                start,
                end: start.offset(PAGE_SIZE),
                phys_addr: start.get_phys_addr()?,
                rw,
                us,
                pwt,
                pcd,
            })?;
            start = start.offset(PAGE_SIZE);
        }

        Ok(())
    }
}

#[derive(Debug)]
struct Bitmap(u8);

impl Bitmap {
    const BITMAP_SIZE: usize = u8::BITS as usize;

    fn get(&self, index: usize) -> Result<bool> {
        if index >= Self::BITMAP_SIZE {
            return Err(Error::IndexOutOfBoundsError(index));
        }

        Ok(((self.0 >> index) & 0x1) != 0)
    }

    fn set(&mut self, index: usize, value: bool) -> Result<()> {
        if index >= Self::BITMAP_SIZE {
            return Err(Error::IndexOutOfBoundsError(index));
        }

        self.0 = (self.0 & !(0x1 << index)) | ((value as u8) << index);
        assert_eq!(self.get(index)?, value);

        Ok(())
    }

    fn fill(&mut self, value: bool) {
        self.0 = if value { 0xff } else { 0 };
    }

    fn is_allocated_all(&self) -> bool {
        self.0 == 0xff
    }

    fn is_free_all(&self) -> bool {
        self.0 == 0
    }
}

impl From<u8> for Bitmap {
    fn from(value: u8) -> Self {
        Self(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BitmapMemoryManagerError {
    FreeMemoryFrameWasNotFoundError,
    MemoryFrameWasAlreadyAllocatedError(usize), // memory frame index,
    MemoryFrameWasAlreadyDeallocatedError(usize), // memory frame index,
    InvalidMemoryFrameLengthError(usize),       // memory frame length
}

#[derive(Debug)]
struct BitmapMemoryManager {
    bitmap_phys_addr: Option<PhysicalAddress>,
    total_frame_len: usize,
    allocated_frame_len: usize,
    free_frame_len: usize,
    allocated_frame_len_in_available_mem: usize,
    total_available_mem_size: usize,
}

impl BitmapMemoryManager {
    const fn new() -> Self {
        Self {
            bitmap_phys_addr: None,
            total_frame_len: 0,
            allocated_frame_len: 0,
            free_frame_len: 0,
            allocated_frame_len_in_available_mem: 0,
            total_available_mem_size: 0,
        }
    }

    fn bitmap_phys_addr(&self) -> Result<PhysicalAddress> {
        self.bitmap_phys_addr.ok_or(Error::NotInitialized)
    }

    fn init(&mut self, mem_map: &[MemoryDescriptor]) -> Result<()> {
        assert_eq!(UEFI_PAGE_SIZE, PAGE_SIZE);

        let max_phys_addr = mem_map
            .iter()
            .map(|d| d.phys_start + d.page_cnt * UEFI_PAGE_SIZE as u64)
            .max()
            .unwrap();
        let total_frame_len = ((max_phys_addr as usize + UEFI_PAGE_SIZE) / UEFI_PAGE_SIZE).max(1);

        // find available memory area for bitmap
        let mut bitmap_phys_addr = PhysicalAddress::default();
        for d in mem_map {
            if !d.ty.is_available_memory()
                || (d.page_cnt as usize) * UEFI_PAGE_SIZE < (total_frame_len / Bitmap::BITMAP_SIZE)
                || d.phys_start == 0
                || d.phys_start % UEFI_PAGE_SIZE as u64 != 0
            {
                continue;
            }

            bitmap_phys_addr.set(d.phys_start);
            break;
        }

        if bitmap_phys_addr.get() == 0 {
            panic!("mem: Failed to allocate memory for bitmap");
        }

        // calc total available memory size
        let mut total_available_mem_size = 0;
        for d in mem_map {
            if !d.ty.is_available_memory() {
                continue;
            }

            if d.phys_start == 0 {
                continue;
            }

            if d.phys_start % (PAGE_SIZE as u64) != 0 {
                continue;
            }

            total_available_mem_size += d.page_cnt as usize * UEFI_PAGE_SIZE;
        }

        self.bitmap_phys_addr = Some(bitmap_phys_addr);
        self.total_frame_len = total_frame_len;
        self.allocated_frame_len = total_frame_len;
        self.free_frame_len = 0;
        self.allocated_frame_len_in_available_mem = total_available_mem_size / UEFI_PAGE_SIZE;
        self.total_available_mem_size = total_available_mem_size;
        Ok(())
    }

    fn init2(&mut self, mem_map: &[MemoryDescriptor]) -> Result<()> {
        // fill all bitmap
        for i in 0..self.bitmap_len() {
            self.bitmap(i)?.fill(true);
        }

        // deallocate available memory frame
        for d in mem_map {
            if !d.ty.is_available_memory() {
                continue;
            }

            if d.phys_start == 0 {
                continue;
            }

            if d.phys_start % (PAGE_SIZE as u64) != 0 {
                continue;
            }

            for i in 0..d.page_cnt {
                let frame_index = (d.phys_start + (i * PAGE_SIZE as u64)) as usize / PAGE_SIZE;

                self.dealloc_frame(frame_index)?;
            }
        }

        // allocate bitmap memory frame
        let bitmap_phys_addr = self.bitmap_phys_addr()?;
        let start = bitmap_phys_addr.get() as usize / PAGE_SIZE;
        let end = bitmap_phys_addr.offset(self.bitmap_len()).get() as usize / PAGE_SIZE;
        for i in start..=end {
            // ignore already allocated error
            let _ = self.alloc_frame(i);
        }

        Ok(())
    }

    fn bitmap_len(&self) -> usize {
        self.total_frame_len / Bitmap::BITMAP_SIZE
    }

    fn get_mem_frame(&self, frame_index: usize) -> Option<MemoryFrameInfo> {
        if let Ok(bitmap) = self.bitmap(self.bitmap_offset(frame_index)) {
            return Some(MemoryFrameInfo {
                frame_start_phys_addr: ((frame_index * PAGE_SIZE) as u64).into(),
                frame_size: PAGE_SIZE,
                frame_index,
                is_allocated: bitmap.get(self.bitmap_pos(frame_index)).unwrap(),
            });
        }

        None
    }

    fn alloc_single_mem_frame(&mut self) -> Result<MemoryFrameInfo> {
        if self.free_frame_len == 0 {
            return Err(BitmapMemoryManagerError::FreeMemoryFrameWasNotFoundError.into());
        }

        let mut found_mem_frame_index = 0;
        'outer: for i in 0..self.bitmap_len() {
            let bitmap = self.bitmap(i)?;
            if bitmap.is_allocated_all() {
                continue;
            }

            for j in 0..Bitmap::BITMAP_SIZE {
                if !bitmap.get(j)? {
                    found_mem_frame_index = i * Bitmap::BITMAP_SIZE + j;

                    if found_mem_frame_index != 0 {
                        break 'outer;
                    }
                }
            }
        }

        assert_ne!(found_mem_frame_index, 0);
        self.alloc_frame(found_mem_frame_index)?;
        let mem_frame_info = MemoryFrameInfo {
            frame_start_phys_addr: ((found_mem_frame_index * PAGE_SIZE) as u64).into(),
            frame_size: PAGE_SIZE,
            frame_index: found_mem_frame_index,
            is_allocated: true,
        };

        Ok(mem_frame_info)
    }

    fn alloc_multi_mem_frame(&mut self, len: usize) -> Result<MemoryFrameInfo> {
        if len == 0 {
            return Err(BitmapMemoryManagerError::InvalidMemoryFrameLengthError(len).into());
        }

        if len == 1 {
            return self.alloc_single_mem_frame();
        }

        if len > self.free_frame_len {
            return Err(BitmapMemoryManagerError::FreeMemoryFrameWasNotFoundError.into());
        }

        let mut start_mem_frame_index = None;
        let mut end_mem_frame_index = None;

        'outer: for i in 0..self.bitmap_len() {
            let bitmap = self.bitmap(i)?;

            if len == Bitmap::BITMAP_SIZE && bitmap.is_free_all() {
                start_mem_frame_index = Some(i * Bitmap::BITMAP_SIZE);
                end_mem_frame_index = Some(i * Bitmap::BITMAP_SIZE + 7);
                break 'outer;
            }

            for j in 0..Bitmap::BITMAP_SIZE {
                // found all free area
                if let (Some(s_i), Some(e_i)) = (start_mem_frame_index, end_mem_frame_index) {
                    if e_i == s_i + len {
                        break 'outer;
                    }
                }

                if !bitmap.get(j)? {
                    if let Some(s_i) = start_mem_frame_index {
                        if let Some(e_i) = end_mem_frame_index.as_mut() {
                            *e_i += 1;
                        } else {
                            end_mem_frame_index = Some(s_i + 1);
                        }
                    } else {
                        start_mem_frame_index = Some(i * Bitmap::BITMAP_SIZE + j);
                    }
                } else {
                    start_mem_frame_index = None;
                    end_mem_frame_index = None;
                }
            }
        }

        let start_mem_frame_index = start_mem_frame_index
            .ok_or(BitmapMemoryManagerError::FreeMemoryFrameWasNotFoundError)?;
        let end_mem_frame_index =
            end_mem_frame_index.ok_or(BitmapMemoryManagerError::FreeMemoryFrameWasNotFoundError)?;

        for i in start_mem_frame_index..=end_mem_frame_index {
            self.alloc_frame(i)?;
        }

        let mem_frame_info = MemoryFrameInfo {
            frame_start_phys_addr: ((start_mem_frame_index * PAGE_SIZE) as u64).into(),
            frame_size: PAGE_SIZE * len,
            frame_index: start_mem_frame_index,
            is_allocated: true,
        };

        Ok(mem_frame_info)
    }

    unsafe fn mem_clear(&self, mem_frame_info: &MemoryFrameInfo) -> Result<()> {
        let frame_size = mem_frame_info.frame_size;
        let start_virt_addr = mem_frame_info.frame_start_virt_addr()?;
        let ptr: *mut u8 = start_virt_addr.as_ptr_mut();
        ptr.write_bytes(0, frame_size);

        Ok(())
    }

    fn dealloc_mem_frame(&mut self, mem_frame_info: MemoryFrameInfo) -> Result<()> {
        let frame_size = mem_frame_info.frame_size;
        let frame_index = mem_frame_info.frame_index;

        for i in frame_index..frame_index + (frame_size / PAGE_SIZE) {
            self.dealloc_frame(i)?;
        }

        Ok(())
    }

    fn bitmap(&self, offset: usize) -> Result<&mut Bitmap> {
        if offset >= self.bitmap_len() {
            return Err(Error::IndexOutOfBoundsError(offset));
        }

        Ok(unsafe { &mut *(self.bitmap_phys_addr()?.offset(offset).get() as *mut Bitmap) })
    }

    fn alloc_frame(&mut self, frame_index: usize) -> Result<()> {
        let bitmap_pos = self.bitmap_pos(frame_index);
        let bitmap = self.bitmap(self.bitmap_offset(frame_index))?;

        // already allocated
        if bitmap.get(bitmap_pos)? {
            return Err(
                BitmapMemoryManagerError::MemoryFrameWasAlreadyAllocatedError(frame_index).into(),
            );
        }

        bitmap.set(bitmap_pos, true)?;

        self.allocated_frame_len += 1;
        self.free_frame_len -= 1;
        self.allocated_frame_len_in_available_mem += 1;
        assert_eq!(
            self.total_frame_len,
            self.allocated_frame_len + self.free_frame_len
        );

        Ok(())
    }

    fn dealloc_frame(&mut self, frame_index: usize) -> Result<()> {
        let bitmap_pos = self.bitmap_pos(frame_index);
        let bitmap = self.bitmap(self.bitmap_offset(frame_index))?;

        // already deallocated
        if !bitmap.get(bitmap_pos)? {
            return Err(
                BitmapMemoryManagerError::MemoryFrameWasAlreadyDeallocatedError(frame_index).into(),
            );
        }

        bitmap.set(bitmap_pos, false)?;

        self.allocated_frame_len -= 1;
        self.free_frame_len += 1;
        self.allocated_frame_len_in_available_mem -= 1;
        assert_eq!(
            self.total_frame_len,
            self.allocated_frame_len + self.free_frame_len
        );

        Ok(())
    }

    fn bitmap_offset(&self, frame_index: usize) -> usize {
        frame_index / Bitmap::BITMAP_SIZE
    }

    fn bitmap_pos(&self, frame_index: usize) -> usize {
        frame_index % Bitmap::BITMAP_SIZE // 0 ~ 7
    }
}

pub fn init(mem_map: &[MemoryDescriptor]) -> Result<()> {
    let mut bmm = unsafe { BMM.try_lock() }?;
    bmm.init(mem_map)?;
    bmm.init2(mem_map)
}

pub fn get_total_mem_size() -> Result<usize> {
    let bmm = unsafe { BMM.try_lock() }?;
    let total = bmm.total_frame_len * PAGE_SIZE;
    Ok(total)
}

pub fn get_mem_size() -> Result<(usize, usize)> {
    let bmm = unsafe { BMM.try_lock() }?;
    let used = bmm.allocated_frame_len_in_available_mem * PAGE_SIZE;
    let total = bmm.total_available_mem_size;
    Ok((used, total))
}

pub fn alloc_mem_frame(len: usize) -> Result<MemoryFrameInfo> {
    let mut bmm = unsafe { BMM.try_lock() }?;
    bmm.alloc_multi_mem_frame(len)
}

pub fn dealloc_mem_frame(mem_frame_info: MemoryFrameInfo) -> Result<()> {
    let mut bmm = unsafe { BMM.try_lock() }?;
    bmm.dealloc_mem_frame(mem_frame_info)
}

pub fn mem_clear(mem_frame_info: &MemoryFrameInfo) -> Result<()> {
    let bmm = unsafe { BMM.try_lock() }?;
    unsafe { bmm.mem_clear(mem_frame_info) }
}

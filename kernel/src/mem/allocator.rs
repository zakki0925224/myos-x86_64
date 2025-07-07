use super::{bitmap, paging::PAGE_SIZE};
use crate::{
    error::{Error, Result},
    sync::mutex::Mutex,
};
use core::{
    alloc::{GlobalAlloc, Layout},
    ptr::{self, NonNull},
};

const HEAP_SIZE: usize = 1024 * 1024 * 128; // 128MiB

#[global_allocator]
static mut ALLOCATOR: LinkedListAllocator = LinkedListAllocator::empty();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllocationError {
    LayoutError(Layout),
}

// reference: https://github.com/rust-osdev/linked-list-allocator
#[derive(Debug, Clone, Copy)]
struct HoleInfo {
    addr: *mut u8,
    size: usize,
}

#[derive(Debug)]
struct Hole {
    size: usize,
    next: Option<NonNull<Hole>>,
}

#[derive(Debug)]
struct HoleList {
    first: Hole,
    bottom: *mut u8,
    top: *mut u8,
}

impl HoleList {
    const fn empty() -> Self {
        Self {
            first: Hole {
                size: 0,
                next: None,
            },
            bottom: ptr::null_mut(),
            top: ptr::null_mut(),
        }
    }

    unsafe fn new(hole_addr: *mut u8, hole_size: usize) -> Self {
        assert_eq!(size_of::<Hole>(), Self::min_size());
        assert!(hole_size >= size_of::<Hole>());

        let aligned_hole_addr = align_up(hole_addr, align_of::<Hole>());
        let requested_hole_size = hole_size - ((aligned_hole_addr as usize) - (hole_addr as usize));
        let aligned_hole_size = align_down_size(requested_hole_size, align_of::<Hole>());
        assert!(aligned_hole_size >= size_of::<Hole>());

        let ptr = aligned_hole_addr as *mut Hole;
        ptr.write(Hole {
            size: aligned_hole_size,
            next: None,
        });

        assert_eq!(
            hole_addr.wrapping_add(hole_size),
            aligned_hole_addr.wrapping_add(requested_hole_size)
        );

        Self {
            first: Hole {
                size: 0,
                next: Some(NonNull::new_unchecked(ptr)),
            },
            bottom: aligned_hole_addr,
            top: aligned_hole_addr.wrapping_add(aligned_hole_size),
        }
    }

    fn cursor(&mut self) -> Option<Cursor> {
        if let Some(hole) = self.first.next {
            Some(Cursor {
                hole,
                prev: NonNull::new(&mut self.first)?,
                top: self.top,
            })
        } else {
            None
        }
    }

    fn alloc_first_fit(&mut self, layout: Layout) -> Result<(NonNull<u8>, Layout)> {
        let aligned_layout = Self::align_layout(layout)?;
        let mut cursor = self.cursor().ok_or(Error::Failed("No cursor"))?;

        loop {
            match cursor.split_current(aligned_layout) {
                Ok((ptr, _len)) => {
                    return Ok((
                        NonNull::new(ptr).ok_or(Error::Failed("Pointer is null"))?,
                        aligned_layout,
                    ));
                }
                Err(curs) => {
                    cursor = curs.next().ok_or(Error::Failed("No next cursor"))?;
                }
            }
        }
    }

    fn dealloc(&mut self, ptr: NonNull<u8>, layout: Layout) -> Result<Layout> {
        let aligned_layout = Self::align_layout(layout)?;
        dealloc(self, ptr.as_ptr(), aligned_layout.size())?;
        Ok(aligned_layout)
    }

    fn min_size() -> usize {
        size_of::<usize>() * 2
    }

    fn align_layout(layout: Layout) -> Result<Layout> {
        let mut size = layout.size();
        if size < Self::min_size() {
            size = Self::min_size();
        }
        let size = align_up_size(size, align_of::<Hole>());
        let new_layout = Layout::from_size_align(size, layout.align())
            .map_err(|_| AllocationError::LayoutError(layout))?;
        Ok(new_layout)
    }
}

#[derive(Debug)]
struct Cursor {
    prev: NonNull<Hole>,
    hole: NonNull<Hole>,
    top: *mut u8,
}

impl Cursor {
    fn next(mut self) -> Option<Self> {
        unsafe {
            self.hole.as_mut().next.map(|hole| Self {
                prev: self.hole,
                hole,
                top: self.top,
            })
        }
    }

    fn current(&self) -> &Hole {
        unsafe { self.hole.as_ref() }
    }

    fn prev(&self) -> &Hole {
        unsafe { self.prev.as_ref() }
    }

    fn split_current(
        self,
        required_layout: Layout,
    ) -> core::result::Result<(*mut u8, usize), Self> {
        let front_padding;
        let alloc_ptr;
        let alloc_size;
        let back_padding;

        {
            let hole_size = self.current().size;
            let hole_addr_u8: *mut u8 = self.hole.as_ptr().cast();
            let required_size = required_layout.size();
            let required_align = required_layout.align();

            if hole_size < required_size {
                return Err(self);
            }

            let aligned_addr = if hole_addr_u8 == align_up(hole_addr_u8, required_align) {
                front_padding = None;
                hole_addr_u8
            } else {
                let new_start = hole_addr_u8.wrapping_add(HoleList::min_size());
                let aligned_addr = align_up(new_start, required_align);
                front_padding = Some(HoleInfo {
                    addr: hole_addr_u8,
                    size: (aligned_addr as usize) - (hole_addr_u8 as usize),
                });
                aligned_addr
            };

            let alloc_end = aligned_addr.wrapping_add(required_size);
            let hole_end = hole_addr_u8.wrapping_add(hole_size);

            if alloc_end > hole_end {
                return Err(self);
            }

            alloc_ptr = aligned_addr;
            alloc_size = required_size;

            let back_padding_size = hole_end as usize - alloc_end as usize;
            back_padding = if back_padding_size == 0 {
                None
            } else {
                let hole_layout = Layout::new::<Hole>();
                let back_padding_start = align_up(alloc_end, hole_layout.align());
                let back_padding_end = back_padding_start.wrapping_add(hole_layout.size());

                if back_padding_end <= hole_end {
                    Some(HoleInfo {
                        addr: back_padding_start,
                        size: back_padding_size,
                    })
                } else {
                    return Err(self);
                }
            }
        }

        let Self {
            mut prev, mut hole, ..
        } = self;

        unsafe {
            prev.as_mut().next = None;
        }
        let maybe_next_addr: Option<NonNull<Hole>> = unsafe { hole.as_mut().next.take() };

        match (front_padding, back_padding) {
            (None, None) => unsafe {
                prev.as_mut().next = maybe_next_addr;
            },
            (None, Some(singlepad)) | (Some(singlepad), None) => unsafe {
                let singlepad_ptr: *mut Hole = singlepad.addr.cast();
                singlepad_ptr.write(Hole {
                    size: singlepad.size,
                    next: maybe_next_addr,
                });

                prev.as_mut().next = Some(NonNull::new_unchecked(singlepad_ptr));
            },
            (Some(frontpad), Some(backpad)) => unsafe {
                let backpad_ptr: *mut Hole = backpad.addr.cast();
                backpad_ptr.write(Hole {
                    size: backpad.size,
                    next: maybe_next_addr,
                });

                let frontpad_ptr: *mut Hole = frontpad.addr.cast();
                frontpad_ptr.write(Hole {
                    size: frontpad.size,
                    next: Some(NonNull::new_unchecked(backpad_ptr)),
                });

                prev.as_mut().next = Some(NonNull::new_unchecked(frontpad_ptr));
            },
        }

        Ok((alloc_ptr, alloc_size))
    }

    fn try_insert_back(
        self,
        node: NonNull<Hole>,
        bottom: *mut u8,
    ) -> core::result::Result<Self, Self> {
        if node < self.hole {
            let node_u8: *mut u8 = node.as_ptr().cast();
            let node_size = unsafe { node.as_ref().size };
            let hole_u8: *mut u8 = self.hole.as_ptr().cast();

            assert!(node_u8.wrapping_add(node_size) <= hole_u8);
            assert_eq!(self.prev().size, 0);

            let Self {
                mut prev,
                hole,
                top,
            } = self;
            unsafe {
                let mut node = check_merge_bottom(node, bottom);
                prev.as_mut().next = Some(node);
                node.as_mut().next = Some(hole);
            }

            Ok(Self {
                prev,
                hole: node,
                top,
            })
        } else {
            Err(self)
        }
    }

    fn try_insert_after(&mut self, mut node: NonNull<Hole>) -> Result<()> {
        let node_u8: *mut u8 = node.as_ptr().cast();
        let node_size = unsafe { node.as_ref().size };

        if let Some(next) = self.current().next.as_ref() {
            if node < *next {
                let node_u8 = node_u8 as *const u8;
                assert!(node_u8.wrapping_add(node_size) <= next.as_ptr().cast::<u8>());
            } else {
                return Err(Error::Failed("Invalid node"));
            }
        }

        assert!(self.hole < node);

        let hole_u8: *mut u8 = self.hole.as_ptr().cast();
        let hole_size = self.current().size;

        assert!(hole_u8.wrapping_add(hole_size) <= node_u8);

        unsafe {
            let maybe_next = self.hole.as_mut().next.replace(node);
            node.as_mut().next = maybe_next;
        }

        Ok(())
    }

    fn try_merge_next_n(self, max: usize) {
        let Self {
            prev: _,
            mut hole,
            top,
            ..
        } = self;

        for _ in 0..max {
            let mut next = if let Some(next) = unsafe { hole.as_mut() }.next.as_ref() {
                *next
            } else {
                check_merge_top(hole, top);
                return;
            };

            let hole_u8: *mut u8 = hole.as_ptr().cast();
            let hole_size = unsafe { hole.as_ref().size };
            let next_u8: *mut u8 = next.as_ptr().cast();
            let end = hole_u8.wrapping_add(hole_size);

            // touching
            if end == next_u8 {
                let next_size;
                let next_next;
                unsafe {
                    let next_mut = next.as_mut();
                    next_size = next_mut.size;
                    next_next = next_mut.next.take();
                }
                unsafe {
                    let hole_mut = hole.as_mut();
                    hole_mut.next = next_next;
                    hole_mut.size += next_size;
                }
            } else {
                hole = next;
            }
        }
    }
}

struct Heap {
    used: usize,
    holes: HoleList,
}

impl Heap {
    const fn empty() -> Self {
        Self {
            used: 0,
            holes: HoleList::empty(),
        }
    }

    unsafe fn init(&mut self, heap_bottom: *mut u8, heap_size: usize) {
        self.holes = HoleList::new(heap_bottom, heap_size);
        self.used = 0;
    }

    fn alloc_first_fit(&mut self, layout: Layout) -> Result<NonNull<u8>> {
        let (ptr, aligned_layout) = self.holes.alloc_first_fit(layout)?;
        self.used += aligned_layout.size();
        Ok(ptr)
    }

    fn dealloc(&mut self, ptr: NonNull<u8>, layout: Layout) -> Result<()> {
        self.used -= self.holes.dealloc(ptr, layout)?.size();
        Ok(())
    }
}

struct LinkedListAllocator {
    heap: Mutex<Heap>,
}

unsafe impl GlobalAlloc for LinkedListAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = self.heap.spin_lock().alloc_first_fit(layout).unwrap();
        ptr.as_ptr()
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.heap
            .spin_lock()
            .dealloc(NonNull::new_unchecked(ptr), layout)
            .unwrap()
    }
}

impl LinkedListAllocator {
    const fn empty() -> Self {
        Self {
            heap: Mutex::new(Heap::empty()),
        }
    }

    unsafe fn init(&mut self, heap_bottom: *mut u8, heap_size: usize) {
        self.heap.try_lock().unwrap().init(heap_bottom, heap_size)
    }
}

pub fn init_heap() -> Result<()> {
    let mem_frame_info = bitmap::alloc_mem_frame(HEAP_SIZE.div_ceil(PAGE_SIZE))?;
    let heap_start_virt_addr = mem_frame_info.frame_start_virt_addr()?;
    bitmap::mem_clear(&mem_frame_info)?;

    unsafe { ALLOCATOR.init(heap_start_virt_addr.as_ptr_mut(), mem_frame_info.frame_size) }
    Ok(())
}

fn align_up(addr: *mut u8, align: usize) -> *mut u8 {
    let offset = addr.align_offset(align);
    addr.wrapping_add(offset)
}

fn align_down_size(size: usize, align: usize) -> usize {
    if align.is_power_of_two() {
        size & !(align - 1)
    } else if align == 0 {
        size
    } else {
        panic!("align must be a power of two");
    }
}

fn align_up_size(size: usize, align: usize) -> usize {
    align_down_size(size + align - 1, align)
}

fn check_merge_top(mut node: NonNull<Hole>, top: *mut u8) {
    let node_u8: *mut u8 = node.as_ptr().cast();
    let node_size = unsafe { node.as_ref().size };

    let end = node_u8.wrapping_add(node_size);
    let hole_layout = Layout::new::<Hole>();
    if end < top {
        let next_hole_end = align_up(end, hole_layout.align()).wrapping_add(hole_layout.size());
        if next_hole_end > top {
            let offset = (top as usize) - (end as usize);
            unsafe { node.as_mut().size += offset };
        }
    }
}

fn check_merge_bottom(node: NonNull<Hole>, bottom: *mut u8) -> NonNull<Hole> {
    if bottom.wrapping_add(size_of::<Hole>()) > node.as_ptr().cast::<u8>() {
        let offset = (node.as_ptr() as usize) - (bottom as usize);
        let size = unsafe { node.as_ref() }.size + offset;
        unsafe { make_hole(bottom, size) }
    } else {
        node
    }
}

unsafe fn make_hole(addr: *mut u8, size: usize) -> NonNull<Hole> {
    let hole_addr: *mut Hole = addr.cast();
    hole_addr.write(Hole { size, next: None });
    NonNull::new_unchecked(hole_addr)
}

fn dealloc(list: &mut HoleList, addr: *mut u8, size: usize) -> Result<()> {
    let hole = unsafe { make_hole(addr, size) };
    let cursor = if let Some(cursor) = list.cursor() {
        cursor
    } else {
        let hole = check_merge_bottom(hole, list.bottom);
        check_merge_top(hole, list.top);
        list.first.next = Some(hole);
        return Ok(());
    };

    let (cursor, n) = match cursor.try_insert_back(hole, list.bottom) {
        Ok(cursor) => (cursor, 1),
        Err(mut curosr) => {
            while let Err(_) = curosr.try_insert_after(hole) {
                curosr = curosr.next().ok_or(Error::Failed("No next cursor"))?;
            }
            (curosr, 2)
        }
    };
    cursor.try_merge_next_n(n);
    Ok(())
}

#[alloc_error_handler]
fn alloc_error_handler(layout: Layout) -> ! {
    panic!("Allocation error: {:?}", layout);
}

#[test_case]
fn test_alloc_string() {
    use crate::alloc::string::ToString;

    let s1 = "Hello, World!".to_string();
    assert_eq!(s1, "Hello, World!");
    let s2 = "hoge huga hogera piyo 012345!\"#$%&".to_string();
    assert_eq!(s2, "hoge huga hogera piyo 012345!\"#$%&");
}

#[test_case]
fn test_alloc_long_string() {
    use alloc::string::String;

    let len = 100000;
    let mut s = String::new();
    for _ in 0..len {
        s.push('a');
    }
    assert_eq!(s.len(), len);

    for c in s.chars() {
        assert_eq!(c, 'a');
    }
}

#[test_case]
fn test_alloc_vec() {
    use alloc::vec::Vec;

    let mut v = Vec::new();

    for i in 0..1000 {
        v.push(i as u64);
    }

    assert_eq!(v.len(), 1000);
    assert_eq!(v.capacity(), 1024);
    assert_eq!(v[0], 0);
    assert_eq!(v[1], 1);
    assert_eq!(v[2], 2);
    assert_eq!(v[997], 997);
    assert_eq!(v[998], 998);
    assert_eq!(v[999], 999);
    assert!(v.get(1000).is_none());
}

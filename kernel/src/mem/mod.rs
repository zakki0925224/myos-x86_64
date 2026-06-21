use crate::{arch::x86_64, error::Result, kdebug, kinfo};
use common::mem_desc::MemoryDescriptor;

pub mod allocator;
pub mod bitmap;
pub mod paging;

pub fn init(mem_map: &[MemoryDescriptor]) -> Result<()> {
    bitmap::init(mem_map)?;
    kinfo!("mem: Bitmap memory manager initialized");

    let start = x86_64::paging::PAGE_SIZE as u64;
    let end = bitmap::total_mem_size()? as u64;

    x86_64::paging::kernel_init(start.into(), end.into())?;
    allocator::init_heap()?;
    kinfo!("mem: Heap allocator initialized");

    Ok(())
}

pub fn debug_usage() {
    fn format_size(size: usize) -> (f64, &'static str) {
        const KIB: usize = 1024;
        const MIB: usize = 1024 * KIB;
        const GIB: usize = 1024 * MIB;

        if size >= GIB {
            (size as f64 / GIB as f64, "GiB")
        } else if size >= MIB {
            (size as f64 / MIB as f64, "MiB")
        } else if size >= KIB {
            (size as f64 / KIB as f64, "KiB")
        } else {
            (size as f64, "B")
        }
    }

    let (used, max) = bitmap::mem_size().unwrap_or((0, 0));
    let (used_value, used_unit) = format_size(used);
    let (max_value, max_unit) = format_size(max);

    let (heap_used, heap_max) = allocator::heap_size();
    let (heap_used_value, heap_used_unit) = format_size(heap_used);
    let (heap_max_value, heap_max_unit) = format_size(heap_max);

    kdebug!(
        "Memory used: {:.2}{} / {:.2}{} ({:.2}%)",
        used_value,
        used_unit,
        max_value,
        max_unit,
        (used as f64 / max as f64) * 100f64
    );

    kdebug!(
        "Heap used: {:.2}{} / {:.2}{} ({:.2}%)",
        heap_used_value,
        heap_used_unit,
        heap_max_value,
        heap_max_unit,
        (heap_used as f64 / heap_max as f64) * 100f64
    );
}

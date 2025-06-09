use crate::{
    error::Result,
    info,
    mem::paging::{EntryMode, PageWriteThroughLevel, ReadWrite, PAGE_SIZE},
    println,
};
use common::mem_desc::MemoryDescriptor;

pub mod allocator;
pub mod bitmap;
pub mod frame_collection;
pub mod paging;

pub fn init(mem_map: &[MemoryDescriptor]) -> Result<()> {
    bitmap::init(mem_map)?;
    info!("mem: Bitmap memory manager initialized");

    let start = PAGE_SIZE as u64;
    let end = bitmap::get_total_mem_size()? as u64;

    paging::create_new_page_table(
        start.into(),
        end.into(),
        start.into(),
        ReadWrite::Write,
        EntryMode::Supervisor,
        PageWriteThroughLevel::WriteBack,
    )?;

    allocator::init_heap()?;
    info!("mem: Heap allocator initialized");

    Ok(())
}

pub fn free() {
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

    let (used, max) = bitmap::get_mem_size().unwrap_or((0, 0));
    let (used_value, used_unit) = format_size(used);
    let (max_value, max_unit) = format_size(max);

    println!(
        "Memory used: {:.2}{}({}B) / {:.2}{}({}B) ({:.2}%)",
        used_value,
        used_unit,
        used,
        max_value,
        max_unit,
        max,
        (used as f64 / max as f64) * 100f64
    );
}

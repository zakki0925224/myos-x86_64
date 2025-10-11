use crate::{
    arch::x86_64::{
        self,
        registers::{
            segment::{self, Cs, Ss},
            Register,
        },
        tss::{self, TaskStateSegmentDescriptor},
    },
    error::Result,
    kinfo,
    sync::mutex::Mutex,
};

pub const KERNEL_MODE_SS_VALUE: u16 = 2 << 3;
pub const KERNEL_MODE_CS_VALUE: u16 = 1 << 3;
pub const USER_MODE_SS_VALUE: u16 = (3 << 3) | 3;
pub const USER_MODE_CS_VALUE: u16 = (4 << 3) | 3;
const GDT_LEN: usize = 5;
const TSS_SEL: u16 = (GDT_LEN as u16) << 3;

static mut GDT: Mutex<GlobalDescriptorTable> = Mutex::new(GlobalDescriptorTable::new());

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
enum SegmentType {
    CodeExecuteRead = 0xa,
    DataReadWrite = 0x2,
}

#[derive(Debug, Clone, Copy)]
#[repr(C, align(8))]
struct SegmentDescriptor(u64);

impl SegmentDescriptor {
    pub const fn new() -> Self {
        Self(0)
    }

    pub fn set_code_seg(&mut self, seg_type: SegmentType, dpl: u8, base: u32, limit: u32) {
        self.set_base(base);
        self.set_limit(limit);

        self.set_seg_type(seg_type);
        self.set_is_code_or_data_seg(true);
        self.set_dpl(dpl);
        self.set_p(true);
        self.set_is_long_mode(true);
        self.set_default_op_size(false);
        self.set_granularity(true);
    }

    pub fn set_data_seg(&mut self, seg_type: SegmentType, dpl: u8, base: u32, limit: u32) {
        self.set_code_seg(seg_type, dpl, base, limit);
        self.set_is_long_mode(false);
        self.set_default_op_size(true);
    }

    fn set_base(&mut self, base: u32) {
        let base_low = base as u16;
        let base_mid = (base >> 16) as u8;
        let base_high = (base >> 24) as u8;

        self.0 = (self.0 & !0xffff_0000) | ((base_low as u64) << 16);
        self.0 = (self.0 & !0x000f_0000_0000) | ((base_mid as u64) << 32);
        self.0 = (self.0 & !0xff00_0000_0000_0000) | ((base_high as u64) << 56);
    }

    fn set_limit(&mut self, limit: u32) {
        let limit_low = limit as u16;
        let limit_high = (limit >> 16) as u8;

        self.0 = (self.0 & !0xffff) | (limit_low as u64);
        self.0 = (self.0 & !0x000f_0000_0000_0000) | ((limit_high as u64) << 48);
    }

    fn set_seg_type(&mut self, seg_type: SegmentType) {
        let seg_type = seg_type as u8;
        self.0 = (self.0 & !0x0f00_0000_0000) | ((seg_type as u64) << 40);
    }

    fn set_is_code_or_data_seg(&mut self, value: bool) {
        self.0 = (self.0 & !0x1000_0000_0000) | ((value as u64) << 44);
    }

    fn set_dpl(&mut self, dpl: u8) {
        let dpl = dpl;
        assert!(dpl <= 3);
        self.0 = (self.0 & !0x6000_0000_0000) | ((dpl as u64) << 45);
    }

    fn set_p(&mut self, value: bool) {
        self.0 = (self.0 & !0x8000_0000_0000) | ((value as u64) << 47);
    }

    fn set_is_long_mode(&mut self, value: bool) {
        self.0 = (self.0 & !0x0020_0000_0000_0000) | ((value as u64) << 53);
    }

    fn set_default_op_size(&mut self, value: bool) {
        self.0 = (self.0 & !0x0040_0000_0000_0000) | ((value as u64) << 54);
    }

    fn set_granularity(&mut self, value: bool) {
        self.0 = (self.0 & !0x0080_0000_0000_0000) | ((value as u64) << 55);
    }
}

#[repr(C)]
struct GlobalDescriptorTable {
    entries: [SegmentDescriptor; GDT_LEN],
    tss_desc: TaskStateSegmentDescriptor,
}

impl GlobalDescriptorTable {
    pub const fn new() -> Self {
        Self {
            entries: [SegmentDescriptor::new(); GDT_LEN],
            tss_desc: TaskStateSegmentDescriptor::new(),
        }
    }

    pub fn set_desc(&mut self, vec_num: usize, desc: SegmentDescriptor) {
        if vec_num >= GDT_LEN {
            return;
        }

        self.entries[vec_num] = desc;
    }

    pub fn set_tss_desc(&mut self, base: u64) {
        self.tss_desc.set(base);
    }

    pub fn load(&self) {
        let limit = (size_of::<Self>() - 1) as u16;
        let base = self.entries.as_ptr() as u64;

        let args = x86_64::DescriptorTableArgs { limit, base };
        x86_64::lgdt(&args);
        x86_64::ltr(TSS_SEL);
    }
}

pub fn init() -> Result<()> {
    let mut gdt1 = SegmentDescriptor::new();
    let mut gdt2 = SegmentDescriptor::new();
    let mut gdt3 = SegmentDescriptor::new();
    let mut gdt4 = SegmentDescriptor::new();

    // kernel segments
    gdt1.set_code_seg(SegmentType::CodeExecuteRead, 0, 0, 0x000f_ffff);
    gdt2.set_data_seg(SegmentType::DataReadWrite, 0, 0, 0x000f_ffff);

    // user segments
    gdt3.set_data_seg(SegmentType::DataReadWrite, 3, 0, 0x000f_ffff);
    gdt4.set_code_seg(SegmentType::CodeExecuteRead, 3, 0, 0x000f_ffff);

    let tss_addr = tss::init()?;

    {
        let mut gdt = unsafe { GDT.try_lock() }.unwrap();
        gdt.set_desc(1, gdt1);
        gdt.set_desc(2, gdt2);
        gdt.set_desc(3, gdt3);
        gdt.set_desc(4, gdt4);
        gdt.set_tss_desc(tss_addr);
        gdt.load();
    }

    segment::set_ds_es_fs_gs(0);
    segment::set_ss_cs(KERNEL_MODE_SS_VALUE, KERNEL_MODE_CS_VALUE);
    assert_eq!(Ss::read().raw(), KERNEL_MODE_SS_VALUE);
    assert_eq!(Cs::read().raw(), KERNEL_MODE_CS_VALUE);

    kinfo!("gdt: Initialized");
    kinfo!("gdt: TSS initialized");
    Ok(())
}

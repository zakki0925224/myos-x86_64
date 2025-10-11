use alloc::string::{String, ToString};
use core::arch::asm;

const CPUID_EAX_VENDOR_ID: u32 = 0;
const CPUID_EAX_VERSION_INFO: u32 = 1;
// const CPUID_EAX_RESERVED: u32 = 0x80000007;

#[derive(Debug)]
pub struct VersionInfo {
    pub stepping_id: u8,
    pub model_id: u8,
    pub family_id: u8,
    pub processor_type: u8,
    pub extended_model_id: u8,
    pub extended_family_id: u8,

    pub feature_fpu: bool,
    pub feature_vme: bool,
    pub feature_de: bool,
    pub feature_pse: bool,
    pub feature_tsc: bool,
    pub feature_msr: bool,
    pub feature_pae: bool,
    pub feature_mce: bool,
    pub feature_cx8: bool,
    pub feature_apic: bool,
    pub feature_sep: bool,
    pub feature_mtrr: bool,
    pub feature_pge: bool,
    pub feature_mca: bool,
    pub feature_cmov: bool,
    pub feature_pat: bool,
    pub feature_pse36: bool,
    pub feature_psn: bool,
    pub feature_clfsh: bool,
    pub feature_nx: bool,
    pub feature_ds: bool,
    pub feature_acpi: bool,
    pub feature_mmx: bool,
    pub feature_fxsr: bool,
    pub feature_sse: bool,
    pub feature_sse2: bool,
    pub feature_ss: bool,
    pub feature_htt: bool,
    pub feature_tm: bool,
    pub feature_ia64: bool,
    pub feature_pbe: bool,

    pub feature_sse3: bool,
    pub feature_pclmulqdq: bool,
    pub feature_dtes64: bool,
    pub feature_monitor: bool,
    pub feature_dscpl: bool,
    pub feature_vmx: bool,
    pub feature_smx: bool,
    pub feature_est: bool,
    pub feature_tm2: bool,
    pub feature_ssse3: bool,
    pub feature_cnxtid: bool,
    pub feature_sdbg: bool,
    pub feature_fma: bool,
    pub feature_cx16: bool,
    pub feature_xtpr: bool,
    pub feature_pdcm: bool,
    pub feature_pcid: bool,
    pub feature_dca: bool,
    pub feature_sse4_1: bool,
    pub feature_sse4_2: bool,
    pub feature_x2apic: bool,
    pub feature_movbe: bool,
    pub feature_popcnt: bool,
    pub feature_tscdeadline: bool,
    pub feature_aesni: bool,
    pub feature_xsave: bool,
    pub feature_osxsave: bool,
    pub feature_avx: bool,
    pub feature_f16c: bool,
    pub feature_rdrnd: bool,
    pub feature_hypervisor: bool,
    pub extended_flags: u32,
}

impl VersionInfo {
    fn parse(eax: u32, ebx: u32, ecx: u32, edx: u32) -> Self {
        Self {
            stepping_id: (eax & 0x0f) as u8,
            model_id: ((eax >> 4) & 0x0f) as u8,
            family_id: ((eax >> 8) & 0x0f) as u8,
            processor_type: ((eax >> 12) & 0x03) as u8,
            extended_model_id: ((eax >> 16) & 0x0f) as u8,
            extended_family_id: ((eax >> 20) & 0xff) as u8,
            feature_fpu: (edx & (1 << 0)) != 0,
            feature_vme: (edx & (1 << 1)) != 0,
            feature_de: (edx & (1 << 2)) != 0,
            feature_pse: (edx & (1 << 3)) != 0,
            feature_tsc: (edx & (1 << 4)) != 0,
            feature_msr: (edx & (1 << 5)) != 0,
            feature_pae: (edx & (1 << 6)) != 0,
            feature_mce: (edx & (1 << 7)) != 0,
            feature_cx8: (edx & (1 << 8)) != 0,
            feature_apic: (edx & (1 << 9)) != 0,
            feature_sep: (edx & (1 << 11)) != 0,
            feature_mtrr: (edx & (1 << 12)) != 0,
            feature_pge: (edx & (1 << 13)) != 0,
            feature_mca: (edx & (1 << 14)) != 0,
            feature_cmov: (edx & (1 << 15)) != 0,
            feature_pat: (edx & (1 << 16)) != 0,
            feature_pse36: (edx & (1 << 17)) != 0,
            feature_psn: (edx & (1 << 18)) != 0,
            feature_clfsh: (edx & (1 << 19)) != 0,
            feature_nx: (edx & (1 << 20)) != 0,
            feature_ds: (edx & (1 << 21)) != 0,
            feature_acpi: (edx & (1 << 22)) != 0,
            feature_mmx: (edx & (1 << 23)) != 0,
            feature_fxsr: (edx & (1 << 24)) != 0,
            feature_sse: (edx & (1 << 25)) != 0,
            feature_sse2: (edx & (1 << 26)) != 0,
            feature_ss: (edx & (1 << 27)) != 0,
            feature_htt: (edx & (1 << 28)) != 0,
            feature_tm: (edx & (1 << 29)) != 0,
            feature_ia64: (edx & (1 << 30)) != 0,
            feature_pbe: (edx & (1 << 31)) != 0,
            feature_sse3: (ecx & (1 << 0)) != 0,
            feature_pclmulqdq: (ecx & (1 << 1)) != 0,
            feature_dtes64: (ecx & (1 << 2)) != 0,
            feature_monitor: (ecx & (1 << 3)) != 0,
            feature_dscpl: (ecx & (1 << 4)) != 0,
            feature_vmx: (ecx & (1 << 5)) != 0,
            feature_smx: (ecx & (1 << 6)) != 0,
            feature_est: (ecx & (1 << 7)) != 0,
            feature_tm2: (ecx & (1 << 8)) != 0,
            feature_ssse3: (ecx & (1 << 9)) != 0,
            feature_cnxtid: (ecx & (1 << 10)) != 0,
            feature_sdbg: (ecx & (1 << 11)) != 0,
            feature_fma: (ecx & (1 << 12)) != 0,
            feature_cx16: (ecx & (1 << 13)) != 0,
            feature_xtpr: (ecx & (1 << 14)) != 0,
            feature_pdcm: (ecx & (1 << 15)) != 0,
            feature_pcid: (ecx & (1 << 17)) != 0,
            feature_dca: (ecx & (1 << 18)) != 0,
            feature_sse4_1: (ecx & (1 << 19)) != 0,
            feature_sse4_2: (ecx & (1 << 20)) != 0,
            feature_x2apic: (ecx & (1 << 21)) != 0,
            feature_movbe: (ecx & (1 << 22)) != 0,
            feature_popcnt: (ecx & (1 << 23)) != 0,
            feature_tscdeadline: (ecx & (1 << 24)) != 0,
            feature_aesni: (ecx & (1 << 25)) != 0,
            feature_xsave: (ecx & (1 << 26)) != 0,
            feature_osxsave: (ecx & (1 << 27)) != 0,
            feature_avx: (ecx & (1 << 28)) != 0,
            feature_f16c: (ecx & (1 << 29)) != 0,
            feature_rdrnd: (ecx & (1 << 30)) != 0,
            feature_hypervisor: (ecx & (1 << 31)) != 0,
            extended_flags: ebx,
        }
    }
}

fn cpuid(eax: u32) -> (u32, u32, u32, u32) {
    let eax_out;
    let ebx;
    let ecx;
    let edx;

    unsafe {
        asm!(
            "cpuid",
            in("eax") eax,
            lateout("eax") eax_out,
            lateout("ecx") ecx,
            lateout("edx") edx,
        );
        asm!("mov {:e}, ebx", out(reg) ebx);
    }

    (eax_out, ebx, ecx, edx)
}

pub fn vendor_id() -> String {
    let (_, ebx, ecx, edx) = cpuid(CPUID_EAX_VENDOR_ID);
    format!(
        "{}{}{}",
        String::from_utf8_lossy(&ebx.to_le_bytes()).to_string(),
        String::from_utf8_lossy(&edx.to_le_bytes()).to_string(),
        String::from_utf8_lossy(&ecx.to_le_bytes()).to_string()
    )
}

pub fn version_info() -> VersionInfo {
    let (eax, ebx, ecx, edx) = cpuid(CPUID_EAX_VERSION_INFO);
    VersionInfo::parse(eax, ebx, ecx, edx)
}

// pub fn invariant_tsc() -> bool {
//     let (_, _, _, edx) = cpuid(CPUID_EAX_RESERVED);
//     ((edx >> 8) & 1) != 0
// }

#[test_case]
fn test_cpuid() {
    assert_eq!(vendor_id(), "GenuineIntel"); // running KVM on Intel CPU
}

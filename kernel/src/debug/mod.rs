use crate::debug::dwarf::Dwarf;
use alloc::string::{String, ToString};

pub mod dwarf;
pub mod logger;
pub mod qemu;

pub fn symbolicate(dwarf: &Dwarf, ip: u64) -> String {
    let Some(info) = dwarf.find_debug_info_by_ip(ip) else {
        return format!("{:#x} in <UNKNOWN> at <UNKNOWN>", ip);
    };

    let mut function_name = None;
    let mut file_name = None;
    let mut dir_name = None;

    for (_, debug_abbrevs) in info {
        for debug_abbrev in debug_abbrevs {
            if !debug_abbrev.contains_ip(ip) {
                continue;
            }

            match debug_abbrev.tag {
                dwarf::AbbrevTag::CompileUnit => {
                    for (attr, form) in &debug_abbrev.attributes {
                        match (attr, form) {
                            (dwarf::AbbrevAttribute::Name, dwarf::AbbrevForm::LineStrp(name)) => {
                                file_name = Some(name.as_str());
                            }
                            (
                                dwarf::AbbrevAttribute::CompDir,
                                dwarf::AbbrevForm::LineStrp(name),
                            ) => {
                                dir_name = Some(name.as_str());
                            }
                            _ => (),
                        }
                    }
                }
                dwarf::AbbrevTag::Subprogram => {
                    for (attr, form) in &debug_abbrev.attributes {
                        match (attr, form) {
                            (dwarf::AbbrevAttribute::Name, dwarf::AbbrevForm::Strp(name)) => {
                                function_name = Some(name.as_str());
                            }
                            _ => (),
                        }
                    }
                }
                _ => (),
            }
        }
    }

    let file_path = file_name.and_then(|name| dir_name.map(|dir| format!("{}/{}", dir, name)));

    format!(
        "{:#x} in {} at {}",
        ip,
        function_name.unwrap_or("<UNKNOWN>"),
        file_path.unwrap_or("<UNKNOWN>".to_string())
    )
}

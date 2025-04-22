use crate::{
    arch::{self, idt::InterruptStackFrame},
    device::console,
    error::Result,
    print, println,
};
use alloc::string::ToString;
use dwarf::Dwarf;

pub mod dwarf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebuggerResult {
    Continue,
    Step,
    Quit,
}

pub fn user_app_debugger(
    stack_frame: &InterruptStackFrame,
    dwarf: &Dwarf,
) -> Result<DebuggerResult> {
    let ip = stack_frame.ins_ptr;

    if let Some(info) = dwarf.find_debug_info_by_ip(ip) {
        let mut function_name = None;
        let mut file_name = None;
        let mut dir_name = None;

        for (_, debug_abbrevs) in info {
            for debug_abbrev in debug_abbrevs {
                if !debug_abbrev.is_contains_by_ip(ip) {
                    continue;
                }

                match debug_abbrev.tag {
                    dwarf::AbbrevTag::CompileUnit => {
                        for (attr, form) in &debug_abbrev.attributes {
                            match (attr, form) {
                                (
                                    dwarf::AbbrevAttribute::Name,
                                    dwarf::AbbrevForm::LineStrp(name),
                                ) => {
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

        println!(
            "0x{:x} in {} at {}",
            ip,
            function_name.unwrap_or("<UNKNOWN>"),
            file_path.unwrap_or("<UNKNOWN>".to_string())
        );
    } else {
        println!("0x{:x} in <UNKNOWN> at <UNKNOWN>", ip);
    }

    let result;

    loop {
        print!("(dbg) ");
        let mut input_s = None;
        while input_s.is_none() {
            if let Ok(s) = arch::disabled_int(|| console::get_line()) {
                input_s = s;
            } else {
                arch::hlt();
            }
        }

        match input_s.unwrap().as_str().trim() {
            "q" => {
                result = DebuggerResult::Quit;
                break;
            }
            "c" => {
                result = DebuggerResult::Continue;
                break;
            }
            "s" => {
                result = DebuggerResult::Step;
                break;
            }
            s => {
                println!("Invalid command: {:?}", s);
                continue;
            }
        }
    }

    Ok(result)
}

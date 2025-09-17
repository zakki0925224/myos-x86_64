use crate::{
    device::{
        self,
        usb::{usb_bus::*, xhc::desc::*, UsbDeviceDriverFunction},
    },
    error::{Error, Result},
    graphics::{frame_buf, simple_window_manager},
    util,
};
use alloc::{collections::vec_deque::VecDeque, vec::Vec};

#[derive(Default, Debug)]
pub struct UsbHidMouseEvent {
    pub middle: bool,
    pub right: bool,
    pub left: bool,
    pub abs_x: usize,
    pub abs_y: usize,
}

pub struct UsbHidTabletDriver {
    pub name: &'static str,
    interface_num: u8,
    input_report_items: Vec<UsbHidReportInputItem>,
    report_size_in_byte: usize,
    prev_report: Vec<u8>,
    res: (usize, usize),
}

impl UsbDeviceDriverFunction for UsbHidTabletDriver {
    fn configure(&mut self, attach_info: &mut UsbDeviceAttachInfo) -> Result<()> {
        let xhci_info = match attach_info {
            UsbDeviceAttachInfo::Xhci(info) => info,
        };
        let slot = xhci_info.slot;
        let interface_descs = xhci_info.interface_descs();
        let target_interface_desc = *interface_descs
            .iter()
            .find(|d| d.triple() == (3, 0, 0))
            .ok_or(Error::Failed("No target interface descriptor found"))?;
        self.interface_num = target_interface_desc.interface_num;

        // request HID report
        let report = device::usb::xhc::request(|xhc| {
            xhc.hid_report_desc(slot, xhci_info.ctrl_ep_ring_mut(), self.interface_num, 4096)
        })?;

        self.input_report_items = self.parse_hid_report_desc(&report)?;
        self.report_size_in_byte = if let Some(last_item) = self.input_report_items.last() {
            (last_item.bit_offset + last_item.bit_size + 7) / 8
        } else {
            return Err(Error::Failed("Report size is zero"));
        };
        self.prev_report = vec![0u8; self.report_size_in_byte];
        self.res = frame_buf::resolution()?;

        Ok(())
    }

    fn poll(&mut self, attach_info: &mut UsbDeviceAttachInfo) -> Result<()> {
        let xhci_info = match attach_info {
            UsbDeviceAttachInfo::Xhci(info) => info,
        };
        let slot = xhci_info.slot;

        let desc_button_l = self
            .input_report_items
            .iter()
            .find(|item| item.usage == UsbHidUsage::Button(1))
            .ok_or(Error::Failed("Button(1) not found"))?;
        let desc_button_r = self
            .input_report_items
            .iter()
            .find(|item| item.usage == UsbHidUsage::Button(2))
            .ok_or(Error::Failed("Button(2) not found"))?;
        let desc_button_c = self
            .input_report_items
            .iter()
            .find(|item| item.usage == UsbHidUsage::Button(3))
            .ok_or(Error::Failed("Button(3) not found"))?;
        let desc_abs_x = self
            .input_report_items
            .iter()
            .find(|item| item.usage == UsbHidUsage::X && item.is_absolute)
            .ok_or(Error::Failed("Absolute X not found"))?;
        let desc_abs_y = self
            .input_report_items
            .iter()
            .find(|item| item.usage == UsbHidUsage::Y && item.is_absolute)
            .ok_or(Error::Failed("Absolute Y not found"))?;

        let report =
            device::usb::xhc::request(|xhc| xhc.hid_report(slot, xhci_info.ctrl_ep_ring_mut()))?;

        if report == self.prev_report {
            return Ok(());
        }

        let (res_x, res_y) = self.res;

        let l = desc_button_l.value_from_report(&report) == Some(1);
        let r = desc_button_r.value_from_report(&report) == Some(1);
        let c = desc_button_c.value_from_report(&report) == Some(1);
        let ax = desc_abs_x.mapped_range_from_report(&report, 0..=(res_x as i64 - 1))? as usize;
        let ay = desc_abs_y.mapped_range_from_report(&report, 0..=(res_y as i64 - 1))? as usize;

        let mouse_event = UsbHidMouseEvent {
            left: l,
            right: r,
            middle: c,
            abs_x: ax,
            abs_y: ay,
        };

        self.prev_report = report;
        let _ = simple_window_manager::mouse_pointer_event(
            simple_window_manager::MouseEvent::UsbHidMouse(mouse_event),
        );

        Ok(())
    }
}

impl UsbHidTabletDriver {
    pub fn new() -> Self {
        Self {
            name: "usb-hid-tablet",
            interface_num: 0,
            input_report_items: Vec::new(),
            report_size_in_byte: 0,
            prev_report: Vec::new(),
            res: (0, 0),
        }
    }

    fn parse_hid_report_desc(&self, report: &[u8]) -> Result<Vec<UsbHidReportInputItem>> {
        let mut it = report.iter();
        let mut input_report_items = Vec::new();
        let mut usage_queue = VecDeque::new();
        let mut usage_page = None;
        let mut usage_min = None;
        let mut usage_max = None;
        let mut report_size = 0;
        let mut report_count = 0;
        let mut bit_offset = 0;
        let mut logical_min = 0;
        let mut logical_max = 0;

        while let Some(prefix) = it.next() {
            let b_size = match prefix & 0b11 {
                0b11 => 4,
                e => e,
            } as usize;
            let b_type = match (prefix >> 2) & 0b11 {
                0 => UsbHidReportItemType::Main,
                1 => UsbHidReportItemType::Global,
                2 => UsbHidReportItemType::Local,
                _ => unimplemented!(), // reserved
            };
            let b_tag = prefix >> 4;
            let data: Vec<u8> = it.by_ref().take(b_size).cloned().collect();
            let data_value = {
                let mut data = data.clone();
                data.resize(4, 0);
                let mut value = [0u8; 4];
                value.copy_from_slice(&data);
                u32::from_le_bytes(value)
            };

            match (&b_type, &b_tag) {
                (UsbHidReportItemType::Main, 0b1000) => {
                    if let Some(usage_page) = usage_page {
                        let is_constant = util::bits::extract_bits(data_value, 0, 1) == 1;
                        let is_array = util::bits::extract_bits(data_value, 1, 1) == 1;
                        let is_absolute = util::bits::extract_bits(data_value, 2, 1) == 0;
                        for i in 0..report_count {
                            let report_usage = if let Some(usage) = usage_queue.pop_front() {
                                usage
                            } else if let (
                                UsbHidUsagePage::Button,
                                Some(usage_min),
                                Some(usage_max),
                            ) = (usage_page, usage_min, usage_max)
                            {
                                let btn_idx = usage_min + i;
                                if btn_idx <= usage_max {
                                    UsbHidUsage::Button(btn_idx)
                                } else {
                                    UsbHidUsage::Unknown(btn_idx)
                                }
                            } else if is_constant {
                                UsbHidUsage::Constant
                            } else {
                                UsbHidUsage::Unknown(0)
                            };

                            input_report_items.push(UsbHidReportInputItem {
                                usage: report_usage,
                                bit_size: report_size,
                                is_array,
                                is_absolute,
                                bit_offset,
                                logical_min,
                                logical_max,
                            });

                            bit_offset += report_size;
                        }
                    }
                }
                (UsbHidReportItemType::Global, 0b0000) => {
                    usage_page = Some(match data_value {
                        0x01 => UsbHidUsagePage::GenericDesktop,
                        0x09 => UsbHidUsagePage::Button,
                        _ => UsbHidUsagePage::Unknown(data_value as usize),
                    });
                }
                (UsbHidReportItemType::Global, 0b0001) => {
                    logical_min = data_value;
                }
                (UsbHidReportItemType::Global, 0b0010) => {
                    logical_max = data_value;
                }
                (UsbHidReportItemType::Global, 0b0111) => {
                    report_size = data_value as usize;
                }
                (UsbHidReportItemType::Global, 0b1001) => {
                    report_count = data_value as usize;
                }
                (UsbHidReportItemType::Local, 0) => {
                    let usage = match &usage_page {
                        Some(UsbHidUsagePage::GenericDesktop) => match data_value {
                            0x01 => UsbHidUsage::Pointer,
                            0x02 => UsbHidUsage::Mouse,
                            0x30 => UsbHidUsage::X,
                            0x31 => UsbHidUsage::Y,
                            0x38 => UsbHidUsage::Wheel,
                            _ => UsbHidUsage::Unknown(data_value as usize),
                        },
                        _ => UsbHidUsage::Unknown(data_value as usize),
                    };
                    usage_queue.push_back(usage);
                }
                (UsbHidReportItemType::Local, 1) => {
                    usage_min = Some(data_value as usize);
                }
                (UsbHidReportItemType::Local, 2) => {
                    usage_max = Some(data_value as usize);
                }
                _ => (),
            }

            if matches!(b_type, UsbHidReportItemType::Main) {
                usage_queue.clear();
                usage_min = None;
                usage_max = None;
            }
        }

        Ok(input_report_items)
    }
}

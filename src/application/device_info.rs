// Copyright 2023 Anton Kharuzhyi <publicantroids@gmail.com>
// SPDX-License-Identifier: GPL-3.0

use crate::application;
use crate::application::{ConnectedDevice, UNDEFINED_STRING};
use eframe::egui;
use hidapi::BusType;
use std::ffi::CString;

pub fn device_info(ui: &mut egui::Ui, info: &DeviceInfo) {
    egui::Grid::new("Info").num_columns(2).show(ui, |ui| {
        ui.label("Path:");
        ui.label(format!("{:?}", info.path));
        ui.end_row();
        ui.label("Vendor Id:");
        ui.label(format!("{:#04x}", info.vendor_id));
        ui.end_row();
        ui.label("Product Id:");
        ui.label(format!("{:#04x}", info.product_id));
        ui.end_row();
        ui.label("Serial Number:");
        ui.label(format!(
            "{}",
            info.serial_number
                .as_ref()
                .unwrap_or(&UNDEFINED_STRING.to_string())
        ));
        ui.end_row();
        ui.label("Release Number:");
        ui.label(info.release_number.to_string());
        ui.end_row();
        ui.label("Manufacturer:");
        ui.label(format!(
            "{}",
            info.manufacturer_string
                .as_ref()
                .unwrap_or(&UNDEFINED_STRING.to_string())
        ));
        ui.end_row();
        ui.label("Product:");
        ui.label(format!(
            "{}",
            info.product_string
                .as_ref()
                .unwrap_or(&UNDEFINED_STRING.to_string())
        ));
        ui.end_row();
        ui.label("Interface Number:");
        ui.label(info.interface_number.to_string());
        ui.end_row();
        ui.label("Bus Type:");
        ui.label(format!("{:?}", info.bus_type));
        ui.end_row();
    });
}

pub struct DeviceInfo {
    pub path: CString,
    pub vendor_id: u16,
    pub product_id: u16,
    pub serial_number: Option<String>,
    pub release_number: u16,
    pub manufacturer_string: Option<String>,
    pub product_string: Option<String>,
    pub interface_number: i32,
    pub bus_type: BusType,
}

impl DeviceInfo {
    pub(super) fn from_connected_device(
        connected_device: &ConnectedDevice,
    ) -> application::Result<Self> {
        Ok(match connected_device {
            ConnectedDevice::DualShock4(_device, ds4) => {
                let info = ds4.hid_device().get_device_info()?;
                Self {
                    path: CString::from(info.path()),
                    vendor_id: info.vendor_id(),
                    product_id: info.product_id(),
                    serial_number: info.serial_number().map(|s| s.to_string()),
                    release_number: info.release_number(),
                    manufacturer_string: info.manufacturer_string().map(|s| s.to_string()),
                    product_string: info.product_string().map(|s| s.to_string()),
                    interface_number: info.interface_number(),
                    bus_type: info.bus_type(),
                }
            }
        })
    }
}

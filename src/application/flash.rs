use crate::application::{ConnectedDevice, DeviceConnected, Panel, StatusHandler};
use crate::dual_shock_4::{FlashMirror, TestCommand};
use eframe::egui;
use eframe::egui::{Color32, RichText};
use std::fs::OpenOptions;
use std::io::{Read, Write};

#[derive(Default)]
pub struct Flash {
    flash_mirror: Option<FlashMirror>,
}

pub fn flash(
    ui: &mut egui::Ui,
    _ctx: &egui::Context,
    state: &mut DeviceConnected,
    sh: StatusHandler,
) {
    if ui
        .checkbox(
            &mut state.permanent,
            "Save changes to permanent memory (WARNING: you can brick you device)",
        )
        .changed()
    {
        let ConnectedDevice::DualShock4(_, ds4) = &state.device;
        let _ = sh.handle_error(ds4.set_test_command(TestCommand::SetPermanent(state.permanent)));
        state.permanent = sh.handle_error(ds4.read_permanent()).unwrap_or(false);
    }
    if ui.button("Read Flash From Device").clicked() {
        let ConnectedDevice::DualShock4(_, ds4) = &state.device;
        if let Some(flash_mirror_from_device) = sh.handle_error(ds4.read_flash_mirror()) {
            if let Panel::Flash(Flash { flash_mirror }) = &mut state.panel {
                *flash_mirror = Some(flash_mirror_from_device);
            }
        }
    }
    if ui.button("Load Flash From File").clicked() {
        if let Some(file) = rfd::FileDialog::new()
            .add_filter("hex", &["hex"])
            .pick_file()
        {
            if let Panel::Flash(Flash { flash_mirror }) = &mut state.panel {
                let file_options = OpenOptions::new().read(true).open(file);
                *flash_mirror = None;
                if let Some(mut file) = sh.handle_error(file_options) {
                    let mut flash_mirror_from_file = FlashMirror::default();
                    sh.handle_error(file.read_exact(&mut flash_mirror_from_file.buf));
                    *flash_mirror = Some(flash_mirror_from_file);
                }
            }
        }
    }
    if let Panel::Flash(Flash {
        flash_mirror: Some(flash_mirror),
    }) = &state.panel
    {
        ui.horizontal(|ui| {
            ui.label("Flash Mirror CRC: ");
            if flash_mirror.check_crc() {
                ui.label(RichText::new("Correct").color(Color32::GREEN));
            } else {
                ui.label(RichText::new("Invalid").color(Color32::RED));
            }
        });
    }
    if let Panel::Flash(Flash {
        flash_mirror: Some(flash_mirror),
    }) = &state.panel
    {
        if ui.button("Save Flash Dump to File").clicked() {
            if let Some(file) = rfd::FileDialog::new()
                .set_file_name("ds4_ieep.hex")
                .save_file()
            {
                let file_options = OpenOptions::new().create_new(true).write(true).open(file);
                if let Some(mut file) = sh.handle_error(file_options) {
                    sh.handle_error(file.write_all(&flash_mirror.buf));
                }
            }
        }
    }
}

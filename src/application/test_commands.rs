use crate::application::{ConnectedDevice, DeviceConnected, StatusHandler};
use crate::dual_shock_4::{TestCommand, TriggerKeyLeftRight};
use eframe::egui;
use std::fmt::format;

pub fn test_commands(
    ui: &mut egui::Ui,
    _ctx: &egui::Context,
    state: &mut DeviceConnected,
    sh: StatusHandler,
) {
    let ConnectedDevice::DualShock4(_, ds4) = &state.device;
    let mut update_test_data_required = false;
    ui.heading("Test Commands");
    ui.label("If you Gamepad is not bricked yet, you can try some pretty buttons from this page");
    if let super::Panel::Test(Some(test_data), _) = &state.panel {
        ui.separator();
        ui.label(format!("Test args & data: {:?}", test_data));
        ui.label(format!(
            "Text data (hex): {}",
            hex::encode(test_data.data())
        ));
        if let Ok(str) = String::from_utf8(test_data.data().clone()) {
            ui.label(format!("Text data (string): {}", str));
        }
    }
    if let super::Panel::Test(_, Some(brick_device_payload)) = &mut state.panel {
        ui.separator();
        ui.label("This is the easiest way to brick you controller!");
        ui.horizontal(|ui| {
            ui.label("Test command in hex format. 0xa0:");
            ui.text_edit_singleline(brick_device_payload);
        });
        if ui.button("Send Test Command").clicked() {
            if let Ok(decoded) = hex::decode(brick_device_payload) {
                let _ =
                    sh.handle_error(ds4.set_test_command(TestCommand::BrickYourDevice(decoded)));
                update_test_data_required = true;
            }
        }
    } else if let Some(ds4_data) = sh.handle_error(ds4.read_last_data()).flatten() {
        if ds4_data.triangle() && ds4_data.cross() && ds4_data.square() && ds4_data.circle() {
            state.panel = super::Panel::Test(None, Some("0802".to_string()));
        }
    }
    ui.separator();
    ui.label("Force triggers calibration (It seems blocked in current firmware)");
    if ui.button("Record Left trigger Min value").clicked() {
        let _ = sh.handle_error(ds4.set_test_command(TestCommand::RecordTriggerMinMax(
            TriggerKeyLeftRight::Left,
            true,
        )));
        update_test_data_required = true;
    }
    if ui.button("Record Left trigger Max value").clicked() {
        let _ = sh.handle_error(ds4.set_test_command(TestCommand::RecordTriggerMinMax(
            TriggerKeyLeftRight::Left,
            false,
        )));
        update_test_data_required = true;
    }
    if ui.button("Record Right trigger Min value").clicked() {
        let _ = sh.handle_error(ds4.set_test_command(TestCommand::RecordTriggerMinMax(
            TriggerKeyLeftRight::Right,
            true,
        )));
        update_test_data_required = true;
    }
    if ui.button("Record Right trigger Max value").clicked() {
        let _ = sh.handle_error(ds4.set_test_command(TestCommand::RecordTriggerMinMax(
            TriggerKeyLeftRight::Right,
            false,
        )));
        update_test_data_required = true;
    }
    if ui.button("Get recorded values").clicked() {
        let _ = sh.handle_error(ds4.set_test_command(TestCommand::ReadTriggerMinMaxFromFlash));
        update_test_data_required = true;
    }
    ui.separator();
    if ui.button("Reset Device").clicked() {
        let _ = sh.handle_error(ds4.set_test_command(TestCommand::ResetDevice));
        update_test_data_required = true;
    }
    ui.separator();

    if update_test_data_required {
        update_test_data(state, sh);
    }
}

fn update_test_data(state: &mut DeviceConnected, sh: StatusHandler) {
    if let super::Panel::Test(_, brick_device_payload) = &state.panel {
        let ConnectedDevice::DualShock4(_, ds4) = &state.device;
        let test_data = sh.handle_error(ds4.read_test_data());
        state.panel = super::Panel::Test(test_data, brick_device_payload.clone());
    }
}

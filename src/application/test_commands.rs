use crate::application::{ConnectedDevice, DeviceConnected, StatusHandler};
use crate::dual_shock_4::{TestCommand, TriggerKeyLeftRight};
use eframe::egui;

pub fn test_commands(
    ui: &mut egui::Ui,
    _ctx: &egui::Context,
    state: &mut DeviceConnected,
    sh: StatusHandler,
) {
    ui.heading("Test Commands");
    ui.label("If you Gamepad is not bricked yet, you can try some pretty buttons from this page");
    ui.separator();
    ui.label("Force triggers calibration");
    let ConnectedDevice::DualShock4(_, ds4) = &state.device;
    if ui.button("Record Left trigger Min value").clicked() {
        let _ = sh.handle_error(ds4.set_test_command(TestCommand::RecordTriggerMinMax(
            TriggerKeyLeftRight::Left,
            true,
        )));
    }
    if ui.button("Record Left trigger Max value").clicked() {
        let _ = sh.handle_error(ds4.set_test_command(TestCommand::RecordTriggerMinMax(
            TriggerKeyLeftRight::Left,
            false,
        )));
    }
    if ui.button("Record Right trigger Min value").clicked() {
        let _ = sh.handle_error(ds4.set_test_command(TestCommand::RecordTriggerMinMax(
            TriggerKeyLeftRight::Right,
            true,
        )));
    }
    if ui.button("Record Right trigger Max value").clicked() {
        let _ = sh.handle_error(ds4.set_test_command(TestCommand::RecordTriggerMinMax(
            TriggerKeyLeftRight::Right,
            false,
        )));
    }
    if ui.button("Apply recorded values").clicked() {
        let _ = sh.handle_error(ds4.set_test_command(TestCommand::ReloadTriggerMinMaxFromFlash));
    }
    ui.separator();
}

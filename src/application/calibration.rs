// Copyright 2023 Anton Kharuzhyi <publicantroids@gmail.com>
// SPDX-License-Identifier: GPL-3.0

use crate::application::font::{
    button_cross, button_triangle, with_gamepad_font, GAMEPAD_FONT_LEFT_ANALOG_CLOCKWISE,
    GAMEPAD_FONT_RIGHT_ANALOG_CLOCKWISE,
};
use crate::application::output::{circle_line, trigger_bar};
use crate::application::{panel_switch_button, ConnectedDevice, DeviceConnected, StatusHandler};
use crate::dual_shock_4::{
    AnalogStickCalibrationType, CalibrationData, CalibrationDeviceType, CalibrationFlag,
    CalibrationResult, CalibrationState, CalibrationType, Data, MotionCalibration,
    StickCenterCalibration, StickMinMaxCalibration, StickPosition, TriggerKeyCalibrationType,
    TriggerKeyLeftRight,
};
use eframe::egui;
use eframe::egui::{Color32, ScrollArea, SliderClamping};
use egui_plot::Points;

#[derive(Clone)]
pub enum Panel {
    Info(Info),
    Wizard(CalibrationWizard),
    MotionSensor(MotionCalibration),
}

impl Panel {
    pub fn info_from_device_connected(
        device_connected: &DeviceConnected,
        sh: StatusHandler,
    ) -> Option<Self> {
        let ConnectedDevice::DualShock4(_, ds4) = &device_connected.device;
        let flag = sh.handle_error(ds4.read_calibration_flag());
        flag.map(|flag| Panel::Info(Info { flag }))
    }
}

#[derive(PartialEq, Clone)]
pub enum CalibrationWizard {
    Start,
    AnalogStickCenter,
    AnalogStickMinMax,
    TriggerKey(TriggerKeyCalibrationType),
    Success(CalibrationDeviceType, CalibrationData),
    Failed,
}

#[derive(Clone)]
pub struct Info {
    flag: CalibrationFlag,
}

pub fn calibration(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    state: &mut DeviceConnected,
    sh: StatusHandler,
) {
    ui.horizontal(|ui| {
        if panel_switch_button(
            ui,
            matches!(state.panel, super::Panel::Calibration(Panel::Info(_))),
            "Calibration Info",
        )
            .clicked()
        {
            if let Some(panel) = Panel::info_from_device_connected(state, sh.clone()) {
                state.panel = super::Panel::Calibration(panel);
            }
        }
        if panel_switch_button(
            ui,
            matches!(state.panel, super::Panel::Calibration(Panel::Wizard(_))),
            "Calibration Wizard",
        )
            .clicked()
        {
            update_calibration_wizard_panel(state, sh.clone());
        }
        if panel_switch_button(
            ui,
            matches!(
                state.panel,
                super::Panel::Calibration(Panel::MotionSensor(_))
            ),
            "Motion Sensor",
        )
            .clicked()
        {
            let ConnectedDevice::DualShock4(_, ds4) = &state.device;
            if let Some(calibration_from_device) =
                sh.handle_error(ds4.read_motion_calibration_data())
            {
                state.panel =
                    super::Panel::Calibration(Panel::MotionSensor(calibration_from_device));
            }
        }
    });
    ui.separator();
    match &state.panel {
        super::Panel::Calibration(Panel::Info(_)) => info_panel(ui, state, sh.clone()),
        super::Panel::Calibration(Panel::Wizard(CalibrationWizard::Start)) => {
            calibration_wizard_start(ui, state, sh.clone())
        }
        super::Panel::Calibration(Panel::Wizard(CalibrationWizard::AnalogStickCenter)) => {
            stick_center_calibration(ui, ctx, state, sh.clone())
        }
        super::Panel::Calibration(Panel::Wizard(CalibrationWizard::AnalogStickMinMax)) => {
            stick_min_max_calibration(ui, ctx, state, sh.clone())
        }
        super::Panel::Calibration(Panel::Wizard(CalibrationWizard::Success(_, _))) => {
            calibration_success(ui, state, sh.clone())
        }
        super::Panel::Calibration(Panel::Wizard(CalibrationWizard::Failed)) => {
            calibration_failed(ui, state, sh.clone())
        }
        super::Panel::Calibration(Panel::MotionSensor(_)) => {
            motion_calibration(ui, state, sh.clone())
        }
        super::Panel::Calibration(Panel::Wizard(CalibrationWizard::TriggerKey(type_))) => {
            triggers_calibration(ui, ctx, state, type_.clone(), sh.clone())
        }
        _ => {
            ui.label("Unknown calibration sub-panel");
        }
    };
}

fn info_panel(ui: &mut egui::Ui, state: &mut DeviceConnected, _sh: StatusHandler) {
    if let super::Panel::Calibration(Panel::Info(info)) = &state.panel {
        ui.columns(2, |columns| {
            columns[0].label("Accelerometer Calibrated: ");
            columns[1].label(info.flag.is_accelerometer_calib_ok().to_string());
            columns[0].label("Gyroscope Calibrated: ");
            columns[1].label(info.flag.is_gyroscope_calib_ok().to_string());
            columns[0].label("Sticks Min/Max Calibrated: ");
            columns[1].label(info.flag.is_stick_min_max_calib_ok().to_string());
            columns[0].label("Sticks Centers Calibrated: ");
            columns[1].label(info.flag.is_stick_center_calib_ok().to_string());
            columns[0].label("Left Trigger Calibrated: ");
            columns[1].label(info.flag.is_l2_calib_ok().to_string());
            columns[0].label("Right Trigger Calibrated: ");
            columns[1].label(info.flag.is_r2_calib_ok().to_string());
        });
    }
}

fn update_calibration_wizard_panel(state: &mut DeviceConnected, sh: StatusHandler) {
    if let Some(wizard) = sh.handle_error(calibration_wizard_panel(state)) {
        state.panel = super::Panel::Calibration(Panel::Wizard(wizard));
    }
}

fn calibration_wizard_panel(state: &mut DeviceConnected) -> super::Result<CalibrationWizard> {
    let ConnectedDevice::DualShock4(_, ds4) = &state.device;
    let calibration_state = ds4.read_calibration_state()?;

    Ok(match calibration_state {
        CalibrationState::Started(CalibrationDeviceType::AnalogStick(
                                      AnalogStickCalibrationType::Center,
                                  )) => CalibrationWizard::AnalogStickCenter,
        CalibrationState::Started(CalibrationDeviceType::AnalogStick(
                                      AnalogStickCalibrationType::MinMax,
                                  )) => CalibrationWizard::AnalogStickMinMax,
        CalibrationState::Started(CalibrationDeviceType::TriggerKey(_)) => {
            CalibrationWizard::TriggerKey(TriggerKeyCalibrationType::RecordMaxSample(
                TriggerKeyLeftRight::Both,
            ))
        }
        CalibrationState::Finished(_) => {
            let calibration_result = ds4.read_calibration_result()?;
            match calibration_result {
                CalibrationResult::Completed(device) => {
                    let calibration_data = ds4.read_calibration_data()?;
                    CalibrationWizard::Success(device, calibration_data)
                }
                CalibrationResult::NotCompleted(_) => CalibrationWizard::Failed,
            }
        }
        CalibrationState::Unknown => CalibrationWizard::Start,
        _ => todo!(),
    })
}

fn calibration_wizard_start(ui: &mut egui::Ui, state: &mut DeviceConnected, sh: StatusHandler) {
    start_calibration_buttons(ui, state, sh.clone());
}

fn calibration_success(ui: &mut egui::Ui, state: &mut DeviceConnected, sh: StatusHandler) {
    if let super::Panel::Calibration(Panel::Wizard(CalibrationWizard::Success(
                                                       calibration_device_type,
                                                       calibration_data,
                                                   ))) = &state.panel
    {
        ui.horizontal(|ui| {
            match calibration_device_type {
                CalibrationDeviceType::AnalogStick(AnalogStickCalibrationType::Center) => {
                    ui.label("Analog Sticks Center calibrated successful!");
                }
                CalibrationDeviceType::AnalogStick(AnalogStickCalibrationType::MinMax) => {
                    ui.label("Analog Sticks Min/Max calibrated successful!");
                }
                CalibrationDeviceType::MotionSensor => {
                    ui.label("Motion Sensor calibrated successful!");
                }
                CalibrationDeviceType::TriggerKey(_) => {
                    ui.label("Trigger Key calibrated successful!");
                }
                _ => {}
            };
        });

        ui.separator();
        calibration_data_form(ui, &calibration_data);
    }
    ui.separator();
    start_calibration_buttons(ui, state, sh.clone());
}

fn calibration_failed(ui: &mut egui::Ui, state: &mut DeviceConnected, sh: StatusHandler) {
    ui.heading("Calibration Failed!");
    start_calibration_buttons(ui, state, sh.clone());
}

fn start_calibration_buttons(ui: &mut egui::Ui, state: &mut DeviceConnected, sh: StatusHandler) {
    {
        let ConnectedDevice::DualShock4(_, ds4) = &state.device;
        let mut panel_update_required = false;
        if ui
            .button("Calibrate Analog Sticks Center Position")
            .clicked()
        {
            let _ = sh.handle_error(ds4.set_calibration_command(CalibrationType::Start(
                CalibrationDeviceType::AnalogStick(AnalogStickCalibrationType::Center),
            )));
            panel_update_required = true;
        }
        if ui.button("Calibrate Analog Sticks Min/Max Range").clicked() {
            let _ = sh.handle_error(ds4.set_calibration_command(CalibrationType::Start(
                CalibrationDeviceType::AnalogStick(AnalogStickCalibrationType::MinMax),
            )));
            panel_update_required = true;
        }
        if ui.button("Calibrate Triggers Keys").clicked() {
            let _ = sh.handle_error(ds4.set_calibration_command(CalibrationType::Start(
                CalibrationDeviceType::TriggerKey(TriggerKeyCalibrationType::Unknown(
                    TriggerKeyLeftRight::Both,
                )),
            )));
            state.panel = super::Panel::Calibration(Panel::Wizard(CalibrationWizard::TriggerKey(
                TriggerKeyCalibrationType::Unknown(TriggerKeyLeftRight::Both),
            )));
            panel_update_required = false;
        }
        if ui.button("Force Read Calibration Data").clicked() {
            if let Some(calibration_data) = sh.handle_error(ds4.read_calibration_data()) {
                state.panel = super::Panel::Calibration(Panel::Wizard(CalibrationWizard::Success(
                    CalibrationDeviceType::None,
                    calibration_data,
                )));
            }
            panel_update_required = false;
        }
        if panel_update_required {
            update_calibration_wizard_panel(state, sh.clone());
        }
    }
}

fn calibration_data_form(ui: &mut egui::Ui, calibration_data: &CalibrationData) {
    ScrollArea::vertical().show(ui, |ui| {
        ui.add_enabled_ui(false, |ui| match calibration_data {
            CalibrationData::StickCenter(calculated, samples) => {
                let mut calculated = calculated.clone();
                ui.label("Calculated calibration data: ");
                stick_center_calibration_form(ui, &mut calculated);
                for (i, sample) in samples.iter().enumerate() {
                    ui.label(format!("Collected sample data {}:", i));
                    let mut sample = sample.clone();
                    stick_center_calibration_form(ui, &mut sample);
                }
            }
            CalibrationData::StickMinMax(calibration) => {
                let mut calibration = calibration.clone();
                ui.label("Calibration data: ");
                stick_min_max_calibration_form(ui, &mut calibration);
            }
            CalibrationData::Triggers(calibration) => {
                ui.label("Calibration data: ");
                ui.label(hex::encode(calibration.buf.as_slice()));
            }
            CalibrationData::None(data) => {
                ui.label("Unknown calibration data: ");
                ui.label(hex::encode(data));
            }
        });
    });
}

fn stick_center_calibration_form(ui: &mut egui::Ui, calibration: &mut StickCenterCalibration) {
    ui.columns(2, |columns| {
        let mut left_x_center = calibration.left_x();
        let mut left_y_center = calibration.left_y();
        let mut right_x_center = calibration.right_x();
        let mut right_y_center = calibration.right_y();
        if columns[0]
            .add(center_calibration_slider(
                &mut left_x_center,
                "Left Stick X-Axis Center",
            ))
            .changed()
        {
            calibration.set_left_x(left_x_center);
        }
        if columns[1]
            .add(center_calibration_slider(
                &mut right_x_center,
                "Right Stick X-Axis Center",
            ))
            .changed()
        {
            calibration.set_right_x(right_x_center);
        }

        if columns[0]
            .add(center_calibration_slider(
                &mut left_y_center,
                "Left Stick Y-Axis Center",
            ))
            .changed()
        {
            calibration.set_left_y(left_y_center);
        }
        if columns[1]
            .add(center_calibration_slider(
                &mut right_y_center,
                "Right Stick Y-Axis Center",
            ))
            .changed()
        {
            calibration.set_right_y(right_y_center);
        }
    });
}

fn stick_min_max_calibration_form(ui: &mut egui::Ui, calibration: &mut StickMinMaxCalibration) {
    let mut left_min_x = calibration.left_min_x();
    let mut left_max_x = calibration.left_max_x();
    let mut left_min_y = calibration.left_min_y();
    let mut left_max_y = calibration.left_max_y();
    let mut right_min_x = calibration.right_min_x();
    let mut right_max_x = calibration.right_max_x();
    let mut right_min_y = calibration.right_min_y();
    let mut right_max_y = calibration.right_max_y();
    ui.columns(2, |columns| {
        columns[0].label("Left Stick X-Axis");
        columns[1].label("");
        if columns[0]
            .add(min_calibration_slider(&mut left_min_x, "Min"))
            .changed()
        {
            calibration.set_left_min_x(left_min_x);
        }
        if columns[1]
            .add(max_calibration_slider(&mut left_max_x, "Max"))
            .changed()
        {
            calibration.set_left_max_x(left_max_x);
        }
        columns[0].label("Left Stick Y-Axis");
        columns[1].label("");
        if columns[0]
            .add(min_calibration_slider(&mut left_min_y, "Min"))
            .changed()
        {
            calibration.set_left_min_y(left_min_y);
        }
        if columns[1]
            .add(max_calibration_slider(&mut left_max_y, "Max"))
            .changed()
        {
            calibration.set_left_min_y(left_max_y);
        }

        columns[0].label("Right Stick X-Axis");
        columns[1].label("");
        if columns[0]
            .add(min_calibration_slider(&mut right_min_x, "Min"))
            .changed()
        {
            calibration.set_right_min_x(right_min_x);
        }
        if columns[1]
            .add(max_calibration_slider(&mut right_max_x, "Max"))
            .changed()
        {
            calibration.set_right_max_x(right_max_x);
        }
        columns[0].label("Right Stick Y-Axis");
        columns[1].label("");
        if columns[0]
            .add(min_calibration_slider(&mut right_min_y, "Min"))
            .changed()
        {
            calibration.set_right_min_y(right_min_y);
        }
        if columns[1]
            .add(max_calibration_slider(&mut right_max_y, "Max"))
            .changed()
        {
            calibration.set_right_min_y(right_max_y);
        }
    });
}

fn stick_center_calibration(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    state: &mut DeviceConnected,
    sh: StatusHandler,
) {
    ui.heading("Analog Sticks Center Calibration");
    ui.label("Don't touch the analog sticks and press the Add Sample key to add sample, ");
    ui.label("or press Finish to save calibration results.");
    let ConnectedDevice::DualShock4(_, ds4) = &state.device;
    let ds4_data = sh.handle_error(ds4.read_last_data()).flatten();
    if let Some(ds4_data) = &ds4_data {
        ui.columns(2, |columns| {
            let stick_position = ds4_data.left_stick_position();
            columns[0].add(stick_preview_plot(
                "Left Stick Preview",
                stick_position,
                0f64,
                0f64,
            ));
            let stick_position = ds4_data.right_stick_position();
            columns[1].add(stick_preview_plot(
                "Right Stick Preview",
                stick_position,
                0f64,
                0f64,
            ));
            ctx.request_repaint();
        });
    }
    if ui.add(button_triangle("Add Sample")).clicked()
        || ds4_data.as_ref().map(|d| d.triangle()).unwrap_or(false)
    {
        {
            let _ = sh.handle_error(ds4.set_calibration_command(CalibrationType::Measure(
                CalibrationDeviceType::AnalogStick(AnalogStickCalibrationType::Center),
            )));
        }
        update_calibration_wizard_panel(state, sh.clone());
    }
    if ui.add(button_cross("Finish")).clicked() || ds4_data.map(|d| d.cross()).unwrap_or(false) {
        {
            let ConnectedDevice::DualShock4(_, ds4) = &state.device;
            let _ = sh.handle_error(ds4.set_calibration_command(CalibrationType::Stop(
                CalibrationDeviceType::AnalogStick(AnalogStickCalibrationType::Center),
            )));
        }
        update_calibration_wizard_panel(state, sh.clone());
    }
}

fn stick_min_max_calibration(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    state: &mut DeviceConnected,
    sh: StatusHandler,
) {
    ui.heading("Analog Sticks Min/Max Range Calibration");
    ui.label("Move analog sticks all around their range and press finish.");
    let ConnectedDevice::DualShock4(_, ds4) = &state.device;
    let ds4_data = sh.handle_error(ds4.read_last_data()).flatten();
    if let Some(ds4_data) = &ds4_data {
        ui.columns(2, |columns| {
            let stick_position = ds4_data.left_stick_position();
            columns[0].vertical_centered(|ui| {
                ui.label(with_gamepad_font(GAMEPAD_FONT_LEFT_ANALOG_CLOCKWISE).size(96f32));
            });
            columns[0].add(stick_preview_plot(
                "Left Stick Preview",
                stick_position,
                0f64,
                0f64,
            ));
            let stick_position = ds4_data.right_stick_position();
            columns[1].vertical_centered(|ui| {
                ui.label(with_gamepad_font(GAMEPAD_FONT_RIGHT_ANALOG_CLOCKWISE).size(96f32));
            });
            columns[1].add(stick_preview_plot(
                "Right Stick Preview",
                stick_position,
                0f64,
                0f64,
            ));
            ctx.request_repaint();
        });
    }
    if ui.add(button_cross("Finish")).clicked() || ds4_data.map(|d| d.cross()).unwrap_or(false) {
        {
            let _ = sh.handle_error(ds4.set_calibration_command(CalibrationType::Stop(
                CalibrationDeviceType::AnalogStick(AnalogStickCalibrationType::MinMax),
            )));
        }
        update_calibration_wizard_panel(state, sh.clone());
    }
}

fn motion_calibration_value_form(
    ui: &mut egui::Ui,
    calibration: &mut MotionCalibration,
    sh: StatusHandler,
) {
    let mut value_string = hex::encode(calibration.buf);
    ui.spacing_mut().text_edit_width = 600f32;
    if ui
        .add(egui::TextEdit::singleline(&mut value_string))
        .changed()
    {
        let max_len = calibration.buf.len() * 2;
        value_string.truncate(max_len);
        value_string = format!("{:0<width$}", value_string, width = max_len);
        sh.handle_error(hex::decode_to_slice(value_string, &mut calibration.buf));
    }
}

fn motion_calibration(ui: &mut egui::Ui, state: &mut DeviceConnected, sh: StatusHandler) {
    ui.heading("Motion Sensor Calibration Value");

    if let super::Panel::Calibration(Panel::MotionSensor(calibration)) = &mut state.panel {
        motion_calibration_value_form(ui, calibration, sh.clone());
    }

    if ui.button("Read from Device").clicked() {
        let ConnectedDevice::DualShock4(_, ds4) = &state.device;
        if let Some(calibration_from_device) = sh.handle_error(ds4.read_motion_calibration_data()) {
            state.panel = super::Panel::Calibration(Panel::MotionSensor(calibration_from_device));
        }
    }
    if ui.button("Write to Device").clicked() {
        if let super::Panel::Calibration(Panel::MotionSensor(calibration)) = &state.panel {
            let ConnectedDevice::DualShock4(_, ds4) = &state.device;
            let _ = sh.handle_error(ds4.set_motion_calibration_data(calibration));
        }
    }
}

fn triggers_calibration(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    state: &mut DeviceConnected,
    type_: TriggerKeyCalibrationType,
    sh: StatusHandler,
) {
    let ConnectedDevice::DualShock4(_, ds4) = &state.device;
    ui.heading("Triggers Calibration");

    let next_step = match type_ {
        TriggerKeyCalibrationType::RecordMaxSample(lr) => {
            ui.label(format!(
                "Press {} trigger(s) and click Next to continue",
                lr
            ));
            TriggerKeyCalibrationType::RecordRangeSample(lr)
        }
        TriggerKeyCalibrationType::RecordRangeSample(lr) => {
            ui.label(format!(
                "Press several times {} trigger(s) all range and click Next to continue",
                lr
            ));
            TriggerKeyCalibrationType::RecordMinSample(lr)
        }
        TriggerKeyCalibrationType::RecordMinSample(lr) => {
            ui.label(format!("Release {} trigger(s) and click Finish", lr));
            TriggerKeyCalibrationType::Unknown(lr)
        }
        TriggerKeyCalibrationType::Unknown(lr) => {
            if let Some(_) = sh.handle_error(ds4.set_calibration_command(CalibrationType::Measure(
                CalibrationDeviceType::TriggerKey(TriggerKeyCalibrationType::RecordMaxSample(
                    lr.clone(),
                )),
            ))) {
                state.panel = super::Panel::Calibration(Panel::Wizard(
                    CalibrationWizard::TriggerKey(TriggerKeyCalibrationType::RecordMaxSample(lr)),
                ))
            }
            return;
        }
        TriggerKeyCalibrationType::None => TriggerKeyCalibrationType::None,
    };

    let data = sh
        .handle_error(ds4.read_last_data())
        .flatten()
        .unwrap_or(Data::zeroed());
    ui.columns(2, |columns| {
        columns[0].add(trigger_bar(data.l2_trigger(), "Left Trigger"));
        columns[1].add(trigger_bar(data.r2_trigger(), "Right Trigger"));
    });

    if let TriggerKeyCalibrationType::Unknown(lr) = next_step {
        if ui.add(button_triangle("Add Sample")).clicked() || data.triangle() {
            state.panel = super::Panel::Calibration(Panel::Wizard(CalibrationWizard::TriggerKey(
                TriggerKeyCalibrationType::Unknown(lr.clone()),
            )))
        }
        if ui.add(button_cross("Finish")).clicked() || data.cross() {
            if let Some(_) = sh.handle_error(ds4.set_calibration_command(CalibrationType::Stop(
                CalibrationDeviceType::TriggerKey(TriggerKeyCalibrationType::Unknown(lr)),
            ))) {
                update_calibration_wizard_panel(state, sh);
            }
        }
    } else {
        if ui.button("Next").clicked() {
            if let Some(_) = sh.handle_error(ds4.set_calibration_command(CalibrationType::Measure(
                CalibrationDeviceType::TriggerKey(next_step.clone()),
            ))) {
                state.panel = super::Panel::Calibration(Panel::Wizard(
                    CalibrationWizard::TriggerKey(next_step),
                ))
            }
        }
    }

    ctx.request_repaint();
}

fn stick_preview_plot<'a>(
    title: &'a str,
    stick_position: StickPosition,
    normalized_x_adjustment: f64,
    normalized_y_adjustment: f64,
) -> impl egui::Widget + 'a {
    move |ui: &mut egui::Ui| {
        ui.vertical_centered(|ui| {
            ui.label(title);
            egui_plot::Plot::new(title)
                .view_aspect(1f32)
                .include_x(-1.1f64)
                .include_x(1.1f64)
                .include_y(-1.1f64)
                .include_y(1.1f64)
                .allow_zoom(false)
                .allow_drag(false)
                .allow_scroll(false)
                .show(ui, |plot_ui| {
                    plot_ui.line(circle_line(0f64, 0f64, 1f64).color(Color32::GRAY));
                    let (x, y) = (stick_position.normalized_x(), stick_position.normalized_y());
                    let points = Points::new([x, y]).radius(3f32).color(Color32::RED);
                    plot_ui.points(points);
                    let (x, y) = (x + normalized_x_adjustment, y + normalized_y_adjustment);
                    let points = Points::new([x, y]).radius(3f32).color(Color32::GREEN);
                    plot_ui.points(points);
                })
                .response
        })
            .response
    }
}

fn center_calibration_slider<'a>(value: &'a mut i16, text: &'a str) -> egui::Slider<'a> {
    egui::Slider::new(value, -512i16..=512i16)
        .clamping(SliderClamping::Always)
        .text(text)
        .logarithmic(true)
        .step_by(1f64)
}

fn min_calibration_slider<'a>(value: &'a mut i16, text: &'a str) -> egui::Slider<'a> {
    egui::Slider::new(value, -4048i16..=0i16)
        .clamping(SliderClamping::Always)
        .text(text)
        .logarithmic(false)
        .step_by(1f64)
}

fn max_calibration_slider<'a>(value: &'a mut i16, text: &'a str) -> egui::Slider<'a> {
    egui::Slider::new(value, 0i16..=4048i16)
        .clamping(SliderClamping::Always)
        .text(text)
        .logarithmic(false)
        .step_by(1f64)
}

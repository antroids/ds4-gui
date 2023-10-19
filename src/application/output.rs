// Copyright 2023 Anton Kharuzhyi <publicantroids@gmail.com>
// SPDX-License-Identifier: GPL-3.0

use crate::application::font::with_gamepad_font;
use crate::application::font::*;
use crate::application::{ConnectedDevice, DeviceConnected, Panel, StatusHandler};
use crate::dual_shock_4::{DPadState, Data, StickPosition};
use eframe::egui;
use eframe::egui::plot::{Line, Plot, PlotPoints, Points};
use eframe::egui::{remap, Color32, RichText, WidgetText};
use std::f64::consts::{PI, TAU};
use std::i16;
use std::ops::Rem;

const STICK_HISTORY_DEGREES: usize = 360;
const STICK_HISTORY_SECTORS: usize = 36;
const STICK_HISTORY_SECTOR_DEGREE: usize = STICK_HISTORY_DEGREES / STICK_HISTORY_SECTORS;

const BUTTON_SIZE: f32 = 32f32;
const BUTTON_FONT_SIZE: f32 = 32f32;

#[derive(Default)]
pub struct Output {
    pub left_stick_history: StickHistory,
    pub right_stick_history: StickHistory,
}

#[derive(Debug)]
#[repr(transparent)]
pub struct StickHistory {
    max_distance: [f64; STICK_HISTORY_SECTORS],
}

impl Default for StickHistory {
    fn default() -> Self {
        Self {
            max_distance: [0f64; STICK_HISTORY_SECTORS],
        }
    }
}

impl StickHistory {
    pub fn update(&mut self, x: f64, y: f64) {
        let distance = (x.powi(2) + y.powi(2)).sqrt();
        let angle = ((y.atan2(x) + PI * 2f64).to_degrees() as usize).rem(STICK_HISTORY_DEGREES);
        let sector = angle / STICK_HISTORY_SECTOR_DEGREE;
        self.max_distance[sector] = self.max_distance[sector].max(distance);
    }

    pub fn clear(&mut self) {
        self.max_distance.fill(0f64);
    }

    pub fn to_points(&self) -> [(f64, f64); STICK_HISTORY_SECTORS] {
        let points: Vec<(f64, f64)> = self
            .max_distance
            .iter()
            .enumerate()
            .map(|(sector, distance)| {
                let angle = sector * STICK_HISTORY_SECTOR_DEGREE + STICK_HISTORY_SECTOR_DEGREE / 2;
                let angle_pi = (angle as f64).to_radians();
                let y = angle_pi.sin() * distance;
                let x = angle_pi.cos() * distance;
                (x, y)
            })
            .collect();
        points.try_into().unwrap()
    }
}

fn stick_plot<'a>(
    title: &'a str,
    stick_position: StickPosition,
    stick_history: &'a mut StickHistory,
) -> impl egui::Widget + 'a {
    move |ui: &mut egui::Ui| {
        ui.label(title);
        Plot::new(title)
            .view_aspect(1f32)
            .include_x(-1.1f64)
            .include_x(1.1f64)
            .include_y(-1.1f64)
            .include_y(1.1f64)
            .allow_zoom(false)
            .allow_drag(false)
            .allow_scroll(false)
            .show(ui, |plot_ui| {
                let (x, y) = (stick_position.normalized_x(), stick_position.normalized_y());
                let points = Points::new([x, y]).radius(3f32).color(Color32::RED);
                stick_history.update(x, y);
                plot_ui.line(circle_line(0f64, 0f64, 1f64).color(Color32::GRAY));
                plot_ui.points(stick_history_peaks(&stick_history).color(Color32::LIGHT_YELLOW));
                plot_ui.points(points);
            })
            .response
    }
}

pub fn circle_line(x: f64, y: f64, r: f64) -> Line {
    let n = 512;
    let circle_points: PlotPoints = (0..=n)
        .map(|i| {
            let t = remap(i as f64, 0.0..=(n as f64), 0.0..=TAU);
            [r * t.cos() + x, r * t.sin() + y]
        })
        .collect();
    Line::new(circle_points)
}

fn stick_history_peaks(stick_history: &StickHistory) -> Points {
    let points = stick_history.to_points();
    let plot_points: PlotPoints = points.into_iter().map(|(x, y)| [x, y]).collect();
    Points::new(plot_points)
}

pub fn output(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    state: &mut DeviceConnected,
    sh: StatusHandler,
) {
    let ConnectedDevice::DualShock4(_, ds4) = &state.device;
    let data = sh
        .handle_error(ds4.read_last_data())
        .flatten()
        .unwrap_or(Data::zeroed());

    ctx.request_repaint();
    if let Panel::Output(output) = &mut state.panel {
        ui.columns(2, |columns| {
            columns[0].add(stick_plot(
                "Left stick plot",
                data.left_stick_position(),
                &mut output.left_stick_history,
            ));
            columns[1].add(stick_plot(
                "Right stick plot",
                data.right_stick_position(),
                &mut output.right_stick_history,
            ));
            if columns[0].button("Clear history").clicked() {
                output.left_stick_history.clear();
            }
            if columns[1].button("Clear history").clicked() {
                output.right_stick_history.clear();
            }
        });
        ui.separator();
        ui.columns(2, |columns| {
            columns[0].add(trigger_bar(data.l2_trigger(), "Left Trigger"));
            columns[1].add(trigger_bar(data.r2_trigger(), "Right Trigger"));
        });
        ui.columns(3, |columns| {
            columns[0].horizontal(|ui| {
                ui.add(gamepad_button_label(data.l1(), GAMEPAD_FONT_L1));
                ui.add(gamepad_button_label(data.l2(), GAMEPAD_FONT_L2));
                ui.add(gamepad_button_label(data.l3(), GAMEPAD_FONT_LEFT_ANALOG));
            });
            columns[1].horizontal(|ui| {
                ui.add(gamepad_button_label(data.share(), GAMEPAD_FONT_SHARE));
                ui.add(gamepad_button_label(data.ps(), GAMEPAD_FONT_PS));
                ui.add(gamepad_button_label(data.t_pad_click(), GAMEPAD_FONT_T_PAD));
                ui.add(gamepad_button_label(data.options(), GAMEPAD_FONT_OPTIONS));
            });
            columns[2].horizontal(|ui| {
                ui.add(gamepad_button_label(data.r3(), GAMEPAD_FONT_RIGHT_ANALOG));
                ui.add(gamepad_button_label(data.r2(), GAMEPAD_FONT_R2));
                ui.add(gamepad_button_label(data.r1(), GAMEPAD_FONT_R1));
            });
            columns[0].add(d_pad_label(data.d_pad()));
            egui::Grid::new("Buttons")
                .num_columns(3)
                .max_col_width(BUTTON_SIZE)
                .min_col_width(BUTTON_SIZE)
                .min_row_height(BUTTON_SIZE)
                .show(&mut columns[2], |ui| {
                    ui.label("");
                    ui.add(gamepad_button_label(data.triangle(), GAMEPAD_FONT_TRIANGLE));
                    ui.end_row();
                    ui.add(gamepad_button_label(data.square(), GAMEPAD_FONT_SQUARE));
                    ui.label("");
                    ui.add(gamepad_button_label(data.circle(), GAMEPAD_FONT_CIRCLE));
                    ui.end_row();
                    ui.label("");
                    ui.add(gamepad_button_label(data.cross(), GAMEPAD_FONT_CROSS));
                });
        });
        ui.columns(3, |columns| {
            columns[0].add(gyroscope_accelerometer_bar(
                data.gyroscope_x(),
                "Gyroscope X",
            ));
            columns[1].add(gyroscope_accelerometer_bar(
                data.gyroscope_y(),
                "Gyroscope Y",
            ));
            columns[2].add(gyroscope_accelerometer_bar(
                data.gyroscope_z(),
                "Gyroscope Z",
            ));
            columns[0].add(gyroscope_accelerometer_bar(
                data.accelerometer_x(),
                "Accelerometer X",
            ));
            columns[1].add(gyroscope_accelerometer_bar(
                data.accelerometer_y(),
                "Accelerometer Y",
            ));
            columns[2].add(gyroscope_accelerometer_bar(
                data.accelerometer_z(),
                "Accelerometer Z",
            ));
        });
        ui.horizontal(|ui| {
            ui.label(format!("Battery: {}", data.battery()));
            ui.label(format!("Counter: {}", data.counter()));
            ui.label(format!("Timestamp: {}", data.timestamp()));
        });
    } else {
        ui.label("Unsupported device");
    }
}

pub fn d_pad_label<'a>(state: DPadState) -> impl egui::Widget + 'a {
    move |ui: &mut egui::Ui| {
        if state == DPadState::Released {
            ui.label("")
        } else {
            ui.colored_label(
                button_label_color(true),
                RichText::new(match state {
                    DPadState::UpLeft => "↖",
                    DPadState::Left => "⬅",
                    DPadState::DownLeft => "↙",
                    DPadState::Down => "⬇",
                    DPadState::DownRight => "↘",
                    DPadState::Right => "➡",
                    DPadState::UpRight => "↗",
                    DPadState::Up => "⬆",
                    DPadState::Released => unreachable!(),
                })
                .size(80f32),
            )
        }
    }
}

pub fn gyroscope_accelerometer_bar(value: i16, text: impl Into<WidgetText>) -> impl egui::Widget {
    move |ui: &mut egui::Ui| {
        ui.add(
            egui::ProgressBar::new(value as f32 / i16::MAX as f32 + 0.5f32)
                .show_percentage()
                .text(text),
        )
    }
}

pub fn trigger_bar(value: u8, text: impl Into<WidgetText>) -> impl egui::Widget {
    move |ui: &mut egui::Ui| {
        ui.add(
            egui::ProgressBar::new(value as f32 / 255f32)
                .show_percentage()
                .text(text),
        )
    }
}

pub fn gamepad_button_label<'a>(pressed: bool, text: &'a str) -> impl egui::Widget + 'a {
    move |ui: &mut egui::Ui| {
        ui.colored_label(
            button_label_color(pressed),
            with_gamepad_font(text).size(BUTTON_FONT_SIZE),
        )
    }
}

fn button_label_color(pressed: bool) -> Color32 {
    if pressed {
        Color32::GREEN
    } else {
        Color32::GRAY
    }
}

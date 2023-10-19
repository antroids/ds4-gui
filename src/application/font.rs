// Copyright 2023 Anton Kharuzhyi <publicantroids@gmail.com>
// SPDX-License-Identifier: GPL-3.0

use eframe::egui;
use eframe::egui::{FontFamily, RichText, WidgetText};

pub const GAMEPAD_FONT_SYMBOL: &str = "\u{243C}";
pub const GAMEPAD_FONT_TRIANGLE: &str = "\u{21E1}";
pub const GAMEPAD_FONT_CIRCLE: &str = "\u{21E2}";
pub const GAMEPAD_FONT_CROSS: &str = "\u{21E3}";
pub const GAMEPAD_FONT_SQUARE: &str = "\u{21E0}";
pub const GAMEPAD_FONT_LEFT_ANALOG: &str = "\u{21CB}";
pub const GAMEPAD_FONT_RIGHT_ANALOG: &str = "\u{21CC}";
pub const GAMEPAD_FONT_L1: &str = "\u{21B0}";
pub const GAMEPAD_FONT_R1: &str = "\u{21B1}";
pub const GAMEPAD_FONT_L2: &str = "\u{21B2}";
pub const GAMEPAD_FONT_R2: &str = "\u{21B3}";
pub const GAMEPAD_FONT_OPTIONS: &str = "\u{21E8}";
pub const GAMEPAD_FONT_SHARE: &str = "\u{21E6}";
pub const GAMEPAD_FONT_PS: &str = "\u{E000}";
pub const GAMEPAD_FONT_T_PAD: &str = "\u{21E7}";
pub const GAMEPAD_FONT_RIGHT_ANALOG_CLOCKWISE: &str = "\u{21AB}";
pub const GAMEPAD_FONT_LEFT_ANALOG_CLOCKWISE: &str = "\u{21A9}";
pub const GAMEPAD_FONT_BOTH_ANALOG_CLOCKWISE: &str = "\u{21AD}";

pub const GAMEPAD_FONT_FAMILY: &str = "GamepadFont";

pub fn with_gamepad_font(text: &str) -> RichText {
    RichText::from(text).family(FontFamily::Name(GAMEPAD_FONT_FAMILY.into()))
}

pub fn button_cross(text: impl Into<WidgetText>) -> egui::widgets::Button {
    egui::widgets::Button::new(text).shortcut_text(with_gamepad_font(GAMEPAD_FONT_CROSS))
}

pub fn button_triangle(text: impl Into<WidgetText>) -> egui::widgets::Button {
    egui::widgets::Button::new(text).shortcut_text(with_gamepad_font(GAMEPAD_FONT_TRIANGLE))
}

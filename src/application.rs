use crate::application::calibration::calibration;
use crate::application::device_info::device_info;
use crate::application::flash::{flash, Flash};
use crate::application::font::{with_gamepad_font, GAMEPAD_FONT_SYMBOL};
use crate::application::output::{output, Output};
use crate::application::test_commands::test_commands;
use crate::dual_shock_4::DualShock4;
use device_info::DeviceInfo;
use eframe::egui::panel::{Side, TopBottomSide};
use eframe::egui::{Color32, Context, FontFamily, Response, RichText, ScrollArea};
use eframe::{egui, Frame};
use font::GAMEPAD_FONT_FAMILY;
use hidapi::{HidApi, HidError};
use log::{error, info};
use std::ffi::CString;
use std::fmt::{Display, Formatter};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::Duration;

mod calibration;
mod device_info;
mod flash;
mod output;
mod test_commands;

mod font;

const UNDEFINED_STRING: &str = "undefined";

#[derive(Debug)]
pub enum Error {
    DualShock4Error(crate::dual_shock_4::Error),
    HidError(HidError),
    EframeError(eframe::Error),
}

impl std::error::Error for Error {}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl From<HidError> for Error {
    fn from(value: HidError) -> Self {
        Self::HidError(value)
    }
}

impl From<crate::dual_shock_4::Error> for Error {
    fn from(value: crate::dual_shock_4::Error) -> Self {
        Self::DualShock4Error(value)
    }
}

impl From<eframe::Error> for Error {
    fn from(value: eframe::Error) -> Self {
        Self::EframeError(value)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

pub enum Status {
    Ok,
    Error(Box<dyn std::error::Error>),
    Message(String),
}

pub struct Application {
    api: HidApi,
    devices: Vec<Device>,
    ui_state: UIState,
    status_receiver: Receiver<Status>,
    status_handler: StatusHandler,
    last_status: Status,
}

enum UIState {
    DeviceNotConnected,
    DeviceConnected(DeviceConnected),
}

pub struct DeviceConnected {
    device: ConnectedDevice,
    panel: Panel,
    permanent: bool,
}

impl ConnectedDevice {
    pub fn path(&self) -> &CString {
        match self {
            ConnectedDevice::DualShock4(device, _) => device.path(),
        }
    }
}

#[derive(Debug, PartialEq, PartialOrd, Clone)]
pub enum Device {
    DualShock4(CString),
}

impl Device {
    pub fn path(&self) -> &CString {
        match self {
            Device::DualShock4(path) => path,
        }
    }
}

pub enum ConnectedDevice {
    DualShock4(Device, DualShock4),
}

impl ConnectedDevice {
    fn device(&self) -> &Device {
        match self {
            ConnectedDevice::DualShock4(device, _) => &device,
        }
    }
}

enum Panel {
    DeviceInfo(DeviceInfo),
    Output(Output),
    Calibration(calibration::Panel),
    Flash(Flash),
    Test,
}

#[derive(Clone)]
pub struct StatusHandler {
    status_sender: Sender<Status>,
}

impl StatusHandler {
    pub fn new(status_sender: Sender<Status>) -> Self {
        Self { status_sender }
    }

    fn handle_error<'a, T, E: std::error::Error + 'static>(
        &self,
        result: std::result::Result<T, E>,
    ) -> Option<T> {
        match result {
            Ok(result) => {
                let _ = self.status_sender.send(Status::Ok);
                Some(result)
            }
            Err(error) => {
                self.error(Box::new(error));
                None
            }
        }
    }

    fn message(&self, message: impl Into<String>) {
        let string: String = message.into();
        info!("Message: {:?}", string);
        let _ = self.status_sender.send(Status::Message(string));
    }

    fn error(&self, error: Box<dyn std::error::Error>) {
        error!("{:?}", error);
        let _ = self.status_sender.send(Status::Error(error));
    }
}

impl eframe::App for Application {
    fn update(&mut self, ctx: &Context, frame: &mut Frame) {
        self.update_ui(ctx, frame);
    }
}

impl Application {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Result<Self> {
        Self::setup_assets(cc);

        let api = HidApi::new().map_err(Error::HidError)?;

        let devices = Vec::new();
        let (status_sender, status_receiver) = channel();
        let status_handler = StatusHandler::new(status_sender);
        let ui_state = UIState::DeviceNotConnected;
        let last_status = Status::Ok;

        let mut self_ = Self {
            api,
            devices,
            ui_state,
            status_receiver,
            status_handler,
            last_status,
        };

        Self::refresh_devices(&mut self_)?;
        Ok(self_)
    }

    pub fn show() -> Result<()> {
        let options = eframe::NativeOptions {
            initial_window_size: Some(egui::vec2(800.0, 800.0)),
            ..Default::default()
        };

        let _ = eframe::run_native(
            "DS4 Utils",
            options,
            Box::new(|cc| Box::new(Application::new(cc).unwrap())),
        )?;
        Ok(())
    }

    fn setup_assets(cc: &eframe::CreationContext<'_>) {
        let mut fonts = egui::FontDefinitions::default();
        fonts.font_data.insert(
            GAMEPAD_FONT_FAMILY.to_owned(),
            egui::FontData::from_static(include_bytes!("../assets/fonts/PromptFont.otf")),
        );
        fonts
            .families
            .entry(FontFamily::Name(GAMEPAD_FONT_FAMILY.into()))
            .or_default()
            .insert(0, GAMEPAD_FONT_FAMILY.into());
        cc.egui_ctx.set_fonts(fonts);
    }

    fn refresh_devices(&mut self) -> Result<()> {
        self.api.refresh_devices().map_err(Error::HidError)?;
        let devices: Vec<Device> = self
            .api
            .device_list()
            .filter(|device| is_dual_shock_4(device.vendor_id(), device.product_id()))
            .map(|device| Device::DualShock4(CString::from(device.path())))
            .collect();
        let contains_current_device = if let UIState::DeviceConnected(state) = &mut self.ui_state {
            let current = state.device.device().clone();
            if devices.contains(&current) {
                true
            } else {
                false
            }
        } else {
            false
        };
        if !contains_current_device {
            self.ui_state = UIState::DeviceNotConnected;
        }
        self.devices = devices;
        Ok(())
    }

    fn device(&self) -> Option<&Device> {
        match &self.ui_state {
            UIState::DeviceNotConnected => None,
            UIState::DeviceConnected(s) => Some(s.device.device()),
        }
    }

    fn update_device(&mut self, device: Option<&Device>) {
        if device == self.device() {
            return;
        }
        let sh = self.status_handler.clone();
        self.ui_state = match device {
            None => UIState::DeviceNotConnected,
            Some(device) => match device {
                Device::DualShock4(path) => {
                    let hid_device = self.api.open_path(path.as_ref());
                    if let Some(hid_device) = sh.handle_error(hid_device) {
                        let connected_device = ConnectedDevice::DualShock4(
                            device.clone(),
                            DualShock4::new(path.clone(), hid_device),
                        );
                        if let Some(device_info) =
                            sh.handle_error(DeviceInfo::from_connected_device(&connected_device))
                        {
                            let ConnectedDevice::DualShock4(_, ds4) = &connected_device;
                            let permanent = ds4.read_permanent().unwrap_or(false);
                            UIState::DeviceConnected(DeviceConnected {
                                device: connected_device,
                                panel: Panel::DeviceInfo(device_info),
                                permanent,
                            })
                        } else {
                            UIState::DeviceNotConnected
                        }
                    } else {
                        UIState::DeviceNotConnected
                    }
                }
            },
        };
    }

    fn update_ui(&mut self, ctx: &Context, _frame: &mut Frame) {
        self.show_status_bar(ctx);
        self.show_devices(ctx);
        self.show_content(ctx);
    }

    fn show_status_bar(&mut self, ctx: &Context) {
        if let Ok(status) = self.status_receiver.try_recv() {
            self.last_status = status;
            ctx.request_repaint();
        }
        egui::TopBottomPanel::new(TopBottomSide::Bottom, "Status")
            .exact_height(32.0)
            .show(ctx, |ui| match &self.last_status {
                Status::Ok => {
                    ui.label(RichText::new("⬤ Ok").color(Color32::GREEN));
                }
                Status::Error(error) => {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("⬤ {}", error)).color(Color32::RED));
                    });
                }
                Status::Message(message) => {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("⬤ {}", message)).color(Color32::GREEN));
                    });
                }
            });
    }

    fn show_devices(&mut self, ctx: &Context) {
        let sh = self.status_handler.clone();
        let _ = sh.handle_error(self.refresh_devices());
        egui::SidePanel::new(Side::Left, "List").show(ctx, |ui| {
            ScrollArea::vertical().show(ui, |ui| {
                let mut current = self.device().cloned();
                for device in &self.devices {
                    ui.horizontal(|ui| {
                        ui.label(with_gamepad_font(GAMEPAD_FONT_SYMBOL));
                        ui.selectable_value(
                            &mut current,
                            Some(device.clone()),
                            device.path().to_str().unwrap(),
                        );
                    });
                }
                self.update_device(current.as_ref());
            });
        });
        ctx.request_repaint_after(Duration::from_secs(1));
    }

    fn show_content(&mut self, ctx: &Context) {
        let sh = self.status_handler.clone();

        egui::CentralPanel::default().show(ctx, |ui| {
            global_styles(ui);
            if let UIState::DeviceConnected(state) = &mut self.ui_state {
                Self::show_panel_selector(ui, state, sh.clone());
                ui.separator();
                Self::show_panel(ui, ctx, state, sh.clone());
            } else {
                ui.label("Please, select controller from the list");
            }
        });
    }

    fn show_panel_selector(ui: &mut egui::Ui, state: &mut DeviceConnected, sh: StatusHandler) {
        ui.horizontal(|ui| {
            if panel_switch_button(
                ui,
                matches!(&state.panel, Panel::DeviceInfo(_)),
                "Device Info",
            )
            .clicked()
            {
                if let Some(device_info) =
                    sh.handle_error(DeviceInfo::from_connected_device(&state.device))
                {
                    state.panel = Panel::DeviceInfo(device_info);
                }
            }
            if panel_switch_button(ui, matches!(&state.panel, Panel::Output(_)), "Output").clicked()
            {
                state.panel = Panel::Output(Output::default());
            }
            if panel_switch_button(
                ui,
                matches!(&state.panel, Panel::Calibration(_)),
                "Calibration",
            )
            .clicked()
            {
                if let Some(panel) =
                    calibration::Panel::info_from_device_connected(state, sh.clone())
                {
                    state.panel = Panel::Calibration(panel);
                }
            }
            if panel_switch_button(ui, matches!(&state.panel, Panel::Flash(_)), "Flash").clicked() {
                state.panel = Panel::Flash(Flash::default());
            }
            if panel_switch_button(ui, matches!(&state.panel, Panel::Test), "Test Commands")
                .clicked()
            {
                state.panel = Panel::Test;
            }
        });
    }

    fn show_panel(
        ui: &mut egui::Ui,
        ctx: &Context,
        state: &mut DeviceConnected,
        sh: StatusHandler,
    ) {
        match &state.panel {
            Panel::DeviceInfo(info) => device_info(ui, info),
            Panel::Output(_) => output(ui, ctx, state, sh.clone()),
            Panel::Calibration(_) => calibration(ui, ctx, state, sh.clone()),
            Panel::Flash(_) => flash(ui, ctx, state, sh.clone()),
            Panel::Test => test_commands(ui, ctx, state, sh.clone()),
            _ => {
                ui.label("Unknown panel");
            }
        };
    }
}

fn global_styles(ui: &mut egui::Ui) {
    ui.style_mut().spacing.slider_width = 150f32;
}

fn panel_switch_button(ui: &mut egui::Ui, selected: bool, text: &str) -> Response {
    ui.add(egui::SelectableLabel::new(selected, text))
}

fn is_dual_shock_4(vendor_id: u16, product_id: u16) -> bool {
    vendor_id == 0x054c && (product_id == 0x05c4 || product_id == 0x09cc)
}

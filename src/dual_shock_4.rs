use crate::dual_shock_4::hid_report::{Report, ReportId};
use hidapi::{HidDevice, HidError};
use log::info;
use std::ffi::CString;
use std::fmt::{Debug, Display, Formatter};
use std::mem;
use std::ops::{Not, Range};

mod hid_report;

const DATA_PACKET_SIZE: usize = 64;
const MOTION_CALIBRATION_DATA_SIZE: usize = 40;
const CALIBRATION_FLAG_SIZE: usize = 4;
const CALIBRATION_STATE_SIZE: usize = 3;
const CALIBRATION_RESULT_SIZE: usize = 3;
const CALIBRATION_DATA_SIZE: usize = 13;

pub const TRIGGER_MIN_MAX_CALIBRATION_SAMPLES: u16 = 0x04E2;

const STICK_CENTER: f64 = 127.5f64;
const STICK_NORMALIZED_INTERVAL: f64 = 2f64;
const STICK_NORMALIZED_CENTER: f64 = STICK_NORMALIZED_INTERVAL / 2f64;

pub const STICK_CALIBRATION_RANGE: u16 = 0xfff;
pub const STICK_CALIBRATION_HALF_RANGE: u16 = STICK_CALIBRATION_RANGE / 2;

const STICK_HISTORY_DEGREES: usize = 360;
const STICK_HISTORY_SECTORS: usize = 36;
const STICK_HISTORY_SECTOR_DEGREE: usize = STICK_HISTORY_DEGREES / STICK_HISTORY_SECTORS;

pub const FLASH_MIRROR_SIZE: usize = 0x800;

#[derive(Debug)]
pub enum Error {
    HidError(HidError),
    OutOfRange(i64, Range<i64>),
    InvalidReport,
    ErrorMessage(String),
}

pub type Result<T> = std::result::Result<T, Error>;

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::error::Error for Error {}

impl From<HidError> for Error {
    fn from(value: HidError) -> Self {
        Error::HidError(value)
    }
}

impl From<String> for Error {
    fn from(value: String) -> Self {
        Self::ErrorMessage(value)
    }
}

pub struct DualShock4 {
    hid_device: HidDevice,
    path: CString,
}

impl DualShock4 {
    pub fn new(path: CString, hid_device: HidDevice) -> Self {
        Self { hid_device, path }
    }

    pub fn read_last_data(&self) -> Result<Option<Data>> {
        let mut last_filled = Data::zeroed();
        let mut last_read = Data::zeroed();

        self.hid_device.read(&mut last_read.buf)?;
        for _ in 0..16 {
            if last_read.buf[0] == 0u8
                || (last_filled.buf[0] != 0u8 && last_read.counter() == last_filled.counter())
            {
                break;
            }
            mem::swap(&mut last_read.buf, &mut last_filled.buf);
            self.hid_device.read(&mut last_read.buf)?;
        }

        if last_filled.buf[0] != 0u8 {
            Ok(Some(last_filled))
        } else {
            Ok(None)
        }
    }

    pub fn read_motion_calibration_data(&self) -> Result<MotionCalibration> {
        let report = self.get_report(ReportId::GetMotionCalibData, MOTION_CALIBRATION_DATA_SIZE)?;
        let mut data = MotionCalibration::default();
        data.buf.copy_from_slice(report.payload());
        Ok(data)
    }

    pub fn set_motion_calibration_data(&self, calibration: &MotionCalibration) -> Result<()> {
        let report = Report::from_payload(ReportId::SetMotionCalibData, &calibration.buf);
        self.send_report(report)
    }

    pub fn read_calibration_flag(&self) -> Result<CalibrationFlag> {
        let report = self.get_report(ReportId::GetCalibFlag, CALIBRATION_FLAG_SIZE)?;
        let mut state = CalibrationFlag::default();
        state.buf.copy_from_slice(report.payload());
        Ok(state)
    }

    pub fn read_calibration_state(&self) -> Result<CalibrationState> {
        let report = self.get_report(ReportId::GetCalibrationState, CALIBRATION_STATE_SIZE)?;
        let mut buf = [0u8; 3];
        buf.copy_from_slice(report.payload());
        Ok(buf.try_into()?)
    }

    pub fn read_calibration_result(&self) -> Result<CalibrationResult> {
        let report = self.get_report(ReportId::GetCalibrationResult, CALIBRATION_RESULT_SIZE)?;
        let mut buf = [0u8; 3];
        buf.copy_from_slice(report.payload());
        Ok(buf.try_into()?)
    }

    pub fn set_calibration_command(&self, command: CalibrationType) -> Result<()> {
        let payload: [u8; 5] = command.into();
        let report = Report::from_payload(ReportId::SetCalibrationCommand, payload.as_slice());
        self.send_report(report)
    }

    pub fn set_test_command(&self, command: TestCommand) -> Result<()> {
        self.send_report(command.into())
    }

    pub fn read_calibration_data(&self) -> Result<CalibrationData> {
        let mut data: Vec<u8> = Vec::new();
        let mut last_device = CalibrationDeviceType::None;

        loop {
            let report = self.get_report(ReportId::GetCalibrationData, CALIBRATION_DATA_SIZE)?;
            let payload = report.payload();
            let chunks = payload[2];
            let current_chunk = payload[3];
            let data_len = payload[4];
            let device = [payload[0], payload[1], 0x00, 0x00].try_into()?;

            if device == CalibrationDeviceType::None || current_chunk > chunks - 1 {
                break;
            } else if last_device != CalibrationDeviceType::None && last_device != device {
                return Err(
                    format!("Mismatch Device Type: {:?}  {:?}", last_device, device).into(),
                );
            }
            if data_len > 8 {
                return Err(format!("Invalid Calibration Data chunk len {}", data_len).into());
            }
            data.extend_from_slice(&payload[5usize..(5 + data_len) as usize]);
            last_device = device;
        }

        Ok(match last_device {
            CalibrationDeviceType::AnalogStick(AnalogStickCalibrationType::Center) => {
                let mut calculated = StickCenterCalibration::default();
                let mut samples: Vec<StickCenterCalibration> = Vec::new();
                calculated.buf.copy_from_slice(&data.as_slice()[0..8]);
                if let Some(samples_count) = data.as_slice().get(8) {
                    for i in 0..(*samples_count as usize) {
                        let mut sample = StickCenterCalibration::default();
                        sample
                            .buf
                            .copy_from_slice(&data.as_slice()[(8 * i + 9)..(8 * i + 9 + 8)]);
                        samples.push(sample);
                    }
                }
                CalibrationData::StickCenter(calculated, samples)
            }
            CalibrationDeviceType::AnalogStick(AnalogStickCalibrationType::MinMax) => {
                let mut calibration = StickMinMaxCalibration::default();
                calibration.buf.copy_from_slice(&data.as_slice()[0..16]);
                CalibrationData::StickMinMax(calibration)
            }
            CalibrationDeviceType::TriggerKey(_) => {
                let mut calibration = TriggersCalibration::default();
                calibration.buf.append(&mut data);
                CalibrationData::Triggers(calibration)
            }
            _ => {
                info!("Calibration data slice: {:?}", data);
                todo!()
            }
        })
    }

    pub fn get_ieep_data(&self) -> Result<[u8; 2]> {
        let report = self.get_report(ReportId::GetIeepData, 2)?;
        let payload = report.payload();
        Ok([payload[0], payload[1]])
    }

    pub fn read_flash_mirror(&self) -> Result<FlashMirror> {
        let mut bytes: Vec<u8> = Vec::with_capacity(FLASH_MIRROR_SIZE);
        for offset in 0..(FLASH_MIRROR_SIZE / 2) as u16 {
            self.send_factory_command(FactoryCommand::SetIeepAddress(offset * 2))?;
            let two_bytes = self.get_ieep_data()?;
            bytes.push(two_bytes[0]);
            bytes.push(two_bytes[1]);
        }

        let mut flash_mirror = FlashMirror::default();
        flash_mirror.buf.copy_from_slice(bytes.as_slice());
        Ok(flash_mirror)
    }

    pub fn send_factory_command(&self, command: FactoryCommand) -> Result<()> {
        let payload: [u8; 3] = command.into();
        let report = Report::from_payload(ReportId::SetFactoryCommand, &payload);
        self.send_report(report)
    }

    pub fn read_permanent(&self) -> Result<bool> {
        self.send_factory_command(FactoryCommand::SetIeepAddress(12))?;
        let two_bytes = self.get_ieep_data()?;
        Ok(two_bytes[0] == 0)
    }

    fn send_report(&self, report: Report) -> Result<()> {
        self.hid_device.send_feature_report(report.data())?;
        info!("Report sent: {:?}", report);
        Ok(())
    }

    fn get_report(&self, id: ReportId, payload_size: usize) -> Result<Report> {
        let mut report = Report::new(id, payload_size);
        self.hid_device.get_feature_report(report.data_mut())?;
        info!("Report received: {:?}", report);
        if report.valid() {
            Ok(report)
        } else {
            Err(Error::InvalidReport)
        }
    }
    pub fn hid_device(&self) -> &HidDevice {
        &self.hid_device
    }
    pub fn path(&self) -> &CString {
        &self.path
    }
}

#[derive(Debug)]
#[repr(transparent)]
pub struct Data {
    pub buf: [u8; DATA_PACKET_SIZE],
}

impl Data {
    pub fn zeroed() -> Self {
        Self {
            buf: [0u8; DATA_PACKET_SIZE],
        }
    }
}

impl Data {
    pub fn left_stick_position(&self) -> StickPosition {
        StickPosition {
            x: self.buf[1],
            y: self.buf[2],
        }
    }

    pub fn right_stick_position(&self) -> StickPosition {
        StickPosition {
            x: self.buf[3],
            y: self.buf[4],
        }
    }

    pub fn triangle(&self) -> bool {
        self.buf[5] & 0b10000000 != 0
    }

    pub fn circle(&self) -> bool {
        self.buf[5] & 0b01000000 != 0
    }

    pub fn cross(&self) -> bool {
        self.buf[5] & 0b00100000 != 0
    }

    pub fn square(&self) -> bool {
        self.buf[5] & 0b00010000 != 0
    }

    pub fn d_pad(&self) -> DPadState {
        match self.buf[5] << 4 {
            0b01110000 => DPadState::UpLeft,
            0b01100000 => DPadState::Left,
            0b01010000 => DPadState::DownLeft,
            0b01000000 => DPadState::Down,
            0b00110000 => DPadState::DownRight,
            0b00100000 => DPadState::Right,
            0b00010000 => DPadState::UpRight,
            0b00000000 => DPadState::Up,
            _ => DPadState::Released,
        }
    }

    pub fn r3(&self) -> bool {
        self.buf[6] & 0b10000000 != 0
    }

    pub fn l3(&self) -> bool {
        self.buf[6] & 0b01000000 != 0
    }

    pub fn options(&self) -> bool {
        self.buf[6] & 0b00100000 != 0
    }

    pub fn share(&self) -> bool {
        self.buf[6] & 0b00010000 != 0
    }

    pub fn r2(&self) -> bool {
        self.buf[6] & 0b00001000 != 0
    }

    pub fn l2(&self) -> bool {
        self.buf[6] & 0b00000100 != 0
    }

    pub fn r1(&self) -> bool {
        self.buf[6] & 0b00000010 != 0
    }

    pub fn l1(&self) -> bool {
        self.buf[6] & 0b00000001 != 0
    }

    pub fn counter(&self) -> u8 {
        self.buf[7] >> 2
    }

    pub fn t_pad_click(&self) -> bool {
        self.buf[7] & 0b00000010 != 0
    }

    pub fn ps(&self) -> bool {
        self.buf[7] & 0b00000001 != 0
    }

    pub fn l2_trigger(&self) -> u8 {
        self.buf[8]
    }

    pub fn r2_trigger(&self) -> u8 {
        self.buf[9]
    }

    pub fn timestamp(&self) -> u16 {
        u16::from_le_bytes([self.buf[10], self.buf[11]])
    }

    pub fn battery(&self) -> u8 {
        self.buf[12]
    }

    pub fn gyroscope_x(&self) -> i16 {
        i16::from_le_bytes([self.buf[13], self.buf[14]])
    }

    pub fn gyroscope_y(&self) -> i16 {
        i16::from_le_bytes([self.buf[15], self.buf[16]])
    }

    pub fn gyroscope_z(&self) -> i16 {
        i16::from_le_bytes([self.buf[17], self.buf[18]])
    }

    pub fn accelerometer_x(&self) -> i16 {
        i16::from_le_bytes([self.buf[19], self.buf[20]])
    }

    pub fn accelerometer_y(&self) -> i16 {
        i16::from_le_bytes([self.buf[21], self.buf[22]])
    }

    pub fn accelerometer_z(&self) -> i16 {
        i16::from_le_bytes([self.buf[23], self.buf[24]])
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum DPadState {
    UpLeft,
    Left,
    DownLeft,
    Down,
    DownRight,
    Right,
    UpRight,
    Up,
    Released,
}

#[derive(Debug, Clone, PartialEq)]
#[repr(transparent)]
pub struct MotionCalibration {
    pub buf: [u8; MOTION_CALIBRATION_DATA_SIZE],
}

impl Default for MotionCalibration {
    fn default() -> Self {
        Self {
            buf: [0u8; MOTION_CALIBRATION_DATA_SIZE],
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
#[repr(transparent)]
// Todo: change representation to parsed values
pub struct StickCenterCalibration {
    pub buf: [u8; 8], // 0x0000..0x0fff
}

impl Default for StickCenterCalibration {
    fn default() -> Self {
        Self { buf: [0u8; 8] }
    }
}

impl StickCenterCalibration {
    fn get_value_at_index(&self, index: u8) -> i16 {
        let index = index * 2;
        let raw =
            u16::from_le_bytes([self.buf[index as usize], self.buf[(index + 1) as usize]]) as i16;
        raw - STICK_CALIBRATION_HALF_RANGE as i16
    }

    fn set_value_at_index(&mut self, index: u8, value: i16) {
        let index = index * 2;
        let raw = value + STICK_CALIBRATION_HALF_RANGE as i16;
        let bytes = (raw as u16).clamp(0, STICK_CALIBRATION_RANGE).to_le_bytes();
        self.buf[index as usize] = bytes[0];
        self.buf[(index + 1) as usize] = bytes[1];
    }

    pub fn left_x(&self) -> i16 {
        self.get_value_at_index(0)
    }

    pub fn left_y(&self) -> i16 {
        self.get_value_at_index(1)
    }

    pub fn right_x(&self) -> i16 {
        self.get_value_at_index(2)
    }

    pub fn right_y(&self) -> i16 {
        self.get_value_at_index(3)
    }

    pub fn normalized_left_x(&self) -> f64 {
        self.left_x() as f64 / STICK_CALIBRATION_HALF_RANGE as f64
    }

    pub fn normalized_left_y(&self) -> f64 {
        self.left_y() as f64 / STICK_CALIBRATION_HALF_RANGE as f64
    }

    pub fn normalized_right_x(&self) -> f64 {
        self.right_x() as f64 / STICK_CALIBRATION_HALF_RANGE as f64
    }

    pub fn normalized_right_y(&self) -> f64 {
        self.right_y() as f64 / STICK_CALIBRATION_HALF_RANGE as f64
    }

    pub fn set_left_x(&mut self, value: i16) {
        self.set_value_at_index(0, value);
    }

    pub fn set_left_y(&mut self, value: i16) {
        self.set_value_at_index(1, value);
    }

    pub fn set_right_x(&mut self, value: i16) {
        self.set_value_at_index(2, value);
    }

    pub fn set_right_y(&mut self, value: i16) {
        self.set_value_at_index(3, value);
    }
}

#[derive(Debug, Clone, PartialEq)]
#[repr(transparent)]
// Todo: change representation to parsed values
pub struct StickMinMaxCalibration {
    pub buf: [u8; 16], // 0x0000..0x0fff
}

impl Default for StickMinMaxCalibration {
    fn default() -> Self {
        Self { buf: [0u8; 16] }
    }
}

impl StickMinMaxCalibration {
    fn get_value_at_index(&self, index: u8) -> i16 {
        let index = index * 2;
        let raw =
            u16::from_le_bytes([self.buf[index as usize], self.buf[(index + 1) as usize]]) as i16;
        raw - STICK_CALIBRATION_HALF_RANGE as i16
    }

    fn set_value_at_index(&mut self, index: u8, value: i16) {
        let index = index * 2;
        let raw = value + STICK_CALIBRATION_HALF_RANGE as i16;
        let bytes = (raw as u16).clamp(0, STICK_CALIBRATION_RANGE).to_le_bytes();
        self.buf[index as usize] = bytes[0];
        self.buf[(index + 1) as usize] = bytes[1];
    }

    pub fn left_min_x(&self) -> i16 {
        self.get_value_at_index(0)
    }
    pub fn left_max_x(&self) -> i16 {
        self.get_value_at_index(1)
    }

    pub fn left_min_y(&self) -> i16 {
        self.get_value_at_index(2)
    }
    pub fn left_max_y(&self) -> i16 {
        self.get_value_at_index(3)
    }

    pub fn right_min_x(&self) -> i16 {
        self.get_value_at_index(4)
    }
    pub fn right_max_x(&self) -> i16 {
        self.get_value_at_index(5)
    }

    pub fn right_min_y(&self) -> i16 {
        self.get_value_at_index(6)
    }
    pub fn right_max_y(&self) -> i16 {
        self.get_value_at_index(7)
    }

    pub fn set_left_min_x(&mut self, value: i16) {
        self.set_value_at_index(0, value);
    }
    pub fn set_left_max_x(&mut self, value: i16) {
        self.set_value_at_index(1, value);
    }

    pub fn set_left_min_y(&mut self, value: i16) {
        self.set_value_at_index(2, value);
    }
    pub fn set_left_max_y(&mut self, value: i16) {
        self.set_value_at_index(3, value);
    }

    pub fn set_right_min_x(&mut self, value: i16) {
        self.set_value_at_index(4, value);
    }
    pub fn set_right_max_x(&mut self, value: i16) {
        self.set_value_at_index(5, value);
    }

    pub fn set_right_min_y(&mut self, value: i16) {
        self.set_value_at_index(6, value);
    }
    pub fn set_right_max_y(&mut self, value: i16) {
        self.set_value_at_index(7, value);
    }
}

#[derive(Debug)]
pub struct StickCenterCalibrationResult {
    pub calculated: StickCenterCalibration,
    pub collected: Vec<StickCenterCalibration>,
}

#[derive(Debug, Clone)]
#[repr(transparent)]
pub struct CalibrationFlag {
    pub buf: [u8; CALIBRATION_FLAG_SIZE],
}

impl CalibrationFlag {
    pub fn is_gyroscope_calib_ok(&self) -> bool {
        self.buf[0] & 0x01 > 0
    }

    pub fn is_accelerometer_calib_ok(&self) -> bool {
        self.buf[0] & 0x02 > 0
    }

    pub fn is_stick_min_max_calib_ok(&self) -> bool {
        self.buf[2] & 0x01 > 0
    }

    pub fn is_stick_center_calib_ok(&self) -> bool {
        self.buf[2] & 0x02 > 0
    }

    pub fn is_l2_calib_ok(&self) -> bool {
        self.buf[2] & 0x04 > 0
    }

    pub fn is_r2_calib_ok(&self) -> bool {
        self.buf[2] & 0x08 > 0
    }
}

impl Default for CalibrationFlag {
    fn default() -> Self {
        let mut s = Self {
            buf: [0u8; CALIBRATION_FLAG_SIZE],
        };
        s.buf[0] = 0x10;
        s
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TriggersCalibration {
    pub buf: Vec<u8>,
}

#[derive(Clone)]
pub struct StickPosition {
    pub x: u8,
    pub y: u8,
}

impl StickPosition {
    pub fn normalized_x(&self) -> f64 {
        self.x as f64 / STICK_CENTER - STICK_NORMALIZED_CENTER
    }

    pub fn normalized_y(&self) -> f64 {
        STICK_NORMALIZED_CENTER - self.y as f64 / STICK_CENTER
    }
}

#[derive(Debug)]
#[repr(u8)]
pub enum CalibrationType {
    Start(CalibrationDeviceType) = 0x01,
    Stop(CalibrationDeviceType) = 0x02,
    Measure(CalibrationDeviceType) = 0x03,

    None = 0xff,
}

impl TryFrom<[u8; 5]> for CalibrationType {
    type Error = String;

    fn try_from(value: [u8; 5]) -> std::result::Result<Self, Self::Error> {
        match value {
            [0x01, type_ @ ..] => Ok(CalibrationType::Start(type_.try_into()?)),
            [0x02, type_ @ ..] => Ok(CalibrationType::Stop(type_.try_into()?)),
            [0x03, type_ @ ..] => Ok(CalibrationType::Measure(type_.try_into()?)),
            [0xff, _, _, _, _] => Ok(CalibrationType::None),
            _ => Err(format!("Invalid calibration type {:?}", value)),
        }
    }
}

impl From<CalibrationType> for [u8; 5] {
    fn from(value: CalibrationType) -> Self {
        match value {
            CalibrationType::Start(type_) => {
                let type_array: [u8; 4] = type_.into();
                [
                    0x01,
                    type_array[0],
                    type_array[1],
                    type_array[2],
                    type_array[3],
                ]
            }
            CalibrationType::Stop(type_) => {
                let type_array: [u8; 4] = type_.into();
                [
                    0x02,
                    type_array[0],
                    type_array[1],
                    type_array[2],
                    type_array[3],
                ]
            }
            CalibrationType::Measure(type_) => {
                let type_array: [u8; 4] = type_.into();
                [
                    0x03,
                    type_array[0],
                    type_array[1],
                    type_array[2],
                    type_array[3],
                ]
            }
            CalibrationType::None => [0xff, 0xff, 0xff, 0x00, 0x00],
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
#[repr(u8)]
pub enum AnalogStickCalibrationType {
    Center = 0x01,
    MinMax = 0x02,

    None = 0xff,
}

impl TryFrom<u8> for AnalogStickCalibrationType {
    type Error = String;

    fn try_from(value: u8) -> std::result::Result<Self, Self::Error> {
        match value {
            0x01 => Ok(AnalogStickCalibrationType::Center),
            0x02 => Ok(AnalogStickCalibrationType::MinMax),
            0xff => Ok(AnalogStickCalibrationType::None),
            _ => Err(format!("Invalid analog stick calibration type {}", value)),
        }
    }
}

impl From<AnalogStickCalibrationType> for u8 {
    fn from(value: AnalogStickCalibrationType) -> Self {
        match value {
            AnalogStickCalibrationType::Center => 0x01,
            AnalogStickCalibrationType::MinMax => 0x02,
            AnalogStickCalibrationType::None => 0xff,
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
#[repr(u8)]
pub enum TriggerKeyCalibrationType {
    RecordMaxSample(TriggerKeyLeftRight) = 0x01,
    RecordRangeSample(TriggerKeyLeftRight) = 0x02,
    RecordMinSample(TriggerKeyLeftRight) = 0x03,
    Unknown(TriggerKeyLeftRight) = 0x00,

    None = 0xff,
}

#[derive(Debug, PartialEq, Clone)]
#[repr(u8)]
pub enum TriggerKeyLeftRight {
    Unknown = 0x00,
    Left = 0x01,
    Right = 0x02,
    Both = 0x03,
}

impl Display for TriggerKeyLeftRight {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            TriggerKeyLeftRight::Unknown => "Unknown",
            TriggerKeyLeftRight::Left => "Left",
            TriggerKeyLeftRight::Right => "Right",
            TriggerKeyLeftRight::Both => "Left and Right",
        })
    }
}

impl TryFrom<u8> for TriggerKeyLeftRight {
    type Error = String;

    fn try_from(value: u8) -> std::result::Result<Self, Self::Error> {
        match value {
            0x00 => Ok(TriggerKeyLeftRight::Unknown),
            0x01 => Ok(TriggerKeyLeftRight::Left),
            0x02 => Ok(TriggerKeyLeftRight::Right),
            0x03 => Ok(TriggerKeyLeftRight::Both),
            _ => Err(format!("Invalid trigger key {}", value)),
        }
    }
}

impl From<TriggerKeyLeftRight> for u8 {
    fn from(value: TriggerKeyLeftRight) -> Self {
        match value {
            TriggerKeyLeftRight::Unknown => 0x00,
            TriggerKeyLeftRight::Left => 0x01,
            TriggerKeyLeftRight::Right => 0x02,
            TriggerKeyLeftRight::Both => 0x03,
        }
    }
}

impl TryFrom<[u8; 2]> for TriggerKeyCalibrationType {
    type Error = String;

    fn try_from(value: [u8; 2]) -> std::result::Result<Self, Self::Error> {
        match value {
            [0x01, lr] => Ok(TriggerKeyCalibrationType::RecordMaxSample(
                TriggerKeyLeftRight::try_from(lr)?,
            )),
            [0x02, lr] => Ok(TriggerKeyCalibrationType::RecordRangeSample(
                TriggerKeyLeftRight::try_from(lr)?,
            )),
            [0x03, lr] => Ok(TriggerKeyCalibrationType::RecordMinSample(
                TriggerKeyLeftRight::try_from(lr)?,
            )),
            [0x00, lr] => Ok(TriggerKeyCalibrationType::Unknown(
                TriggerKeyLeftRight::try_from(lr)?,
            )),
            [0xff, _] => Ok(TriggerKeyCalibrationType::None),
            _ => Err(format!("Invalid trigger key calibration type {}", value[0])),
        }
    }
}

impl From<TriggerKeyCalibrationType> for [u8; 2] {
    fn from(value: TriggerKeyCalibrationType) -> Self {
        match value {
            TriggerKeyCalibrationType::RecordMaxSample(lr) => [0x01, lr.into()],
            TriggerKeyCalibrationType::RecordRangeSample(lr) => [0x02, lr.into()],
            TriggerKeyCalibrationType::RecordMinSample(lr) => [0x03, lr.into()],
            TriggerKeyCalibrationType::Unknown(lr) => [0x00, lr.into()],
            TriggerKeyCalibrationType::None => [0xff, 0x00],
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
#[repr(u8)]
pub enum CalibrationDeviceType {
    AnalogStick(AnalogStickCalibrationType) = 0x01,
    MotionSensor = 0x02,
    TriggerKey(TriggerKeyCalibrationType) = 0x03,

    None = 0xff,
}

impl TryFrom<[u8; 4]> for CalibrationDeviceType {
    type Error = String;

    fn try_from(value: [u8; 4]) -> std::result::Result<Self, Self::Error> {
        match value {
            [0x01, type_, 0x00, 0x00] => Ok(CalibrationDeviceType::AnalogStick(type_.try_into()?)),
            [0x02, 0x00, 0x00, 0x00] => Ok(CalibrationDeviceType::MotionSensor), // Unknown params
            [0x03, type_, lr, 0x00] => {
                Ok(CalibrationDeviceType::TriggerKey([type_, lr].try_into()?))
            }
            [0xff, _, _, _] => Ok(CalibrationDeviceType::None),
            _ => Err(format!("Invalid calibration device type {:?}", value)),
        }
    }
}

impl From<CalibrationDeviceType> for [u8; 4] {
    fn from(value: CalibrationDeviceType) -> Self {
        match value {
            CalibrationDeviceType::AnalogStick(type_) => [0x01, type_.into(), 0x00, 0x00],
            CalibrationDeviceType::MotionSensor => [0x02, 0x00, 0x00, 0x00],
            CalibrationDeviceType::TriggerKey(type_) => {
                let type_: [u8; 2] = type_.into();
                [0x03, type_[0], type_[1], 0x00]
            }
            CalibrationDeviceType::None => [0xff, 0xff, 0x00, 0x00],
        }
    }
}

#[derive(Debug)]
#[repr(u8)]
pub enum CalibrationState {
    Started(CalibrationDeviceType) = 0x01,
    Finished(CalibrationDeviceType) = 0x02,
    Unknown = 0xff,
}

impl TryFrom<[u8; 3]> for CalibrationState {
    type Error = String;

    fn try_from(value: [u8; 3]) -> std::result::Result<Self, Self::Error> {
        match value {
            [dev, type_, 0x01] => Ok(Self::Started([dev, type_, 0x00, 0x00].try_into()?)),
            [dev, type_, 0x02] => Ok(Self::Finished([dev, type_, 0x00, 0x00].try_into()?)),
            [_, _, 0xff] => Ok(Self::Unknown),
            _ => Err(format!("Invalid calibration state {:?}", value)),
        }
    }
}

#[derive(Debug)]
#[repr(u8)]
pub enum CalibrationResult {
    Completed(CalibrationDeviceType) = 0x01,
    NotCompleted(CalibrationDeviceType) = 0xff,
}

impl TryFrom<[u8; 3]> for CalibrationResult {
    type Error = String;

    fn try_from(value: [u8; 3]) -> std::result::Result<Self, Self::Error> {
        match value {
            [dev, type_, 0x01] => Ok(Self::Completed([dev, type_, 0x00, 0x00].try_into()?)),
            [dev, type_, 0xff] => Ok(Self::NotCompleted([dev, type_, 0x00, 0x00].try_into()?)),
            _ => Err(format!("Invalid calibration state {:?}", value)),
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum CalibrationData {
    StickCenter(StickCenterCalibration, Vec<StickCenterCalibration>),
    StickMinMax(StickMinMaxCalibration),
    Triggers(TriggersCalibration),
}

#[derive(Debug, PartialEq, Clone)]
pub enum FactoryCommand {
    SetIeepAddress(u16),
    TriggerMinMaxCalibration(TriggerMinMaxCalibrationType),
}

impl From<FactoryCommand> for [u8; 3] {
    fn from(value: FactoryCommand) -> Self {
        match value {
            FactoryCommand::SetIeepAddress(offset) => {
                let arg = offset.to_be_bytes();
                [0xff, arg[0], arg[1]]
            }
            FactoryCommand::TriggerMinMaxCalibration(type_) => {
                let arg: u8 = match type_ {
                    TriggerMinMaxCalibrationType::StartRecordMinMax(TriggerKeyLeftRight::Left) => {
                        0x40 | 0x02
                    }
                    TriggerMinMaxCalibrationType::StartRecordMinMax(TriggerKeyLeftRight::Right) => {
                        0x40 | 0x08
                    }
                    TriggerMinMaxCalibrationType::StartRecordMinMax(TriggerKeyLeftRight::Both) => {
                        0x40 | 0x02 | 0x08
                    }
                    TriggerMinMaxCalibrationType::StartRecordMinMax(
                        TriggerKeyLeftRight::Unknown,
                    ) => 0x40,
                    TriggerMinMaxCalibrationType::SaveMin(TriggerKeyLeftRight::Left) => 0x80 | 0x02,
                    TriggerMinMaxCalibrationType::SaveMin(TriggerKeyLeftRight::Right) => {
                        0x80 | 0x08
                    }
                    TriggerMinMaxCalibrationType::SaveMin(TriggerKeyLeftRight::Both) => {
                        0x80 | 0x02 | 0x08
                    }
                    TriggerMinMaxCalibrationType::SaveMin(TriggerKeyLeftRight::Unknown) => 0x80,
                    TriggerMinMaxCalibrationType::SaveMax(TriggerKeyLeftRight::Left) => 0x02,
                    TriggerMinMaxCalibrationType::SaveMax(TriggerKeyLeftRight::Right) => 0x08,
                    TriggerMinMaxCalibrationType::SaveMax(TriggerKeyLeftRight::Both) => 0x02 | 0x08,
                    TriggerMinMaxCalibrationType::SaveMax(TriggerKeyLeftRight::Unknown) => 0x00,
                    _ => panic!("Unsupported Factory Command"),
                };
                [0x02, arg, arg]
            }
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum TestCommand {
    SetPermanent(bool),
    RecordTriggerMinMax(TriggerKeyLeftRight, bool),
    ReloadTriggerMinMaxFromFlash,
}

impl From<TestCommand> for Report {
    fn from(value: TestCommand) -> Self {
        let payload = match value {
            TestCommand::SetPermanent(true) => vec![0x0a, 0x02, 0x3e, 0x71, 0x7f, 0x89],
            TestCommand::SetPermanent(false) => vec![0x0a, 0x01, 0x00],
            TestCommand::RecordTriggerMinMax(TriggerKeyLeftRight::Left, true) => {
                vec![0x08, 0x01, 0x01, 0x01]
            }
            TestCommand::RecordTriggerMinMax(TriggerKeyLeftRight::Left, false) => {
                vec![0x08, 0x01, 0x01, 0x00]
            }
            TestCommand::RecordTriggerMinMax(TriggerKeyLeftRight::Right, true) => {
                vec![0x08, 0x01, 0x02, 0x01]
            }
            TestCommand::RecordTriggerMinMax(TriggerKeyLeftRight::Right, false) => {
                vec![0x08, 0x01, 0x02, 0x00]
            }
            TestCommand::RecordTriggerMinMax(TriggerKeyLeftRight::Both, true) => {
                vec![0x08, 0x01, 0x00, 0x01]
            }
            TestCommand::RecordTriggerMinMax(TriggerKeyLeftRight::Both, false) => {
                vec![0x08, 0x01, 0x00, 0x00]
            }
            TestCommand::ReloadTriggerMinMaxFromFlash => vec![0x08, 0x02],
            _ => panic!("Unsupported test command"),
        };
        Report::from_payload(ReportId::SetTestCommand, payload.as_slice())
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum TriggerMinMaxCalibrationType {
    StartRecordMinMax(TriggerKeyLeftRight),
    SaveMin(TriggerKeyLeftRight),
    SaveMax(TriggerKeyLeftRight),
}

#[derive(Debug, Clone)]
#[repr(transparent)]
pub struct FlashMirror {
    pub buf: [u8; FLASH_MIRROR_SIZE],
}

impl Default for FlashMirror {
    fn default() -> Self {
        Self {
            buf: [0u8; FLASH_MIRROR_SIZE],
        }
    }
}

impl FlashMirror {
    pub fn calc_crc(&self) -> u16 {
        let mut crc = 0i16;
        for half_offset in 1..FLASH_MIRROR_SIZE / 2 {
            let first_byte_offset = half_offset * 2;
            let bytes: [u8; 2] = [self.buf[first_byte_offset], self.buf[first_byte_offset + 1]];
            let value = i16::from_le_bytes(bytes);
            crc = crc.overflowing_add(value).0;
        }
        crc.not() as u16
    }

    pub fn crc(&self) -> u16 {
        u16::from_le_bytes([self.buf[0], self.buf[1]])
    }

    pub fn check_crc(&self) -> bool {
        self.calc_crc() == self.crc()
    }

    pub fn update_crc(&mut self) {
        let crc = self.calc_crc().to_le_bytes();
        self.buf[0] = crc[0];
        self.buf[1] = crc[1];
    }

    pub fn stick_center_calibration(&self) -> StickCenterCalibration {
        let mut calibration = StickCenterCalibration::default();
        calibration.buf.copy_from_slice(&self.buf[0x11a..0x122]);
        calibration
    }
}

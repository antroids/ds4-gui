// Copyright 2023 Anton Kharuzhyi <publicantroids@gmail.com>
// SPDX-License-Identifier: GPL-3.0

#[derive(Debug)]
pub struct Report {
    id: ReportId,
    data: Vec<u8>,
}

impl Report {
    pub fn new(id: ReportId, payload_size: usize) -> Self {
        let mut data = vec![0u8; payload_size + 1];
        data[0] = id.clone() as u8;
        Self { id, data }
    }

    pub fn from_payload(id: ReportId, payload: &[u8]) -> Self {
        let mut data = vec![0u8; payload.len() + 1];
        data.as_mut_slice()[1..].copy_from_slice(payload);
        data[0] = id.clone() as u8;
        Self { id, data }
    }

    pub fn data_mut(&mut self) -> &mut [u8] {
        self.data.as_mut_slice()
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn id(&self) -> &ReportId {
        &self.id
    }

    pub fn payload(&self) -> &[u8] {
        &self.data[1..]
    }

    pub fn payload_mut(&mut self) -> &mut [u8] {
        &mut self.data[1..]
    }

    pub fn valid(&self) -> bool {
        self.data[0] == self.id.clone() as u8
    }
}

#[derive(Clone, Debug, PartialEq)]
#[repr(u8)]
pub enum ReportId {
    InputReport = 0x1,
    OutputDevice = 0x5,
    GetMotionCalibData = 0x2,
    SetMotionCalibData = 0x4,
    SetFactoryCommand = 0x8,
    GetCalibFlag = 0x10,
    GetIeepData = 0x11,
    GetParingInfo = 0x12,
    SetParingInfo = 0x13,
    SetUsbBtControl = 0x14,
    SetBdAdr = 0x80,
    GetBdAdr = 0x81,
    SetFactoryData = 0x82,
    SetAdrToGetFactoryData = 0x83,
    GetFactoryData = 0x84,
    SetPcbaId = 0x85,
    GetPcbaId = 0x86,
    GetTrackRecord = 0x87,
    SetCalibrationCommand = 0x90,
    GetCalibrationState = 0x91,
    GetCalibrationResult = 0x92,
    GetCalibrationData = 0x93,
    SetTestCommand = 0xa0,
    SetBtEnable = 0xa1,
    SetDfuEnable = 0xa2,
    GetFirmInfo = 0xa3,
    GetTestData = 0xa4,
}

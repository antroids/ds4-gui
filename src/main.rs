// Copyright 2023 Anton Kharuzhyi <publicantroids@gmail.com>
// SPDX-License-Identifier: GPL-3.0

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use crate::application::Application;
use clap::Parser;
use log::LevelFilter;
use simplelog::{ColorChoice, CombinedLogger, Config, TermLogger, TerminalMode, WriteLogger};
use std::fs;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::str::FromStr;

mod application;
mod dual_shock_4;

const APPLICATION_DIR: &str = "ds4-gui";
const LOG_FILE_NAME: &str = "ds4-gui.log";

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    log_dir: Option<String>,
}

fn main() -> application::Result<()> {
    let args = Args::parse();
    let default_log_dir = dirs::data_local_dir().unwrap().join(APPLICATION_DIR);

    let log_dir = args
        .log_dir
        .unwrap_or(default_log_dir.to_str().unwrap().to_string());
    let log_dir = Path::new(&log_dir);
    if !log_dir.exists() {
        fs::create_dir_all(&log_dir).expect("Cannot create log dir");
    }
    let log_file = log_dir.join(LOG_FILE_NAME);
    if log_file.exists() {
        for i in 0u16..999 {
            let mut rename_to = log_file.clone();
            rename_to.set_extension(i.to_string());
            if !rename_to.exists() {
                fs::rename(&log_file, rename_to).unwrap();
                break;
            }
        }
    }

    CombinedLogger::init(vec![
        TermLogger::new(
            LevelFilter::Debug,
            Config::default(),
            TerminalMode::Mixed,
            ColorChoice::Auto,
        ),
        WriteLogger::new(
            LevelFilter::Info,
            Config::default(),
            File::create(log_file).unwrap(),
        ),
    ])
    .unwrap();

    Application::show()
}

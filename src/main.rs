#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use crate::application::Application;
use clap::Parser;
use log::LevelFilter;
use simplelog::{ColorChoice, CombinedLogger, Config, TermLogger, TerminalMode, WriteLogger};
use std::fs;
use std::fs::File;
use std::path::Path;

mod application;
mod dual_shock_4;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    log: Option<String>,
}

fn main() -> application::Result<()> {
    let args = Args::parse();
    let log_file = args.log.unwrap_or("ds4-gui.log".to_string());

    if Path::new(log_file.clone().as_str()).exists() {
        for i in 0u16..999 {
            let rename_to = format!("{}{}", log_file.clone(), i);
            if !Path::new(rename_to.as_str()).exists() {
                fs::rename(log_file.clone(), rename_to).unwrap();
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

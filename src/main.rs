#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]
mod ui;
use cpal::traits::DeviceTrait;
use eframe::egui;
use jetseat_massage::{audio, diagnostic, i18n::tr, updater, usb};
fn cli(args: &[String]) -> Result<bool, String> {
    match args.first().map(String::as_str) {
        None => Ok(false),
        Some("--diagnose") => {
            let report = diagnostic::report(&diagnostic::scan()?)?;
            match args.get(1) {
                Some(path) => std::fs::write(path, report).map_err(|e| e.to_string())?,
                None => println!("{report}"),
            }
            Ok(true)
        }
        Some("--list-audio") => {
            for endpoint in audio::endpoints()? {
                let config = endpoint
                    .device
                    .default_output_config()
                    .map(|c| {
                        format!(
                            "{} Hz / {} channels / {:?}",
                            c.sample_rate().0,
                            c.channels(),
                            c.sample_format()
                        )
                    })
                    .unwrap_or_else(|e| e.to_string());
                println!("{}: {config}", endpoint.name);
            }
            Ok(true)
        }
        Some("--usb-self-test") => {
            usb::self_test()?;
            Ok(true)
        }
        Some("--check-updates") => {
            match updater::check()? {
                Some(release) => {
                    println!("Version {} is available: {}", release.version, release.url)
                }
                None => println!("Version {} is up to date", env!("CARGO_PKG_VERSION")),
            }
            Ok(true)
        }
        Some("--help" | "-h") => {
            println!(
                "JetSeat Massage\nNo arguments: GUI\n--diagnose [file.json]: USB/HID report without sending commands\n--list-audio: list audio outputs\n--usb-self-test: 8 zero-force USB cycles\n--check-updates: check GitHub for updates"
            );
            Ok(true)
        }
        Some(argument) => Err(format!("Unknown argument {argument}. Use --help.")),
    }
}

fn main() -> eframe::Result {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match cli(&args) {
        Ok(true) => return Ok(()),
        Ok(false) => {}
        Err(e) => {
            eprintln!("{}", tr(e));
            std::process::exit(1);
        }
    }
    eframe::run_native(
        concat!("JetSeat ", env!("CARGO_PKG_VERSION")),
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([760.0, 640.0])
                .with_min_inner_size([680.0, 600.0])
                .with_icon(
                    eframe::icon_data::from_png_bytes(include_bytes!(
                        "../assets/icons/app-256.png"
                    ))
                    .expect("embedded app icon"),
                )
                .with_app_id("jetseat-massage"),
            ..Default::default()
        },
        Box::new(|cc| Ok(Box::new(ui::App::new(cc)))),
    )
}

use cpal::traits::DeviceTrait;
use eframe::egui::{self, Color32, FontId, RichText, Stroke, Vec2};
use jetseat_massage::{
    audio::{self, Audio, Endpoint},
    diagnostic::{self, HidInfo},
    signal::{self, Pattern},
    updater::{self, Status as UpdateStatus, Updater},
    usb::{self, Usb},
};
use serde::{Deserialize, Serialize};
use std::{
    sync::mpsc::{self, Receiver},
    time::{Duration, Instant, SystemTime},
};

const BG: Color32 = Color32::from_rgb(16, 23, 30);
const CARD: Color32 = Color32::from_rgb(24, 33, 42);
const ACCENT: Color32 = Color32::from_rgb(115, 223, 192);
const MUTED: Color32 = Color32::from_rgb(146, 163, 177);
const LINE: Color32 = Color32::from_rgb(46, 61, 73);
const WARNING: Color32 = Color32::from_rgb(245, 186, 107);

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
struct Settings {
    pattern: Pattern,
    strength: u32,
    speed: f32,
    minutes: u64,
    frequency: u32,
    zones: [bool; 6],
    auto_updates: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            pattern: Pattern::Relax,
            strength: 25,
            speed: 1.0,
            minutes: 10,
            frequency: 70,
            zones: [true; 6],
            auto_updates: true,
        }
    }
}

impl Settings {
    fn sanitize(&mut self) {
        self.strength = self.strength.min(100);
        self.speed = if self.speed.is_finite() {
            self.speed.clamp(0.5, 2.0)
        } else {
            1.0
        };
        self.minutes = self.minutes.clamp(1, 30);
        self.frequency = self.frequency.clamp(40, 120);
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Page {
    Massage,
    Usb,
    Help,
    Updates,
}

#[derive(Clone, Copy, PartialEq)]
enum Output {
    Demo,
    Audio,
    Usb,
}

struct Run {
    started: Instant,
    duration: Duration,
    output: Output,
}

struct App {
    settings: Settings,
    updater: Updater,
    audio: Audio,
    usb_output: Usb,
    usb_devices: Vec<usb::Device>,
    usb_selected: Option<usize>,
    usb_scan_rx: Option<Receiver<Result<Vec<usb::Device>, String>>>,
    usb_scan_message: String,
    endpoints: Vec<Endpoint>,
    selected: Option<usize>,
    output: Output,
    page: Page,
    run: Option<Run>,
    message: String,
    error: bool,
    hid: Vec<HidInfo>,
    scan_rx: Option<Receiver<Result<Vec<HidInfo>, String>>>,
    scan_message: String,
    last_update: SystemTime,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = BG;
        visuals.window_fill = CARD;
        visuals.extreme_bg_color = BG;
        visuals.override_text_color = Some(Color32::from_rgb(230, 239, 246));
        visuals.selection.bg_fill = Color32::from_rgb(41, 89, 78);
        visuals.selection.stroke = Stroke::new(1.0, ACCENT);
        visuals.widgets.inactive.bg_fill = Color32::from_rgb(35, 47, 59);
        visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, LINE);
        visuals.widgets.hovered.bg_fill = Color32::from_rgb(47, 66, 76);
        visuals.widgets.active.bg_fill = Color32::from_rgb(48, 97, 84);
        cc.egui_ctx.set_visuals(visuals);
        cc.egui_ctx.style_mut(|style| {
            style.spacing.item_spacing = Vec2::new(12.0, 7.0);
            style.spacing.button_padding = Vec2::new(14.0, 7.0);
            style.spacing.slider_width = 260.0;
            style
                .text_styles
                .insert(egui::TextStyle::Body, FontId::proportional(16.0));
            style
                .text_styles
                .insert(egui::TextStyle::Button, FontId::proportional(16.0));
            style
                .text_styles
                .insert(egui::TextStyle::Small, FontId::proportional(13.0));
            style
                .text_styles
                .insert(egui::TextStyle::Heading, FontId::proportional(28.0));
        });
        let mut settings: Settings = cc
            .storage
            .and_then(|s| eframe::get_value(s, "settings"))
            .unwrap_or_default();
        settings.sanitize();
        let (endpoints, message, error) = match audio::endpoints() {
            Ok(devices) => (
                devices,
                "Готово. Выберите программу и начните сеанс.".into(),
                false,
            ),
            Err(e) => (
                Vec::new(),
                format!("Не удалось получить аудиовыходы: {e}"),
                true,
            ),
        };
        let mut app = Self {
            settings,
            updater: Updater::default(),
            audio: Audio::default(),
            usb_output: Usb::default(),
            usb_devices: Vec::new(),
            usb_selected: None,
            usb_scan_rx: None,
            usb_scan_message: String::new(),
            endpoints,
            selected: None,
            output: Output::Demo,
            page: Page::Massage,
            run: None,
            message,
            error,
            hid: Vec::new(),
            scan_rx: None,
            scan_message: "Подключите накидку по USB и нажмите «Обновить».".into(),
            last_update: SystemTime::now(),
        };
        app.find_usb();
        #[cfg(feature = "screenshot")]
        match std::env::var("JETSEAT_PREVIEW").as_deref() {
            Ok("usb") => app.output = Output::Usb,
            Ok("help") => app.page = Page::Help,
            Ok("updates") => app.page = Page::Updates,
            _ => {}
        }
        app
    }

    fn usb_config(&self) -> usb::Config {
        usb::Config {
            pattern: self.settings.pattern,
            strength: self.settings.strength,
            speed: self.settings.speed,
            zones: self.settings.zones,
        }
    }

    fn start(&mut self) {
        if matches!(self.updater.status, UpdateStatus::Installing) {
            self.message = "Дождитесь завершения установки обновления.".into();
            self.error = true;
            return;
        }
        self.settings.sanitize();
        let seconds = self.settings.minutes * 60;
        let result = match self.output {
            Output::Demo => Ok(()),
            Output::Audio => {
                self.audio.configure(
                    self.settings.pattern,
                    self.settings.strength,
                    self.settings.speed,
                    self.settings.frequency,
                );
                match self.selected.and_then(|i| self.endpoints.get(i)) {
                    Some(endpoint) => self.audio.start(&endpoint.device, seconds),
                    None => Err("Выберите аудиовыход, к которому подключён кабель накидки.".into()),
                }
            }
            Output::Usb => {
                self.usb_output.configure(self.usb_config());
                match self.usb_selected.and_then(|i| self.usb_devices.get(i)) {
                    Some(device) => self.usb_output.start(device, seconds),
                    None => Err("Подключите KW-901 по USB и выберите устройство.".into()),
                }
            }
        };
        if let Err(error) = result {
            self.message = error;
            self.error = true;
            return;
        }
        self.run = Some(Run {
            started: Instant::now(),
            duration: Duration::from_secs(seconds),
            output: self.output,
        });
        self.message = match self.output {
            Output::Demo => "Демосеанс: звук и команды на устройство не отправляются.",
            Output::Audio => "Сигнал подаётся на выбранный аудиовыход.",
            Output::Usb => "USB-сеанс: отдельное управление шестью зонами.",
        }
        .into();
        self.error = false;
    }

    fn stop(&mut self, message: &str) {
        self.audio.stop();
        let result = self.usb_output.stop();
        self.run = None;
        self.message = result
            .as_ref()
            .err()
            .cloned()
            .unwrap_or_else(|| message.into());
        self.error = result.is_err();
    }

    fn scan(&mut self) {
        if self
            .run
            .as_ref()
            .is_some_and(|run| run.output == Output::Usb)
        {
            self.scan_message = "Остановите USB-массаж перед диагностикой устройств.".into();
            return;
        }
        if self.scan_rx.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.scan_rx = Some(rx);
        self.scan_message = "Поиск HID-устройств…".into();
        std::thread::spawn(move || {
            let _ = tx.send(diagnostic::scan());
        });
    }

    fn find_usb(&mut self) {
        if self.usb_scan_rx.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.usb_scan_rx = Some(rx);
        self.usb_scan_message = "Поиск KW-901…".into();
        // macOS HID enumeration pumps its thread's CFRunLoop. Keep it outside
        // winit's main event handler, where it would re-enter the GUI loop.
        std::thread::spawn(move || {
            let _ = tx.send(usb::devices());
        });
    }

    fn tick(&mut self, ctx: &egui::Context) {
        self.updater.poll();
        if self.updater.close_requested {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if self.settings.auto_updates && self.run.is_none() {
            if self.updater.due() {
                self.updater.check();
            } else if matches!(self.updater.status, UpdateStatus::Available(_)) {
                self.updater.download();
            }
        }
        ctx.request_repaint_after(Duration::from_secs(1));
        if let Some(rx) = &self.usb_scan_rx
            && let Ok(result) = rx.try_recv()
        {
            match result {
                Ok(devices) => {
                    self.usb_devices = devices;
                    self.usb_selected = (self.usb_devices.len() == 1).then_some(0);
                    self.usb_scan_message = if self.usb_devices.is_empty() {
                        "Накидка не найдена. Подключите USB и штатное питание, включите пульт и повторите поиск.".into()
                    } else {
                        "Аудиокабель не нужен. При запуске проверяется HID-дескриптор и отправляется команда нулевой силы.".into()
                    };
                }
                Err(error) => self.usb_scan_message = format!("Ошибка поиска: {error}"),
            }
            self.usb_scan_rx = None;
        }
        let now = SystemTime::now();
        if self.run.is_some()
            && now.duration_since(self.last_update).unwrap_or_default() > Duration::from_secs(3)
        {
            self.stop("Сеанс остановлен после сна компьютера или длительной паузы приложения.");
        }
        self.last_update = now;
        self.audio.configure(
            self.settings.pattern,
            self.settings.strength,
            self.settings.speed,
            self.settings.frequency,
        );
        self.audio.heartbeat();
        self.usb_output.configure(self.usb_config());
        self.usb_output.heartbeat();
        if let Some(error) = self.audio.poll_error() {
            self.stop("Аудиосеанс остановлен.");
            self.message = format!("Аудиовыход недоступен: {error}");
            self.error = true;
        } else if let Some(error) = self.usb_output.poll_error() {
            self.stop("USB-сеанс остановлен.");
            self.message =
                format!("USB: {error}. Если вибрация продолжается, выключите питание на пульте.");
            self.error = true;
        } else if let Some(run) = &self.run {
            if run.started.elapsed() >= run.duration {
                self.stop("Сеанс завершён. Сигнал выключен.");
            } else if (run.output == Output::Audio && !self.audio.is_active())
                || (run.output == Output::Usb && !self.usb_output.is_active())
            {
                self.stop("Сигнал остановлен. Для продолжения нажмите «Начать».");
            }
        }
        if let Some(rx) = &self.scan_rx
            && let Ok(result) = rx.try_recv()
        {
            match result {
                Ok(hid) => {
                    self.scan_message = format!(
                        "HID-интерфейсов: {}. Совпадение по названию: {}.",
                        hid.len(),
                        hid.iter().filter(|d| d.candidate).count()
                    );
                    self.hid = hid;
                }
                Err(e) => {
                    self.hid.clear();
                    self.scan_message = format!("Ошибка поиска: {e}");
                }
            }
            self.scan_rx = None;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.stop("Остановлено клавишей Escape.");
        }
        if ctx.input(|i| i.viewport().close_requested()) {
            self.stop("Остановлено при закрытии приложения.");
        }
        if self.run.is_some() || self.scan_rx.is_some() || self.usb_scan_rx.is_some() {
            ctx.request_repaint_after(Duration::from_millis(40));
        }
    }

    fn header(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(RichText::new("JETSEAT").size(12.0).color(ACCENT).strong());
                ui.heading("Массаж в вашем ритме");
                ui.label(RichText::new("Gametrix KW-901").color(MUTED));
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let (label, color) = if self.run.is_some() {
                    if self.output == Output::Demo {
                        ("ДЕМОСЕАНС", WARNING)
                    } else {
                        (
                            if self.output == Output::Usb {
                                "USB-СЕАНС"
                            } else {
                                "АУДИО ВКЛЮЧЕНО"
                            },
                            ACCENT,
                        )
                    }
                } else {
                    ("ГОТОВ К ЗАПУСКУ", MUTED)
                };
                ui.label(RichText::new(label).color(color).size(12.0).strong());
            });
        });
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.page, Page::Massage, "Массаж");
            ui.selectable_value(&mut self.page, Page::Usb, "USB-диагностика");
            ui.selectable_value(&mut self.page, Page::Help, "Подключение");
            ui.selectable_value(
                &mut self.page,
                Page::Updates,
                if matches!(self.updater.status, UpdateStatus::Ready(_)) {
                    "Обновление готово"
                } else {
                    "Обновления"
                },
            );
        });
        ui.add_space(4.0);
        ui.separator();
        ui.add_space(8.0);
    }

    fn massage(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.add_enabled_ui(self.run.is_none(), |ui| {
                ui.selectable_value(&mut self.output, Output::Demo, "Демо");
                ui.selectable_value(&mut self.output, Output::Usb, "USB · 6 зон");
                ui.selectable_value(&mut self.output, Output::Audio, "Аудио · 3,5 мм");
            });
            ui.label(
                RichText::new(match self.output {
                    Output::Demo => "Посмотрите программы без подключения",
                    Output::Usb => "Отдельное управление моторами",
                    Output::Audio => "Общая реакция на звук",
                })
                .color(MUTED)
                .size(14.0),
            );
        });
        ui.add_space(10.0);
        if self.output == Output::Audio {
            if matches!(self.settings.pattern, Pattern::Snake | Pattern::Alternating) {
                self.settings.pattern = Pattern::Relax;
            }
            card(ui, |ui| {
                ui.label(RichText::new("Аудиовыход").strong());
                ui.horizontal_wrapped(|ui| {
                    ui.add_enabled_ui(self.run.is_none(), |ui| {
                        egui::ComboBox::from_id_salt("output")
                            .selected_text(
                                self.selected
                                    .and_then(|i| self.endpoints.get(i))
                                    .map(|e| e.name.as_str())
                                    .unwrap_or("Выберите выход…"),
                            )
                            .width(290.0)
                            .show_ui(ui, |ui| {
                                for (i, endpoint) in self.endpoints.iter().enumerate() {
                                    ui.selectable_value(
                                        &mut self.selected,
                                        Some(i),
                                        &endpoint.name,
                                    );
                                }
                            });
                        if ui.button("Обновить").clicked() {
                            match audio::endpoints() {
                                Ok(endpoints) => {
                                    self.endpoints = endpoints;
                                    self.selected = None;
                                }
                                Err(e) => {
                                    self.message = e;
                                    self.error = true;
                                }
                            }
                        }
                    });
                });
                ui.label(RichText::new("Подключите аудиокабель накидки к выбранному выходу и включите перехват звука. Если выбрать динамики, сигнал будет слышен.").size(13.0).color(MUTED));
            });
            ui.add_space(12.0);
        } else if self.output == Output::Usb {
            card(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.add_enabled_ui(self.run.is_none(), |ui| {
                        egui::ComboBox::from_id_salt("usb_output")
                            .selected_text(
                                self.usb_selected
                                    .and_then(|i| self.usb_devices.get(i))
                                    .map(|e| e.name.as_str())
                                    .unwrap_or("Выберите KW-901…"),
                            )
                            .width(420.0)
                            .show_ui(ui, |ui| {
                                for (i, device) in self.usb_devices.iter().enumerate() {
                                    ui.selectable_value(
                                        &mut self.usb_selected,
                                        Some(i),
                                        &device.name,
                                    );
                                }
                            });
                        if ui
                            .add_enabled(
                                self.usb_scan_rx.is_none(),
                                egui::Button::new("Найти накидку"),
                            )
                            .clicked()
                        {
                            self.find_usb();
                        }
                    });
                });
                ui.label(
                    RichText::new(&self.usb_scan_message)
                        .size(13.0)
                        .color(MUTED),
                );
            });
            ui.add_space(12.0);
        }
        ui.columns(2, |columns| {
            card(&mut columns[0], |ui| {
                ui.label(
                    RichText::new("01  ПРОГРАММА")
                        .size(12.0)
                        .color(MUTED)
                        .strong(),
                );
                ui.add_space(6.0);
                let cell_width = (ui.available_width() - 12.0) / 2.0 - 20.0;
                egui::Grid::new("programs")
                    .num_columns(2)
                    .spacing(Vec2::new(10.0, 10.0))
                    .show(ui, |ui| {
                        for (i, pattern) in Pattern::ALL
                            .into_iter()
                            .filter(|p| {
                                self.output != Output::Audio
                                    || !matches!(p, Pattern::Snake | Pattern::Alternating)
                            })
                            .enumerate()
                        {
                            let chosen = self.settings.pattern == pattern;
                            let response = egui::Frame::new()
                                .fill(if chosen {
                                    Color32::from_rgb(30, 65, 59)
                                } else {
                                    Color32::from_rgb(29, 40, 51)
                                })
                                .stroke(Stroke::new(1.0, if chosen { ACCENT } else { LINE }))
                                .corner_radius(10)
                                .inner_margin(10)
                                .show(ui, |ui| {
                                    ui.vertical(|ui| {
                                        ui.set_width(cell_width);
                                        ui.set_min_height(68.0);
                                        ui.label(
                                            RichText::new(pattern.title())
                                                .size(18.0)
                                                .color(if chosen { ACCENT } else { Color32::WHITE })
                                                .strong(),
                                        );
                                        ui.add(
                                            egui::Label::new(
                                                RichText::new(pattern.description())
                                                    .size(12.0)
                                                    .color(MUTED),
                                            )
                                            .wrap(),
                                        );
                                    });
                                })
                                .response
                                .interact(egui::Sense::click());
                            if response.clicked() {
                                self.settings.pattern = pattern;
                            }
                            response.on_hover_cursor(egui::CursorIcon::PointingHand);
                            if i % 2 == 1 {
                                ui.end_row();
                            }
                        }
                    });
            });
            card(&mut columns[1], |ui| {
                ui.label(
                    RichText::new("02  НАСТРОЙКИ СЕАНСА")
                        .size(12.0)
                        .color(MUTED)
                        .strong(),
                );
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    ui.label(if self.output == Output::Audio {
                        "Сила сигнала"
                    } else {
                        "Сила вибрации"
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!("{}%", self.settings.strength))
                                .size(25.0)
                                .color(ACCENT)
                                .strong(),
                        );
                    });
                });
                ui.add(egui::Slider::new(&mut self.settings.strength, 0..=100).show_value(false));
                ui.label(
                    RichText::new(if self.output == Output::Audio {
                        "Зависит также от громкости выхода и пульта."
                    } else {
                        "Уровень команды моторам. Начальная сила — 25%."
                    })
                    .size(12.0)
                    .color(MUTED),
                );
                ui.add_space(7.0);
                ui.label("Темп программы");
                ui.add(
                    egui::Slider::new(&mut self.settings.speed, 0.5..=2.0)
                        .step_by(0.1)
                        .suffix("×"),
                );
                ui.add_space(7.0);
                ui.label("Длительность");
                ui.add_enabled_ui(self.run.is_none(), |ui| {
                    ui.horizontal(|ui| {
                        for minutes in [5, 10, 15, 20] {
                            ui.selectable_value(
                                &mut self.settings.minutes,
                                minutes,
                                format!("{minutes} мин"),
                            );
                        }
                    });
                    ui.add(egui::Slider::new(&mut self.settings.minutes, 1..=30).suffix(" мин"));
                });
                if self.output == Output::Audio {
                    egui::CollapsingHeader::new("Настройка аудиосигнала").show(ui, |ui| {
                        ui.label(
                            RichText::new("Частота звука, а не обороты моторов.")
                                .size(12.0)
                                .color(MUTED),
                        );
                        ui.add(
                            egui::Slider::new(&mut self.settings.frequency, 40..=120).suffix(" Гц"),
                        );
                    });
                }
            });
        });
        if self.output != Output::Audio {
            ui.add_space(12.0);
            let t = self
                .run
                .as_ref()
                .map(|r| r.started.elapsed().as_secs_f64())
                .unwrap_or(0.0);
            let levels = if self.output == Output::Usb {
                self.usb_output.levels()
            } else if self.run.is_some() {
                signal::motor_levels(
                    self.settings.pattern,
                    t,
                    self.settings.speed as f64,
                    self.settings.strength as f32 / 100.0,
                    self.settings.minutes as f64 * 60.0,
                    self.settings.zones,
                )
            } else {
                [0.0; 6]
            };
            card(ui, |ui| {
                ui.label(
                    RichText::new("03  ЗОНЫ НАКИДКИ")
                        .size(12.0)
                        .color(MUTED)
                        .strong(),
                );
                ui.horizontal_wrapped(|ui| {
                    for (i, name) in signal::ZONES.iter().enumerate() {
                        ui.vertical(|ui| {
                            ui.checkbox(&mut self.settings.zones[i], *name);
                            ui.add(
                                egui::ProgressBar::new(levels[i])
                                    .fill(ACCENT)
                                    .desired_width(132.0)
                                    .desired_height(4.0),
                            );
                        });
                    }
                });
            });
        }
    }

    fn session(&mut self, ui: &mut egui::Ui) {
        let (elapsed, remaining, progress) = match &self.run {
            Some(run) => {
                let t = run.started.elapsed().as_secs_f64();
                (
                    t,
                    (run.duration.as_secs_f64() - t).max(0.0),
                    (t / run.duration.as_secs_f64()) as f32,
                )
            }
            None => (0.0, self.settings.minutes as f64 * 60.0, 0.0),
        };
        let level = if self.run.is_none() {
            0.0
        } else {
            match self.output {
                Output::Usb => self.usb_output.levels().into_iter().fold(0.0_f32, f32::max),
                Output::Audio => self.audio.meter(),
                Output::Demo => signal::motor_levels(
                    self.settings.pattern,
                    elapsed,
                    self.settings.speed as f64,
                    self.settings.strength as f32 / 100.0,
                    self.settings.minutes as f64 * 60.0,
                    self.settings.zones,
                )
                .into_iter()
                .fold(0.0_f32, f32::max),
            }
        };
        ui.horizontal(|ui| {
            let secs = remaining.ceil() as u64;
            ui.label(
                RichText::new(format!("{:02}:{:02}", secs / 60, secs % 60))
                    .size(32.0)
                    .strong(),
            );
            ui.add_space(12.0);
            let ready = self.run.is_none()
                && !matches!(self.updater.status, UpdateStatus::Installing)
                && match self.output {
                    Output::Demo => true,
                    Output::Audio => self.selected.is_some(),
                    Output::Usb => self.usb_selected.is_some(),
                };
            if ui
                .add_enabled(
                    ready,
                    egui::Button::new(
                        RichText::new(if self.output == Output::Demo {
                            "Начать демосеанс"
                        } else {
                            "Начать массаж"
                        })
                        .color(BG)
                        .strong(),
                    )
                    .fill(ACCENT)
                    .min_size(Vec2::new(210.0, 42.0)),
                )
                .clicked()
            {
                self.start();
            }
            if ui
                .add_enabled(
                    self.run.is_some(),
                    egui::Button::new("Стоп · Esc").min_size(Vec2::new(125.0, 42.0)),
                )
                .clicked()
            {
                self.stop("Сеанс остановлен. Сигнал выключен.");
            }
            ui.label(
                RichText::new(format!("Уровень: {:.0}%", level * 100.0))
                    .color(MUTED)
                    .size(13.0),
            );
        });
        ui.add(
            egui::ProgressBar::new(progress)
                .fill(ACCENT)
                .desired_height(4.0),
        );
        ui.label(
            RichText::new(&self.message)
                .size(13.0)
                .color(if self.error { WARNING } else { MUTED }),
        );
    }

    fn usb(&mut self, ui: &mut egui::Ui) {
        card(ui, |ui| {
            ui.heading("Подключение по USB");
            ui.label("Прямое USB-управление добавлено: на вкладке «Массаж» выберите «USB · 6 зон». Здесь можно проверить идентификаторы устройства.");
            ui.label(RichText::new("Перед отправкой команд проверяются VID/PID и формат HID-отчёта. Для USB-массажа аудиокабель не нужен.").color(MUTED));
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(
                        self.scan_rx.is_none(),
                        egui::Button::new("Обновить устройства"),
                    )
                    .clicked()
                {
                    self.scan();
                }
                if ui
                    .add_enabled(
                        !self.hid.is_empty(),
                        egui::Button::new("Копировать JSON-отчёт"),
                    )
                    .clicked()
                {
                    match diagnostic::report(&self.hid) {
                        Ok(report) => {
                            ui.ctx().copy_text(report);
                            self.scan_message =
                                "Отчёт скопирован. Его можно прислать для проверки подключения."
                                    .into();
                        }
                        Err(e) => self.scan_message = e,
                    }
                }
            });
            ui.label(RichText::new(&self.scan_message).size(13.0).color(MUTED));
        });
        ui.add_space(12.0);
        for device in &self.hid {
            card(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        RichText::new(if device.product.is_empty() {
                            "Без названия"
                        } else {
                            &device.product
                        })
                        .strong(),
                    );
                    if device.known_protocol {
                        ui.label(RichText::new("ПРОТОКОЛ GAMETRIX").color(ACCENT).size(11.0));
                    } else if device.candidate {
                        ui.label(RichText::new("ВОЗМОЖНО, JETSEAT").color(ACCENT).size(11.0));
                    }
                });
                ui.label(
                    RichText::new(format!(
                        "VID {:04X} · PID {:04X} · Usage {:04X}:{:04X} · интерфейс {}",
                        device.vendor_id,
                        device.product_id,
                        device.usage_page,
                        device.usage,
                        device.interface
                    ))
                    .size(13.0)
                    .color(MUTED),
                );
                if !device.manufacturer.is_empty() {
                    ui.small(&device.manufacturer);
                }
            });
            ui.add_space(6.0);
        }
    }

    fn help(&mut self, ui: &mut egui::Ui) {
        card(ui, |ui| {
            ui.heading("USB: шесть отдельных зон");
            ui.label("1. Подключите штатное питание KW-901 и включите пульт.");
            ui.label("2. Подключите USB к MacBook или компьютеру с CachyOS.");
            ui.label("3. На вкладке «Массаж» выберите «USB · 6 зон», нажмите «Найти накидку» и выберите устройство.");
            ui.label("4. Выберите программу, зоны и силу. Нажмите «Начать массаж».");
            ui.label(RichText::new("Аудиокабель не требуется. Если устройство найдено, но не вибрирует, проверьте питание и USB-режим контроллера. Эта версия не меняет прошивку или сохранённые настройки контроллера.").color(MUTED));
        });
        ui.add_space(12.0);
        card(ui, |ui| {
            ui.heading("Аудио: общая реакция на звук");
            ui.label("Подключите аудиокабель 3,5 мм к выходу MacBook или звуковой карты, включите перехват звука на пульте, выберите «Аудио · 3,5 мм» и соответствующий аудиовыход.");
            ui.label(RichText::new("Начните с небольшой системной громкости и силы 25%. Другие звуки на этом выходе тоже могут вызывать вибрацию. Стоп выключает только сигнал этого приложения.").color(MUTED));
        });
        ui.add_space(12.0);
        card(ui, |ui| {
            ui.label(RichText::new("Программы").strong());
            ui.label("Релакс — общее плавное нарастание. Волна по USB — движение от сиденья к спине. Пульс — импульсы с отдыхом. Ровный — постоянная сила. Змейка — обход всех зон. Лево / право — чередование сторон.");
            ui.label(RichText::new("Змейка, чередование сторон и выбор отдельных зон доступны через USB и в деморежиме. По аудиокабелю доступны четыре общих программы.").color(MUTED));
            ui.hyperlink_to(
                "KW-901: инструкция производителя",
                "https://gametrix.ru/catalog/gaming/vibration-pads/kw901-jetseat-live-sense",
            );
            ui.small(format!("JetSeat Massage {}", env!("CARGO_PKG_VERSION")));
        });
    }

    fn updates(&mut self, ui: &mut egui::Ui) {
        let mut check = false;
        let mut download = false;
        let mut install = false;
        card(ui, |ui| {
            ui.heading("Обновления");
            ui.label(format!("Установлена версия {}", env!("CARGO_PKG_VERSION")));
            ui.add_space(10.0);
            ui.checkbox(
                &mut self.settings.auto_updates,
                "Автоматически проверять и скачивать обновления",
            );
            ui.label(RichText::new("Проверка при запуске и каждые 6 часов, когда массаж остановлен. Перед установкой сверяется SHA-256.").color(MUTED));
            ui.add_space(14.0);
            match &self.updater.status {
                UpdateStatus::Idle => {
                    ui.label("Обновления ещё не проверялись.");
                }
                UpdateStatus::Checking => {
                    ui.spinner();
                    ui.label("Проверяем новые версии…");
                }
                UpdateStatus::Current => {
                    ui.label("Установлена актуальная версия.");
                }
                UpdateStatus::Available(release) => {
                    ui.label(format!("Доступна версия {}", release.version));
                    download = ui.button("Скачать обновление").clicked();
                }
                UpdateStatus::Downloading => {
                    ui.spinner();
                    ui.label("Скачиваем и проверяем обновление…");
                }
                UpdateStatus::Ready(prepared) => {
                    ui.label(
                        RichText::new(format!(
                            "Версия {} готова к установке",
                            prepared.release.version
                        ))
                        .color(ACCENT),
                    );
                    ui.label("Обновление устанавливается системным установщиком; может понадобиться пароль администратора. После установки запустите JetSeat Massage снова.");
                    if self.run.is_some() {
                        ui.label("Остановите массаж перед установкой.");
                    }
                    install = ui
                        .add_enabled(
                            self.run.is_none(),
                            egui::Button::new("Установить обновление"),
                        )
                        .clicked();
                }
                UpdateStatus::Installing => {
                    ui.spinner();
                    ui.label("Открываем установщик…");
                }
                UpdateStatus::Error(error) => {
                    ui.label(RichText::new(error).color(WARNING));
                }
            }
            ui.add_space(12.0);
            check = ui
                .add_enabled(!self.updater.busy(), egui::Button::new("Проверить сейчас"))
                .clicked();
            ui.hyperlink_to("Все версии на GitHub", updater::RELEASES_URL);
        });
        if check {
            self.updater.check();
        }
        if download {
            self.updater.download();
        }
        if install {
            self.stop("Установка обновления. Сигнал выключен.");
            if !self.error {
                self.updater.install();
            }
        }
    }
}

fn card(ui: &mut egui::Ui, content: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(CARD)
        .corner_radius(14)
        .inner_margin(14)
        .stroke(Stroke::new(1.0, LINE))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            content(ui);
        });
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        #[cfg(feature = "screenshot")]
        if let Ok(path) = std::env::var("JETSEAT_SCREENSHOT") {
            if let Some(pixels) = ctx.input(|input| {
                input.events.iter().find_map(|event| {
                    if let egui::Event::Screenshot { image, .. } = event {
                        Some(image.clone())
                    } else {
                        None
                    }
                })
            }) {
                let bytes: Vec<u8> = pixels
                    .pixels
                    .iter()
                    .flat_map(|color| color.to_array())
                    .collect();
                image::save_buffer(
                    path,
                    &bytes,
                    pixels.size[0] as u32,
                    pixels.size[1] as u32,
                    image::ColorType::Rgba8,
                )
                .expect("save GUI screenshot");
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            } else if ctx.cumulative_pass_nr() > 12 {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
            }
            ctx.request_repaint_after(Duration::from_millis(40));
        }
        self.tick(ctx);
        if self.page == Page::Massage || self.run.is_some() {
            egui::TopBottomPanel::bottom("session")
                .frame(egui::Frame::new().fill(CARD).inner_margin(18))
                .show(ctx, |ui| self.session(ui));
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(BG).inner_margin(24))
            .show(ctx, |ui| {
                self.header(ui);
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| match self.page {
                        Page::Massage => self.massage(ui),
                        Page::Usb => self.usb(ui),
                        Page::Help => self.help(ui),
                        Page::Updates => self.updates(ui),
                    });
            });
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, "settings", &self.settings);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.audio.stop();
        let _ = self.usb_output.stop();
    }
}

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
                            "{} Гц / {} каналов / {:?}",
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
                Some(release) => println!("Доступна версия {}: {}", release.version, release.url),
                None => println!("Версия {} актуальна", env!("CARGO_PKG_VERSION")),
            }
            Ok(true)
        }
        Some("--help" | "-h") => {
            println!(
                "JetSeat Massage\nБез аргументов: GUI\n--diagnose [file.json]: USB/HID-отчёт без отправки команд\n--list-audio: доступные аудиовыходы\n--usb-self-test: 8 циклов USB с нулевой силой\n--check-updates: проверить доступную версию на GitHub"
            );
            Ok(true)
        }
        Some(argument) => Err(format!(
            "Неизвестный аргумент {argument}. Используйте --help."
        )),
    }
}

fn main() -> eframe::Result {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match cli(&args) {
        Ok(true) => return Ok(()),
        Ok(false) => {}
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
    eframe::run_native(
        concat!("JetSeat Massage ", env!("CARGO_PKG_VERSION"), " · KW-901"),
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([1050.0, 860.0])
                .with_min_inner_size([820.0, 620.0])
                .with_app_id("jetseat-massage"),
            ..Default::default()
        },
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
}

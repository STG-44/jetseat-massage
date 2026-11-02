mod theme;
use eframe::egui::{self, RichText, Stroke, Vec2};
use jetseat_massage::{
    audio::{self, Audio, Endpoint},
    diagnostic::{self, HidInfo},
    i18n::{self, Language, tr},
    session::End,
    signal::{self, Pattern},
    updater::{self, Status as UpdateStatus, Updater},
    usb::{self, Usb},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    sync::mpsc::{self, Receiver},
    time::{Duration, Instant},
};
use theme::*;

#[derive(Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
enum Output {
    #[default]
    Usb,
    Audio,
    Demo,
}
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
    language: Language,
    auto_connect: bool,
    output: Output,
    developer_mode: bool,
    layout_revision: u32,
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
            language: Language::default(),
            auto_connect: true,
            output: Output::Usb,
            developer_mode: false,
            layout_revision: 0,
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
#[derive(Default)]
struct View {
    settings: bool,
    advanced: bool,
    diagnostics: bool,
}
struct Run {
    started: Instant,
    duration: Duration,
    output: Output,
}
pub struct App {
    settings: Settings,
    view: View,
    updater: Updater,
    audio: Audio,
    usb: Usb,
    devices: Vec<usb::Device>,
    selected: Option<usize>,
    usb_scan: Option<Receiver<Result<Vec<usb::Device>, String>>>,
    last_scan: Instant,
    endpoints: Vec<Endpoint>,
    endpoint: Option<usize>,
    run: Option<Run>,
    message: String,
    error: bool,
    hid: Vec<HidInfo>,
    hid_scan: Option<Receiver<Result<Vec<HidInfo>, String>>>,
    logs: VecDeque<String>,
    opened: Instant,
}
impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::configure(&cc.egui_ctx);
        let mut settings: Settings = cc
            .storage
            .and_then(|s| eframe::get_value(s, "settings"))
            .unwrap_or_default();
        settings.sanitize();
        if settings.layout_revision < 2 {
            cc.egui_ctx
                .send_viewport_cmd(egui::ViewportCommand::InnerSize(Vec2::new(760.0, 640.0)));
            settings.layout_revision = 2;
        }
        #[cfg(feature = "screenshot")]
        if let Ok(tag) = std::env::var("JETSEAT_LANGUAGE") {
            settings.language = Language::from_tag(&tag);
        }
        #[cfg(feature = "screenshot")]
        if std::env::var_os("JETSEAT_ZERO_FORCE_QA").is_some() {
            // Opt-in hardware QA can never send a nonzero motor command.
            settings.strength = 0;
            settings.minutes = 30;
            settings.output = Output::Usb;
            settings.auto_updates = false;
            settings.developer_mode = true;
        }
        i18n::set_language(settings.language);
        i18n::configure_fonts(&cc.egui_ctx, settings.language);
        let endpoints = audio::endpoints().unwrap_or_default();
        let mut app = Self {
            settings,
            view: View::default(),
            updater: Updater::default(),
            audio: Audio::default(),
            usb: Usb::default(),
            devices: Vec::new(),
            selected: None,
            usb_scan: None,
            last_scan: Instant::now() - Duration::from_secs(10),
            endpoints,
            endpoint: None,
            run: None,
            message: String::new(),
            error: false,
            hid: Vec::new(),
            hid_scan: None,
            logs: VecDeque::new(),
            opened: Instant::now(),
        };
        if app.settings.auto_connect {
            app.find_usb();
        }
        #[cfg(feature = "screenshot")]
        match std::env::var("JETSEAT_PREVIEW").as_deref() {
            Ok("updates" | "help" | "settings") => app.view.settings = true,
            Ok("advanced") => app.view.advanced = true,
            Ok("diagnostics") => {
                app.settings.developer_mode = true;
                app.view.diagnostics = true;
            }
            Ok("demo") => app.settings.output = Output::Demo,
            _ => {}
        }
        app
    }
    fn busy(&self) -> bool {
        self.run.is_some() || self.usb.is_busy() || self.audio.is_active()
    }
    fn config(&self) -> usb::Config {
        #[cfg(feature = "screenshot")]
        let strength = if std::env::var_os("JETSEAT_ZERO_FORCE_QA").is_some() {
            0
        } else {
            self.settings.strength
        };
        #[cfg(not(feature = "screenshot"))]
        let strength = self.settings.strength;
        usb::Config {
            pattern: self.settings.pattern,
            strength,
            speed: self.settings.speed,
            zones: self.settings.zones,
        }
    }
    fn log(&mut self, text: impl Into<String>) {
        if self.logs.len() >= 100 {
            self.logs.pop_front();
        }
        self.logs.push_back(format!(
            "[+{:.1}s] {}",
            self.opened.elapsed().as_secs_f32(),
            text.into()
        ));
    }
    fn fail(&mut self, user: &str, detail: impl Into<String>) {
        self.log(detail);
        self.message = user.into();
        self.error = true;
    }
    fn ready(&self) -> bool {
        !self.busy()
            && !matches!(self.updater.status, UpdateStatus::Installing)
            && match self.settings.output {
                Output::Usb => self.selected.and_then(|i| self.devices.get(i)).is_some(),
                Output::Audio => self.endpoint.is_some(),
                Output::Demo => true,
            }
    }
    fn start(&mut self) {
        if !self.ready() {
            return;
        }
        self.settings.sanitize();
        let seconds = self.settings.minutes * 60;
        self.audio.stop();
        if let Err(e) = self.usb.stop() {
            self.fail(
                "Команда остановки не подтверждена. Отключите питание накидки.",
                e,
            );
            return;
        }
        let result = match self.settings.output {
            Output::Demo => Ok(()),
            Output::Usb => {
                self.usb.configure(self.config());
                self.usb
                    .start(&self.devices[self.selected.unwrap()], seconds)
            }
            Output::Audio => {
                self.audio.configure(
                    self.settings.pattern,
                    self.settings.strength,
                    self.settings.speed,
                    self.settings.frequency,
                );
                self.audio
                    .start(&self.endpoints[self.endpoint.unwrap()].device, seconds)
            }
        };
        if let Err(e) = result {
            if e.starts_with("Накидка уже используется") {
                self.fail(&e.clone(), e);
            } else {
                self.fail(
                    "Не удалось начать сеанс. Проверьте подключение и питание.",
                    e,
                );
            }
            return;
        }
        self.run = Some(Run {
            started: Instant::now(),
            duration: Duration::from_secs(seconds),
            output: self.settings.output,
        });
        self.message.clear();
        self.error = false;
        self.log("Session started");
    }
    fn stop(&mut self, message: &str) {
        self.audio.stop();
        let result = self.usb.stop();
        self.run = None;
        match result {
            Ok(()) => {
                self.message = message.into();
                self.error = false;
            }
            Err(e) => self.fail(
                "Команда остановки не подтверждена. Отключите питание накидки.",
                e,
            ),
        }
        self.log("Session stopped");
    }
    fn find_usb(&mut self) {
        if self.usb_scan.is_some() || self.usb.is_busy() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.usb_scan = Some(rx);
        self.last_scan = Instant::now();
        std::thread::spawn(move || {
            let _ = tx.send(usb::devices());
        });
    }
    fn tick(&mut self, ctx: &egui::Context) {
        self.updater.poll();
        if self.updater.close_requested {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if self.settings.auto_updates && !self.busy() {
            if self.updater.due() {
                self.updater.check();
            } else if matches!(self.updater.status, UpdateStatus::Available(_)) {
                self.updater.download();
            }
        }
        if let Some(rx) = &self.usb_scan
            && let Ok(result) = rx.try_recv()
        {
            match result {
                Ok(devices) => {
                    let previous = self.selected.and_then(|i| self.devices.get(i)).cloned();
                    self.selected = previous
                        .and_then(|old| devices.iter().position(|d| d.same_device(&old)))
                        .or_else(|| (devices.len() == 1).then_some(0));
                    self.devices = devices;
                }
                Err(e) => {
                    self.selected = None;
                    self.log(e);
                }
            }
            self.usb_scan = None;
        }
        if let Some(rx) = &self.hid_scan
            && let Ok(result) = rx.try_recv()
        {
            match result {
                Ok(hid) => self.hid = hid,
                Err(e) => self.log(e),
            }
            self.hid_scan = None;
        }
        self.usb.configure(self.config());
        self.audio.configure(
            self.settings.pattern,
            self.settings.strength,
            self.settings.speed,
            self.settings.frequency,
        );
        if let Some(e) = self.usb.poll_error() {
            self.stop("");
            self.selected = None;
            let message = if self.usb.is_busy() {
                "Команда остановки не подтверждена. Отключите питание накидки."
            } else {
                "Соединение с накидкой потеряно. Проверьте USB и питание."
            };
            self.fail(message, e);
        } else if let Some(e) = self.audio.poll_error() {
            self.stop("");
            self.fail("Аудиовыход недоступен. Проверьте настройки подключения.", e);
        } else if let Some(run) = &self.run {
            if self.usb.end_reason() == Some(End::Suspended) && run.output == Output::Usb {
                self.stop("Сеанс остановлен после сна компьютера.");
            } else if run.started.elapsed() >= run.duration {
                self.stop("Сеанс завершён. Сигнал выключен.");
            } else if (run.output == Output::Usb && !self.usb.is_active())
                || (run.output == Output::Audio && !self.audio.is_active())
            {
                self.stop("Сеанс остановлен.");
            }
        }
        if self.settings.auto_connect
            && self.settings.output == Output::Usb
            && !self.busy()
            && self.last_scan.elapsed() > Duration::from_secs(8)
        {
            self.find_usb();
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.stop("Сеанс остановлен.");
        }
        if ctx.input(|i| i.viewport().close_requested()) {
            self.stop("Сеанс остановлен.");
            if self.usb.is_busy() {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            }
        }
        ctx.request_repaint_after(if self.busy() {
            Duration::from_millis(100)
        } else {
            Duration::from_secs(1)
        });
    }
    fn header(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("JetSeat").size(24.0).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add_sized(
                        [34.0, 32.0],
                        egui::Button::new(RichText::new("⚙").size(20.0)),
                    )
                    .on_hover_text(tr("Настройки"))
                    .clicked()
                {
                    self.view.settings = true;
                }
                let (label, color) = match self.settings.output {
                    Output::Demo => ("Демо", MUTED),
                    Output::Audio => (
                        "Аудио",
                        if self.endpoint.is_some() {
                            ACCENT
                        } else {
                            MUTED
                        },
                    ),
                    Output::Usb if self.selected.is_some() => ("Подключено", ACCENT),
                    Output::Usb if self.usb_scan.is_some() => ("Подключение…", MUTED),
                    _ => ("Не подключено", MUTED),
                };
                ui.label(RichText::new(tr(label)).size(13.0).color(color));
                ui.label(RichText::new("●").size(10.0).color(color));
            });
        });
        ui.add_space(12.0);
        ui.separator();
        ui.add_space(12.0);
    }
    fn connection_prompt(&mut self, ui: &mut egui::Ui) -> bool {
        let needs_usb = self.settings.output == Output::Usb && self.selected.is_none();
        let needs_audio = self.settings.output == Output::Audio && self.endpoint.is_none();
        if !needs_usb && !needs_audio {
            return false;
        }
        ui.add_space(60.0);
        ui.vertical_centered(|ui| {
            ui.label(
                RichText::new(tr(if needs_audio {
                    "Выберите аудиовыход в настройках."
                } else if self.devices.len() > 1 {
                    "Найдено несколько накидок. Выберите устройство."
                } else {
                    "Подключите накидку по USB."
                }))
                .size(20.0),
            );
            ui.add_space(12.0);
            ui.label(
                RichText::new(tr("Подключите штатное питание и включите пульт.")).color(MUTED),
            );
            ui.add_space(16.0);
            if needs_audio || self.devices.len() > 1 {
                if ui.button(tr("Настройки")).clicked() {
                    self.view.settings = true;
                }
            } else if ui
                .add_enabled(
                    self.usb_scan.is_none(),
                    egui::Button::new(tr("Подключить")).fill(ACCENT),
                )
                .clicked()
            {
                self.find_usb();
            }
            if ui.link(tr("Другие способы подключения")).clicked() {
                self.view.settings = true;
            }
        });
        true
    }
    fn massage(&mut self, ui: &mut egui::Ui) {
        if self.connection_prompt(ui) {
            return;
        }
        ui.label(RichText::new(tr("Программы массажа")).strong());
        ui.add_space(4.0);
        let programs: Vec<_> = Pattern::ALL
            .into_iter()
            .filter(|p| {
                self.settings.output != Output::Audio
                    || !matches!(p, Pattern::Snake | Pattern::Alternating)
            })
            .collect();
        let count = if ui.available_width() > 610.0 && programs.len() == 6 {
            3
        } else {
            2
        };
        let width = ((ui.available_width() - 10.0 * (count - 1) as f32) / count as f32).max(144.0);
        egui::Grid::new("program-grid")
            .num_columns(count)
            .spacing([10.0, 10.0])
            .show(ui, |ui| {
                for (i, pattern) in programs.into_iter().enumerate() {
                    let selected = self.settings.pattern == pattern;
                    let response = egui::Frame::new()
                        .fill(if selected {
                            egui::Color32::from_rgb(26, 48, 46)
                        } else {
                            CARD
                        })
                        .stroke(Stroke::new(1.0_f32, if selected { ACCENT } else { LINE }))
                        .corner_radius(4)
                        .inner_margin(12)
                        .show(ui, |ui| {
                            ui.vertical(|ui| {
                                ui.set_width((width - 26.0).max(100.0));
                                ui.set_min_height(42.0);
                                ui.label(RichText::new(tr(pattern.title())).size(16.0).strong());
                                ui.label(
                                    RichText::new(tr(short_description(pattern)))
                                        .size(12.0)
                                        .color(MUTED),
                                );
                            });
                        })
                        .response
                        .interact(egui::Sense::click());
                    if response.clicked() {
                        self.settings.pattern = pattern;
                    }
                    response.on_hover_cursor(egui::CursorIcon::PointingHand);
                    if i % count == count - 1 {
                        ui.end_row();
                    }
                }
            });
        ui.add_space(16.0);
        ui.separator();
        ui.add_space(12.0);
        heading_value(ui, "Интенсивность", format!("{}%", self.settings.strength));
        ui.scope(|ui| {
            ui.spacing_mut().slider_width = ui.available_width().max(80.0);
            ui.visuals_mut().selection.bg_fill = ACCENT;
            ui.add(
                egui::Slider::new(&mut self.settings.strength, 0..=100)
                    .show_value(false)
                    .trailing_fill(true),
            );
        });
        ui.add_space(12.0);
        heading_value(ui, "Темп программы", format!("{:.1}×", self.settings.speed));
        ui.horizontal(|ui| {
            ui.label(RichText::new(tr("Медленно")).small().color(MUTED));
            let width = (ui.available_width() - 80.0).max(100.0);
            ui.scope(|ui| {
                ui.spacing_mut().slider_width = width;
                ui.add(
                    egui::Slider::new(&mut self.settings.speed, 0.5..=2.0)
                        .step_by(0.1)
                        .show_value(false),
                );
            });
            ui.label(RichText::new(tr("Быстро")).small().color(MUTED));
        });
        ui.add_space(12.0);
        ui.label(RichText::new(tr("Длительность")).strong());
        ui.add_enabled_ui(!self.busy(), |ui| {
            ui.horizontal(|ui| {
                let width = ((ui.available_width() - 40.0) / 5.0).max(54.0);
                for minutes in [5, 10, 15, 20, 30] {
                    let selected = self.settings.minutes == minutes;
                    if ui
                        .add_sized(
                            [width, 34.0],
                            egui::Button::new(tr(format!("{minutes} мин"))).selected(selected),
                        )
                        .clicked()
                    {
                        self.settings.minutes = minutes;
                    }
                }
            });
        });
    }
    fn footer(&mut self, ui: &mut egui::Ui) {
        let remaining = self
            .run
            .as_ref()
            .map(|r| r.duration.as_secs_f64() - r.started.elapsed().as_secs_f64())
            .unwrap_or(self.settings.minutes as f64 * 60.0)
            .max(0.0)
            .ceil() as u64;
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(
                    RichText::new(format!("{:02}:{:02}", remaining / 60, remaining % 60))
                        .size(34.0)
                        .strong(),
                );
                ui.label(RichText::new(tr("Осталось времени")).small().color(MUTED));
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add_enabled(
                        self.ready(),
                        egui::Button::new(RichText::new(tr("Начать")).color(BG).strong())
                            .fill(ACCENT)
                            .min_size(Vec2::new(104.0, 42.0)),
                    )
                    .clicked()
                {
                    self.start();
                }
                if ui
                    .add_enabled(
                        self.busy(),
                        egui::Button::new(tr("Стоп")).min_size(Vec2::new(92.0, 42.0)),
                    )
                    .on_hover_text("Esc")
                    .clicked()
                {
                    self.stop("Сеанс остановлен.");
                }
            });
        });
        if !self.message.is_empty() {
            ui.label(
                RichText::new(tr(&self.message))
                    .small()
                    .color(if self.error { WARNING } else { MUTED }),
            );
        }
    }
    fn settings_window(&mut self, ctx: &egui::Context, bounds: egui::Rect) {
        let mut open = self.view.settings;
        egui::Window::new(tr("Настройки"))
            .open(&mut open)
            .constrain_to(bounds)
            .max_height(bounds.height().max(140.0) - 12.0)
            .default_width(470.0)
            .resizable(true)
            .collapsible(false)
            .vscroll(true)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(tr("Язык"));
                    let previous = self.settings.language;
                    egui::ComboBox::from_id_salt("language")
                        .selected_text(self.settings.language.name())
                        .show_ui(ui, |ui| {
                            for language in Language::ALL {
                                ui.selectable_value(&mut self.settings.language, language, language.name());
                            }
                        });
                    if previous != self.settings.language {
                        i18n::set_language(self.settings.language);
                        i18n::configure_fonts(ctx, self.settings.language);
                    }
                });
                ui.add_space(10.0);
                ui.separator();
                ui.label(RichText::new(tr("Подключение")).strong());
                let before = self.settings.output;
                ui.add_enabled_ui(!self.busy(), |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.selectable_value(&mut self.settings.output, Output::Usb, "USB");
                        ui.selectable_value(&mut self.settings.output, Output::Audio, tr("Аудио · 3,5 мм"));
                        ui.selectable_value(&mut self.settings.output, Output::Demo, tr("Демо"));
                    });
                });
                if before != self.settings.output {
                    self.stop("");
                    if self.settings.output == Output::Audio {
                        self.settings.pattern = Pattern::Relax;
                    }
                }
                match self.settings.output {
                    Output::Usb => {
                        ui.checkbox(&mut self.settings.auto_connect, tr("Подключаться автоматически"));
                        if self.devices.len() > 1 {
                            ui.add_enabled_ui(!self.busy(), |ui| {
                                egui::ComboBox::from_id_salt("seat")
                                    .selected_text(self.selected.and_then(|i| self.devices.get(i))
                                        .map(|d| d.title.clone()).unwrap_or_else(|| tr("Выберите устройство…")))
                                    .show_ui(ui, |ui| {
                                        for (i, device) in self.devices.iter().enumerate() {
                                            ui.selectable_value(&mut self.selected, Some(i), format!("{} · {}", device.title, i + 1));
                                        }
                                    });
                            });
                        } else if let Some(device) = self.selected.and_then(|i| self.devices.get(i)) {
                            ui.label(&device.title);
                        }
                        if (self.selected.is_none() || self.devices.len() > 1)
                            && ui.add_enabled(!self.usb.is_busy() && self.usb_scan.is_none(), egui::Button::new(tr("Подключить"))).clicked()
                        {
                            self.find_usb();
                        }
                        ui.label(RichText::new(tr("Подключите штатное питание и включите пульт.")).small().color(MUTED));
                    }
                    Output::Audio => {
                        ui.add_enabled_ui(!self.busy(), |ui| {
                            egui::ComboBox::from_id_salt("audio-output")
                                .selected_text(self.endpoint.and_then(|i| self.endpoints.get(i))
                                    .map(|e| e.name.clone()).unwrap_or_else(|| tr("Выберите выход…")))
                                .show_ui(ui, |ui| {
                                    for (i, endpoint) in self.endpoints.iter().enumerate() {
                                        ui.selectable_value(&mut self.endpoint, Some(i), &endpoint.name);
                                    }
                                });
                        });
                        if ui.add_enabled(!self.busy(), egui::Button::new(tr("Обновить"))).clicked() {
                            match audio::endpoints() {
                                Ok(devices) => { self.endpoints = devices; self.endpoint = None; }
                                Err(e) => self.log(e),
                            }
                        }
                        ui.label(RichText::new(tr("Подключите аудиокабель накидки к выбранному выходу и включите перехват звука. Если выбрать динамики, сигнал будет слышен.")).small().color(MUTED));
                    }
                    Output::Demo => { ui.label(tr("Демосеанс: звук и команды на устройство не отправляются.")); }
                }
                ui.add_space(10.0);
                ui.separator();
                self.updates(ui);
                ui.add_space(10.0);
                ui.separator();
                if ui.button(tr("Расширенные настройки")).clicked() {
                    self.view.advanced = true;
                }
                ui.small(format!("JetSeat Massage {}", env!("CARGO_PKG_VERSION")));
            });
        self.view.settings = open;
    }
    fn updates(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new(tr("Обновления")).strong());
        ui.checkbox(
            &mut self.settings.auto_updates,
            tr("Автоматически проверять и скачивать обновления"),
        );
        let mut download = false;
        let mut install = false;
        match &self.updater.status {
            UpdateStatus::Idle => {
                ui.small(tr("Обновления ещё не проверялись."));
            }
            UpdateStatus::Checking => {
                ui.small(tr("Проверяем новые версии…"));
            }
            UpdateStatus::Current => {
                ui.small(tr("Установлена актуальная версия."));
            }
            UpdateStatus::Available(release) => {
                ui.label(tr(format!("Доступна версия {}", release.version)));
                download = ui.button(tr("Скачать обновление")).clicked();
            }
            UpdateStatus::Downloading => {
                ui.small(tr("Скачиваем и проверяем обновление…"));
            }
            UpdateStatus::Ready(prepared) => {
                ui.label(tr(format!(
                    "Версия {} готова к установке",
                    prepared.release.version
                )));
                ui.small(tr("После установки снова запустите JetSeat Massage. Для системных пакетов может понадобиться пароль администратора."));
                install = ui
                    .add_enabled(!self.busy(), egui::Button::new(tr("Установить обновление")))
                    .clicked();
            }
            UpdateStatus::Installing => {
                ui.small(tr("Открываем установщик…"));
            }
            UpdateStatus::Error(error) => {
                ui.label(RichText::new(tr(error)).small().color(WARNING));
            }
        }
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    !self.updater.busy(),
                    egui::Button::new(tr("Проверить сейчас")),
                )
                .clicked()
            {
                self.updater.check();
            }
            ui.hyperlink_to(tr("Все версии на GitHub"), updater::RELEASES_URL);
        });
        if download {
            self.updater.download();
        }
        if install {
            self.stop("");
            if !self.error {
                self.updater.install();
            }
        }
    }
    fn advanced_window(&mut self, ctx: &egui::Context, bounds: egui::Rect) {
        let mut open = self.view.advanced;
        egui::Window::new(tr("Расширенные настройки"))
            .open(&mut open)
            .constrain_to(bounds)
            .max_height(bounds.height().max(140.0) - 12.0)
            .default_width(480.0)
            .resizable(true)
            .collapsible(false)
            .vscroll(true)
            .show(ctx, |ui| {
                if self.settings.output != Output::Audio {
                    ui.label(RichText::new(tr("Зоны накидки")).strong());
                    egui::Grid::new("zones").num_columns(2).show(ui, |ui| {
                        for (i, name) in signal::ZONES.iter().enumerate() {
                            ui.checkbox(&mut self.settings.zones[i], tr(*name));
                            if i % 2 == 1 {
                                ui.end_row();
                            }
                        }
                    });
                } else {
                    ui.label(tr("Настройка аудиосигнала"));
                    ui.add(
                        egui::Slider::new(&mut self.settings.frequency, 40..=120).suffix(tr(" Гц")),
                    );
                    ui.small(tr("Частота звука, а не обороты моторов."));
                }
                ui.add_space(10.0);
                ui.label(tr("Длительность"));
                ui.add_enabled(
                    !self.busy(),
                    egui::Slider::new(&mut self.settings.minutes, 1..=30).suffix(tr(" мин")),
                );
                ui.add_space(10.0);
                ui.separator();
                ui.checkbox(&mut self.settings.developer_mode, tr("Режим разработчика"));
                if self.settings.developer_mode && ui.button(tr("Открыть диагностику")).clicked()
                {
                    self.view.diagnostics = true;
                }
            });
        self.view.advanced = open;
    }
    fn diagnostics_window(&mut self, ctx: &egui::Context, bounds: egui::Rect) {
        let mut open = self.view.diagnostics;
        egui::Window::new(tr("Диагностика"))
            .open(&mut open)
            .constrain_to(bounds)
            .max_height(bounds.height().max(140.0) - 12.0)
            .default_width(590.0)
            .resizable(true)
            .collapsible(false)
            .vscroll(true)
            .show(ctx, |ui| {
                ui.label(tr("Текущая команда моторам"));
                let levels = self.usb.levels();
                ui.monospace(format!("{:?}", levels.map(|x| (x * 100.0).round() as u32)));
                ui.monospace(format!("TX #{}: {}", self.usb.write_count(), self.usb.last_report()
                    .iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" ")));
                ui.small(tr("Индикаторы показывают команду, а не измеренную вибрацию."));
                ui.horizontal(|ui| {
                    if ui.add_enabled(!self.usb.is_busy() && self.hid_scan.is_none(), egui::Button::new(tr("Обновить устройства"))).clicked() {
                        let (tx, rx) = mpsc::channel();
                        self.hid_scan = Some(rx);
                        std::thread::spawn(move || { let _ = tx.send(diagnostic::scan()); });
                    }
                    if ui.button(tr("Копировать JSON-отчёт")).clicked() {
                        let report = serde_json::json!({
                            "application": "JetSeat Massage", "version": env!("CARGO_PKG_VERSION"),
                            "os": std::env::consts::OS, "arch": std::env::consts::ARCH,
                            "devices": self.hid, "motor_command": levels,
                            "write_count": self.usb.write_count(), "last_report": self.usb.last_report(), "logs": self.logs
                        });
                        if let Ok(json) = serde_json::to_string_pretty(&report) { ctx.copy_text(json); }
                    }
                });
                for device in &self.hid {
                    ui.label(&device.product);
                    ui.monospace(format!("VID {:04X} · PID {:04X} · Usage {:04X}:{:04X} · interface {}",
                        device.vendor_id, device.product_id, device.usage_page, device.usage, device.interface));
                }
                ui.separator();
                ui.label(tr("Журнал событий"));
                for line in &self.logs { ui.label(RichText::new(line).monospace().small()); }
            });
        self.view.diagnostics = open;
    }
}
fn short_description(pattern: Pattern) -> &'static str {
    match pattern {
        Pattern::Relax => "Мягкий",
        Pattern::Wave => "Плавный",
        Pattern::Pulse => "Ритмичный",
        Pattern::Steady => "Постоянный",
        Pattern::Snake => "Последовательный",
        Pattern::Alternating => "Попеременный",
    }
}
fn heading_value(ui: &mut egui::Ui, label: &str, value: String) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(tr(label)).strong());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new(value).strong());
        });
    });
}
impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        #[cfg(feature = "screenshot")]
        if let Ok(path) = std::env::var("JETSEAT_SCREENSHOT") {
            if let Some(pixels) = ctx.input(|input| {
                input.events.iter().find_map(|e| {
                    if let egui::Event::Screenshot { image, .. } = e {
                        Some(image.clone())
                    } else {
                        None
                    }
                })
            }) {
                let bytes: Vec<u8> = pixels.pixels.iter().flat_map(|c| c.to_array()).collect();
                image::save_buffer(
                    path,
                    &bytes,
                    pixels.size[0] as u32,
                    pixels.size[1] as u32,
                    image::ColorType::Rgba8,
                )
                .expect("save GUI screenshot");
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            } else if ctx.cumulative_frame_nr() > 30 {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
            }
            ctx.request_repaint_after(Duration::from_millis(40));
        }
        self.tick(ctx);
        let footer = egui::TopBottomPanel::bottom("session")
            .frame(egui::Frame::new().fill(BG).inner_margin(20))
            .show(ctx, |ui| self.footer(ui))
            .response
            .rect;
        let mut bounds = ctx.content_rect();
        bounds.max.y = footer.min.y - 4.0;
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(BG).inner_margin(24))
            .show(ctx, |ui| {
                self.header(ui);
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| self.massage(ui));
            });
        if self.view.settings {
            self.settings_window(ctx, bounds);
        }
        if self.view.advanced {
            self.advanced_window(ctx, bounds);
        }
        if self.view.diagnostics && self.settings.developer_mode {
            self.diagnostics_window(ctx, bounds);
        }
    }
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        #[cfg(feature = "screenshot")]
        if std::env::var_os("JETSEAT_ZERO_FORCE_QA").is_some() {
            return;
        }
        eframe::set_value(storage, "settings", &self.settings);
    }
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.audio.stop();
        let _ = self.usb.stop();
    }
}

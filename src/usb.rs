use crate::{
    protocol,
    signal::{self, Pattern},
};
use hidapi::HidDevice;
use std::{
    ffi::CString,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant, SystemTime},
};

#[derive(Clone)]
pub struct Device {
    pub name: String,
    path: CString,
    vendor: u16,
    product: u16,
}

pub fn devices() -> Result<Vec<Device>, String> {
    crate::hid_service::with_api(|api| {
        api.refresh_devices().map_err(|e| e.to_string())?;
        Ok(api
            .device_list()
            .filter(|d| protocol::supported_id(d.vendor_id(), d.product_id()))
            .map(|d| Device {
                name: format!(
                    "{} · {:04X}:{:04X} · интерфейс {}",
                    d.product_string().unwrap_or("Gametrix JetSeat"),
                    d.vendor_id(),
                    d.product_id(),
                    d.interface_number()
                ),
                path: d.path().to_owned(),
                vendor: d.vendor_id(),
                product: d.product_id(),
            })
            .collect())
    })
}

/// Exercises the real enumerate/open/write/close path without vibration.
/// Short-lived callers reproduce the macOS 0.2 lifetime regression.
pub fn self_test() -> Result<(), String> {
    for cycle in 1..=8 {
        let devices = thread::spawn(devices)
            .join()
            .map_err(|_| "Ошибка потока поиска USB".to_owned())??;
        if devices.len() != 1 {
            return Err(format!(
                "Для проверки нужна одна накидка, найдено: {}",
                devices.len()
            ));
        }
        let mut usb = Usb::default();
        usb.configure(Config {
            strength: 0,
            ..Config::default()
        });
        usb.start(&devices[0], 2)?;
        let deadline = Instant::now() + Duration::from_millis(220);
        while Instant::now() < deadline {
            usb.heartbeat();
            if let Some(error) = usb.poll_error() {
                return Err(error);
            }
            thread::sleep(Duration::from_millis(20));
        }
        usb.stop()?;
        if let Some(error) = usb.poll_error() {
            return Err(error);
        }
        println!("USB zero-force cycle {cycle}/8: OK · {}", devices[0].name);
    }
    Ok(())
}

#[derive(Clone, Copy)]
pub struct Config {
    pub pattern: Pattern,
    pub strength: u32,
    pub speed: f32,
    pub zones: [bool; 6],
}

impl Default for Config {
    fn default() -> Self {
        Self {
            pattern: Pattern::Relax,
            strength: 25,
            speed: 1.0,
            zones: [true; 6],
        }
    }
}

struct State {
    active: AtomicBool,
    config: Mutex<Config>,
    heartbeat: Mutex<SystemTime>,
    levels: Mutex<[f32; 6]>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            active: AtomicBool::new(false),
            config: Mutex::new(Config::default()),
            heartbeat: Mutex::new(SystemTime::now()),
            levels: Mutex::new([0.0; 6]),
        }
    }
}

pub struct Usb {
    state: Arc<State>,
    worker: Option<JoinHandle<()>>,
    tx: Sender<String>,
    errors: Receiver<String>,
}

impl Default for Usb {
    fn default() -> Self {
        let (tx, errors) = mpsc::channel();
        Self {
            state: Arc::new(State::default()),
            worker: None,
            tx,
            errors,
        }
    }
}

impl Usb {
    pub fn configure(&self, config: Config) {
        if let Ok(mut value) = self.state.config.lock() {
            *value = config;
        }
    }

    pub fn heartbeat(&self) {
        if let Ok(mut time) = self.state.heartbeat.lock() {
            *time = SystemTime::now();
        }
    }

    pub fn start(&mut self, device: &Device, seconds: u64) -> Result<(), String> {
        self.stop()?;
        while self.errors.try_recv().is_ok() {}
        if !protocol::supported_id(device.vendor, device.product) {
            return Err("USB-устройство не поддерживается".into());
        }
        self.heartbeat();
        self.state.active.store(true, Ordering::Release);
        let selected = device.clone();
        let state = self.state.clone();
        let tx = self.tx.clone();
        self.worker = Some(thread::spawn(move || {
            let control = state.clone();
            let result = crate::hid_service::with_api(move |api| {
                let handle = api
                    .open_path(&selected.path)
                    .map_err(|e| format!("Не удалось открыть KW-901: {e}"))?;
                let info = handle.get_device_info().map_err(|e| e.to_string())?;
                if !protocol::supported_id(info.vendor_id(), info.product_id()) {
                    return Err("Идентификатор подключённого устройства изменился".into());
                }
                let mut descriptor = [0_u8; hidapi::MAX_REPORT_DESCRIPTOR_SIZE];
                let length = handle
                    .get_report_descriptor(&mut descriptor)
                    .map_err(|e| format!("Не удалось прочитать HID-дескриптор: {e}"))?;
                protocol::validate_descriptor(&descriptor[..length])?;
                run_worker(
                    handle,
                    &control,
                    Duration::from_secs(seconds.clamp(1, 1800)),
                )
            });
            state.active.store(false, Ordering::Release);
            if let Ok(mut levels) = state.levels.lock() {
                *levels = [0.0; 6];
            }
            if let Err(error) = result {
                let _ = tx.send(error);
            }
        }));
        Ok(())
    }

    /// Waits briefly for the zero packet and releases the handle. A stuck system
    /// I/O call is reported; a new worker cannot open the same seat concurrently.
    pub fn stop(&mut self) -> Result<(), String> {
        self.state.active.store(false, Ordering::Release);
        if let Some(worker) = &self.worker {
            worker.thread().unpark();
            let deadline = Instant::now() + Duration::from_millis(350);
            while !worker.is_finished() && Instant::now() < deadline {
                thread::sleep(Duration::from_millis(5));
            }
            if !worker.is_finished() {
                return Err(
                    "USB не подтвердил остановку. Выключите накидку кнопкой питания.".into(),
                );
            }
        }
        if let Some(worker) = self.worker.take() {
            worker.join().map_err(|_| "Ошибка USB-потока".to_owned())?;
        }
        Ok(())
    }

    pub fn is_active(&self) -> bool {
        self.state.active.load(Ordering::Acquire)
            && self
                .worker
                .as_ref()
                .is_some_and(|worker| !worker.is_finished())
    }
    pub fn levels(&self) -> [f32; 6] {
        self.state.levels.lock().map(|x| *x).unwrap_or([0.0; 6])
    }
    pub fn poll_error(&self) -> Option<String> {
        self.errors.try_recv().ok()
    }
}

impl Drop for Usb {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

trait Sink {
    fn send(&mut self, report: [u8; 9]) -> Result<(), String>;
}
impl Sink for HidDevice {
    fn send(&mut self, report: [u8; 9]) -> Result<(), String> {
        let written = self.write(&report).map_err(|e| e.to_string())?;
        if written != report.len() {
            return Err(format!("Неполная USB-запись: {written}/{}", report.len()));
        }
        Ok(())
    }
}

struct StopOnDrop<T: Sink>(T);
impl<T: Sink> Drop for StopOnDrop<T> {
    fn drop(&mut self) {
        let _ = self.0.send(protocol::encode([0; 6], [0; 2]));
    }
}

fn run_worker<T: Sink>(transport: T, state: &State, duration: Duration) -> Result<(), String> {
    let mut transport = StopOnDrop(transport);
    transport.0.send(protocol::encode([0; 6], [0; 2]))?;
    let started = Instant::now();
    let result = (|| {
        while state.active.load(Ordering::Acquire) && started.elapsed() < duration {
            let heartbeat = state
                .heartbeat
                .lock()
                .map_err(|_| "Ошибка контроля USB-потока")?;
            if SystemTime::now()
                .duration_since(*heartbeat)
                .unwrap_or_default()
                >= Duration::from_millis(1500)
            {
                break;
            }
            drop(heartbeat);
            let config = *state.config.lock().map_err(|_| "Ошибка настроек USB")?;
            let levels = signal::motor_levels(
                config.pattern,
                started.elapsed().as_secs_f64(),
                config.speed as f64,
                config.strength.min(100) as f32 / 100.0,
                duration.as_secs_f64(),
                config.zones,
            );
            let amplitudes = levels.map(|level| (level * 255.0).round() as u8);
            if !state.active.load(Ordering::Acquire) {
                break;
            }
            transport.0.send(protocol::encode(amplitudes, [0; 2]))?;
            *state.levels.lock().map_err(|_| "Ошибка отображения зон")? = levels;
            thread::park_timeout(Duration::from_millis(20));
        }
        Ok::<_, String>(())
    })();
    state.active.store(false, Ordering::Release);
    // Explicit Stop reports failure to the GUI; Drop also retries the zero packet.
    let stopped = transport.0.send(protocol::encode([0; 6], [0; 2]));
    result.and(stopped)
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fake(Arc<Mutex<Vec<[u8; 9]>>>);
    impl Sink for Fake {
        fn send(&mut self, report: [u8; 9]) -> Result<(), String> {
            self.0.lock().unwrap().push(report);
            Ok(())
        }
    }

    #[test]
    fn stale_heartbeat_sends_only_zero_and_never_resumes() {
        let packets = Arc::new(Mutex::new(Vec::new()));
        let state = State::default();
        state.active.store(true, Ordering::Release);
        *state.heartbeat.lock().unwrap() = SystemTime::now() - Duration::from_secs(2);
        run_worker(Fake(packets.clone()), &state, Duration::from_secs(10)).unwrap();
        assert!(!state.active.load(Ordering::Acquire));
        assert!(
            packets
                .lock()
                .unwrap()
                .iter()
                .all(|p| *p == protocol::encode([0; 6], [0; 2]))
        );
    }

    #[test]
    fn expiry_emits_stop_after_a_live_session() {
        let packets = Arc::new(Mutex::new(Vec::new()));
        let state = State::default();
        state.active.store(true, Ordering::Release);
        *state.config.lock().unwrap() = Config {
            pattern: Pattern::Steady,
            strength: 100,
            ..Config::default()
        };
        run_worker(Fake(packets.clone()), &state, Duration::from_millis(120)).unwrap();
        let packets = packets.lock().unwrap();
        let zero = protocol::encode([0; 6], [0; 2]);
        assert_eq!(packets.first(), Some(&zero));
        assert_eq!(packets.last(), Some(&zero));
        assert!(packets.iter().any(|p| *p != zero));
        assert!(!state.active.load(Ordering::Acquire));
    }

    #[test]
    fn failed_write_still_attempts_to_send_stop() {
        struct Failing {
            packets: Arc<Mutex<Vec<[u8; 9]>>>,
        }
        impl Sink for Failing {
            fn send(&mut self, report: [u8; 9]) -> Result<(), String> {
                let mut packets = self.packets.lock().unwrap();
                packets.push(report);
                if packets.len() == 2 {
                    Err("simulated disconnect".into())
                } else {
                    Ok(())
                }
            }
        }
        let packets = Arc::new(Mutex::new(Vec::new()));
        let state = State::default();
        state.active.store(true, Ordering::Release);
        assert!(
            run_worker(
                Failing {
                    packets: packets.clone()
                },
                &state,
                Duration::from_secs(30)
            )
            .is_err()
        );
        assert_eq!(
            packets.lock().unwrap().last(),
            Some(&protocol::encode([0; 6], [0; 2]))
        );
        assert!(!state.active.load(Ordering::Acquire));
    }
}

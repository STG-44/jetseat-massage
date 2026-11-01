//! HIDAPI's macOS manager retains the initializing thread's CFRunLoop.
//! That thread must outlive every enumeration, open and close. A mutex around
//! short-lived scan threads does not preserve that lifetime (SIGILL in IOKit).
use hidapi::HidApi;
use std::sync::{OnceLock, mpsc};

type Job = Box<dyn FnOnce(&mut Result<HidApi, String>) + Send>;
static SERVICE: OnceLock<mpsc::Sender<Job>> = OnceLock::new();

pub(crate) fn with_api<T: Send + 'static>(
    operation: impl FnOnce(&mut HidApi) -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    let service = SERVICE.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<Job>();
        std::thread::Builder::new()
            .name("jetseat-hid-owner".into())
            .spawn(move || {
                let mut api = HidApi::new().map_err(|error| error.to_string());
                for job in rx {
                    job(&mut api);
                }
            })
            .expect("start HID owner thread");
        tx
    });
    let (tx, rx) = mpsc::channel();
    service
        .send(Box::new(move |api| {
            let result = match api {
                Ok(api) => operation(api),
                Err(error) => Err(error.clone()),
            };
            let _ = tx.send(result);
        }))
        .map_err(|_| "USB-поток недоступен. Перезапустите приложение.".to_owned())?;
    rx.recv()
        .map_err(|_| "USB-поток завершился. Перезапустите приложение.".to_owned())?
}

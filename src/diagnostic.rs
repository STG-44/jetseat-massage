use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct HidInfo {
    pub vendor_id: u16,
    pub product_id: u16,
    pub product: String,
    pub manufacturer: String,
    pub usage_page: u16,
    pub usage: u16,
    pub interface: i32,
    pub candidate: bool,
    pub known_protocol: bool,
}

/// Enumeration only: does not open devices, send reports, or claim compatibility.
pub fn scan() -> Result<Vec<HidInfo>, String> {
    crate::hid_service::with_api(|api| {
        api.refresh_devices().map_err(|e| e.to_string())?;
        let mut devices: Vec<_> = api
            .device_list()
            .map(|device| {
                let product = device.product_string().unwrap_or("").to_owned();
                let manufacturer = device.manufacturer_string().unwrap_or("").to_owned();
                let text = format!("{product} {manufacturer}").to_lowercase();
                HidInfo {
                    vendor_id: device.vendor_id(),
                    product_id: device.product_id(),
                    product,
                    manufacturer,
                    usage_page: device.usage_page(),
                    usage: device.usage(),
                    interface: device.interface_number(),
                    candidate: ["gametrix", "jetseat", "uberwoorf"]
                        .iter()
                        .any(|term| text.contains(term)),
                    known_protocol: crate::protocol::supported_id(
                        device.vendor_id(),
                        device.product_id(),
                    ),
                }
            })
            .collect();
        devices.sort_by_key(|d| (!d.candidate, d.vendor_id, d.product_id, d.interface));
        Ok(devices)
    })
}

pub fn report(devices: &[HidInfo]) -> Result<String, String> {
    serde_json::to_string_pretty(&serde_json::json!({
        "application": "JetSeat Massage",
        "version": env!("CARGO_PKG_VERSION"),
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "usb_motor_control": "implemented_original_uwapi_protocol_hardware_test_pending",
        "devices": devices,
    }))
    .map_err(|e| e.to_string())
}

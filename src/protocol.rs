//! Gametrix 9-byte output report, independently reconstructed from the original
//! uwApi.dll. Original packet oracle and evidence are documented in PROTOCOL.md.

const KEY: [u32; 8] = [
    0x9f3be6c9, 0x4502ab6f, 0x80944295, 0xe0f578ec, 0x1907fa1f, 0x439e493a, 0x6401749d, 0x99447f2b,
];

pub fn supported_id(vendor: u16, product: u16) -> bool {
    matches!(
        (vendor, product),
        (0x16c0, 0x0613 | 0x0125) | (0x231d, 0x2001 | 0x2002)
    )
}

pub fn encode(motors: [u8; 6], salt: [u8; 2]) -> [u8; 9] {
    let mut plain = [0_u8; 8];
    plain[..6].copy_from_slice(&motors);
    plain[6..].copy_from_slice(&salt);
    let mut low = u32::from_le_bytes(plain[..4].try_into().unwrap());
    let mut high = u32::from_le_bytes(plain[4..].try_into().unwrap());
    for round in 0..64_u32 {
        let input = if round % 2 == 0 { low } else { high };
        let term = (input >> 8 ^ input.wrapping_shl(6))
            .wrapping_add(KEY[round as usize % 8])
            .wrapping_add(round)
            .wrapping_add(input);
        if round % 2 == 0 {
            high = high.wrapping_add(term);
        } else {
            low = low.wrapping_add(term);
        }
    }
    let mut report = [0_u8; 9];
    report[1..5].copy_from_slice(&low.to_le_bytes());
    report[5..9].copy_from_slice(&high.to_le_bytes());
    report
}

/// Only the unnumbered 64-bit Output report used by the original protocol is
/// accepted. Global PUSH/POP are tracked; malformed descriptors are rejected.
pub fn validate_descriptor(bytes: &[u8]) -> Result<(), String> {
    let mut pos = 0;
    let mut size = 0_u32;
    let mut count = 0_u32;
    let mut stack = Vec::new();
    let mut output_bits = 0_u32;
    while pos < bytes.len() {
        let prefix = bytes[pos];
        pos += 1;
        if prefix == 0xfe {
            return Err("HID Long Item не поддерживается".into());
        }
        let length = match prefix & 3 {
            3 => 4,
            n => n as usize,
        };
        if pos + length > bytes.len() {
            return Err("Повреждённый HID-дескриптор".into());
        }
        let mut raw = [0_u8; 4];
        raw[..length].copy_from_slice(&bytes[pos..pos + length]);
        pos += length;
        let value = u32::from_le_bytes(raw);
        match prefix & 0xfc {
            0x74 => size = value,
            0x94 => count = value,
            0x84 => return Err("Номерные HID-отчёты не соответствуют протоколу KW-901".into()),
            0xa4 => stack.push((size, count)),
            0xb4 => (size, count) = stack.pop().ok_or("HID POP без PUSH")?,
            0x90 => {
                let bits = size.checked_mul(count).ok_or("Слишком большой HID-отчёт")?;
                output_bits = output_bits
                    .checked_add(bits)
                    .ok_or("Слишком большой HID-отчёт")?;
            }
            _ => {}
        }
    }
    if output_bits != 64 {
        return Err(format!(
            "Ожидался Output report 64 бит, получено {output_bits}"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packets_match_original_dll_execution() {
        let json: serde_json::Value =
            serde_json::from_str(include_str!("../tests/usb-golden-vectors.json")).unwrap();
        for vector in json["vectors"].as_array().unwrap() {
            let values = |key: &str| -> Vec<u8> {
                vector[key]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|x| x.as_u64().unwrap() as u8)
                    .collect()
            };
            assert_eq!(
                encode(
                    values("motors").try_into().unwrap(),
                    values("salt").try_into().unwrap()
                )
                .as_slice(),
                values("report")
            );
        }
    }

    #[test]
    fn rejects_other_hardware_and_report_shapes() {
        assert!(supported_id(0x231d, 0x2001));
        assert!(!supported_id(0x046d, 0xc33a));
        assert!(validate_descriptor(&[0x75, 8, 0x95, 8, 0x91, 2]).is_ok());
        assert!(validate_descriptor(&[0x75, 8, 0x95, 9, 0x91, 2]).is_err());
        assert!(validate_descriptor(&[0x85, 1, 0x75, 8, 0x95, 8, 0x91, 2]).is_err());
        assert!(validate_descriptor(&[0x75]).is_err());
        assert!(validate_descriptor(&[0xb4]).is_err());
    }
}

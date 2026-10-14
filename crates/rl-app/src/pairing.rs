//! Pairing: the URL (with the token in the fragment) and its QR code.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use qrcode::render::{svg, unicode};
use qrcode::QrCode;
use serde::Serialize;
use ts_rs::TS;

use crate::platform::LanAddresses;

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct PairingInfo {
    pub url: String,
    pub port: u16,
    pub addresses: Vec<String>,
    pub qr_svg: String,
}

/// The fragment (`#t=`) is never sent to the server, so the token stays out of request logs.
pub fn pairing_url(ip: IpAddr, port: u16, token: &str) -> String {
    format!("http://{}/#t={token}", SocketAddr::new(ip, port))
}

pub fn qr_svg(text: &str) -> Result<String, String> {
    let code = QrCode::new(text.as_bytes()).map_err(|e| e.to_string())?;
    Ok(code
        .render::<svg::Color<'_>>()
        .min_dimensions(240, 240)
        .quiet_zone(true)
        .build())
}

pub fn qr_text(text: &str) -> Result<String, String> {
    let code = QrCode::new(text.as_bytes()).map_err(|e| e.to_string())?;
    Ok(code.render::<unicode::Dense1x2>().quiet_zone(true).build())
}

pub fn pairing_info(lan: &LanAddresses, port: u16, token: &str) -> Result<PairingInfo, String> {
    let ip = lan
        .preferred
        .or_else(|| lan.all.first().copied())
        .unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST));
    let url = pairing_url(ip, port, token);
    Ok(PairingInfo {
        qr_svg: qr_svg(&url)?,
        addresses: lan
            .all
            .iter()
            .map(|ip| format!("http://{}/", SocketAddr::new(*ip, port)))
            .collect(),
        port,
        url,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn url_carries_the_token_in_the_fragment() {
        let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 20));
        assert_eq!(
            pairing_url(ip, 49_321, "abc"),
            "http://192.168.1.20:49321/#t=abc"
        );
        let info = pairing_info(
            &LanAddresses {
                preferred: Some(ip),
                all: vec![ip],
            },
            49_321,
            "abc",
        )
        .unwrap();
        assert!(info.qr_svg.contains("<svg"));
        assert_eq!(info.addresses, ["http://192.168.1.20:49321/"]);
        assert!(qr_text(&info.url).unwrap().lines().count() > 10);
    }
}

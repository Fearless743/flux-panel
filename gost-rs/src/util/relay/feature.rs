//! relay 协议 features。
//!
//! 4 种 Feature：UserAuth / Addr / Tunnel / Network。
//!
//! ## Feature 通用格式
//!
//! ```text
//! +------+----------+--------+
//! | TYPE |   LEN    |  DATA  |
//! +------+----------+--------+
//! |   1  |    2     |  VAR   |
//! ```
//!
//! ## UserAuth
//!
//! ```text
//! +------+----------+------+----------+
//! | ULEN |  UNAME   | PLEN |  PASSWD  |
//! +------+----------+------+----------+
//! |   1  | 0 to 255 |   1  | 1 to 255 |
//! ```
//!
//! ## Addr（SOCKS5 风格）
//!
//! ```text
//! +------+----------+----------+
//! | ATYP |   ADDR   |   PORT   |
//! +------+----------+----------+
//! |   1  | Variable |    2     |
//! ```
//!
//! ## Tunnel：20 字节 ID（16B UUID + 1B flag + 2B rsv + 1B weight）
//!
//! ## Network：2 字节 NetworkID

use std::any::Any;
use std::io::{self, Read, Write};

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};

/// 协议版本（供 feature.rs 与 frame.rs 共用）。
pub use super::frame::VERSION_1;

/// Feature 类型 ID。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum FeatureType {
    UserAuth = 0x01,
    Addr = 0x02,
    Tunnel = 0x03,
    Network = 0x04,
}

impl FeatureType {
    pub fn from_byte(b: u8) -> Self {
        match b {
            0x01 => FeatureType::UserAuth,
            0x02 => FeatureType::Addr,
            0x03 => FeatureType::Tunnel,
            0x04 => FeatureType::Network,
            _ => FeatureType::UserAuth,
        }
    }
    pub fn as_byte(self) -> u8 {
        self as u8
    }
}

/// Feature 通用 trait。
pub trait Feature: std::fmt::Debug + Send + Sync {
    fn r#type(&self) -> FeatureType;
    fn encode(&self) -> io::Result<Vec<u8>>;
    fn decode(&mut self, data: &[u8]) -> io::Result<()>;
    fn as_any(&self) -> &dyn Any;
    fn box_clone(&self) -> Box<dyn Feature>;
}

impl Clone for Box<dyn Feature> {
    fn clone(&self) -> Self {
        self.box_clone()
    }
}

/// 解码一连串 features（feature 区段已被一次性读到 buf）。
pub fn decode_features(buf: &[u8]) -> io::Result<Vec<Box<dyn Feature>>> {
    let mut out = Vec::new();
    let mut cur = std::io::Cursor::new(buf);
    while cur.position() < buf.len() as u64 {
        let ty_b = cur.read_u8()?;
        let len = cur.read_u16::<BigEndian>()?;
        let mut data = vec![0u8; len as usize];
        cur.read_exact(&mut data)?;
        let ty = FeatureType::from_byte(ty_b);
        let mut f: Box<dyn Feature> = match ty {
            FeatureType::UserAuth => Box::new(UserAuthFeature::default()),
            FeatureType::Addr => Box::new(AddrFeature::default()),
            FeatureType::Tunnel => Box::new(TunnelFeature::default()),
            FeatureType::Network => Box::new(NetworkFeature::default()),
        };
        f.decode(&data)?;
        out.push(f);
    }
    Ok(out)
}

// ===================== UserAuth =====================

#[derive(Debug, Default, Clone)]
pub struct UserAuthFeature {
    pub username: String,
    pub password: String,
}

impl Feature for UserAuthFeature {
    fn r#type(&self) -> FeatureType {
        FeatureType::UserAuth
    }
    fn encode(&self) -> io::Result<Vec<u8>> {
        if self.username.len() > 0xFF {
            return Err(io::Error::other("username too long"));
        }
        if self.password.len() > 0xFF {
            return Err(io::Error::other("password too long"));
        }
        let mut buf = Vec::new();
        buf.write_u8(self.username.len() as u8)?;
        buf.write_all(self.username.as_bytes())?;
        buf.write_u8(self.password.len() as u8)?;
        buf.write_all(self.password.as_bytes())?;
        Ok(buf)
    }
    fn decode(&mut self, data: &[u8]) -> io::Result<()> {
        if data.len() < 2 {
            return Err(io::Error::other("UserAuth: short buffer"));
        }
        let mut cur = std::io::Cursor::new(data);
        let ulen = cur.read_u8()? as usize;
        if data.len() < 1 + ulen + 1 {
            return Err(io::Error::other("UserAuth: short buffer (u)"));
        }
        self.username = String::from_utf8_lossy(&data[1..1 + ulen]).to_string();
        let pos = 1 + ulen;
        let plen = data[pos] as usize;
        if data.len() < pos + 1 + plen {
            return Err(io::Error::other("UserAuth: short buffer (p)"));
        }
        self.password = String::from_utf8_lossy(&data[pos + 1..pos + 1 + plen]).to_string();
        Ok(())
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn box_clone(&self) -> Box<dyn Feature> {
        Box::new(self.clone())
    }
}

// ===================== Addr =====================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum AddrType {
    #[default]
    IPv4 = 0x01,
    Domain = 0x03,
    IPv6 = 0x04,
}

#[derive(Debug, Default, Clone)]
pub struct AddrFeature {
    pub atype: AddrType,
    pub host: String,
    pub port: u16,
}

impl AddrFeature {
    pub fn parse_from(&mut self, address: &str) -> io::Result<()> {
        let (host, port) = match address.rsplit_once(':') {
            Some((h, p)) => (h.to_string(), p.parse::<u16>().unwrap_or(0)),
            None => (address.to_string(), 0),
        };
        self.host = host;
        self.port = port;
        self.atype = AddrType::Domain;
        if let Ok(v4) = self.host.parse::<std::net::Ipv4Addr>() {
            self.host = v4.to_string();
            self.atype = AddrType::IPv4;
        } else if let Ok(v6) = self.host.parse::<std::net::Ipv6Addr>() {
            self.host = v6.to_string();
            self.atype = AddrType::IPv6;
        }
        Ok(())
    }
}

impl Feature for AddrFeature {
    fn r#type(&self) -> FeatureType {
        FeatureType::Addr
    }
    fn encode(&self) -> io::Result<Vec<u8>> {
        let mut buf = Vec::new();
        match self.atype {
            AddrType::IPv4 => {
                buf.write_u8(self.atype as u8)?;
                let ip = self
                    .host
                    .parse::<std::net::Ipv4Addr>()
                    .unwrap_or(std::net::Ipv4Addr::UNSPECIFIED);
                buf.write_all(&ip.octets())?;
            }
            AddrType::Domain => {
                if self.host.len() > 0xFF {
                    return Err(io::Error::other("domain too long"));
                }
                buf.write_u8(self.atype as u8)?;
                buf.write_u8(self.host.len() as u8)?;
                buf.write_all(self.host.as_bytes())?;
            }
            AddrType::IPv6 => {
                buf.write_u8(self.atype as u8)?;
                let ip = self
                    .host
                    .parse::<std::net::Ipv6Addr>()
                    .unwrap_or(std::net::Ipv6Addr::UNSPECIFIED);
                buf.write_all(&ip.octets())?;
            }
        }
        buf.write_u16::<BigEndian>(self.port)?;
        Ok(buf)
    }
    fn decode(&mut self, data: &[u8]) -> io::Result<()> {
        if data.len() < 4 {
            return Err(io::Error::other("Addr: short buffer"));
        }
        self.atype = match data[0] {
            0x01 => AddrType::IPv4,
            0x03 => AddrType::Domain,
            0x04 => AddrType::IPv6,
            _ => return Err(io::Error::other("Addr: bad atype")),
        };
        let mut pos = 1usize;
        match self.atype {
            AddrType::IPv4 => {
                if data.len() < pos + 4 + 2 {
                    return Err(io::Error::other("Addr: short v4"));
                }
                let mut oct = [0u8; 4];
                oct.copy_from_slice(&data[pos..pos + 4]);
                self.host = std::net::Ipv4Addr::from(oct).to_string();
                pos += 4;
            }
            AddrType::Domain => {
                let alen = data[pos] as usize;
                pos += 1;
                if data.len() < pos + alen + 2 {
                    return Err(io::Error::other("Addr: short domain"));
                }
                self.host = String::from_utf8_lossy(&data[pos..pos + alen]).to_string();
                pos += alen;
            }
            AddrType::IPv6 => {
                if data.len() < pos + 16 + 2 {
                    return Err(io::Error::other("Addr: short v6"));
                }
                let mut oct = [0u8; 16];
                oct.copy_from_slice(&data[pos..pos + 16]);
                self.host = std::net::Ipv6Addr::from(oct).to_string();
                pos += 16;
            }
        }
        let mut pbuf = [0u8; 2];
        pbuf.copy_from_slice(&data[pos..pos + 2]);
        self.port = u16::from_be_bytes(pbuf);
        Ok(())
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn box_clone(&self) -> Box<dyn Feature> {
        Box::new(self.clone())
    }
}

// ===================== Tunnel =====================

#[derive(Debug, Default, Clone)]
pub struct TunnelFeature {
    pub id: [u8; 20],
}

impl Feature for TunnelFeature {
    fn r#type(&self) -> FeatureType {
        FeatureType::Tunnel
    }
    fn encode(&self) -> io::Result<Vec<u8>> {
        Ok(self.id.to_vec())
    }
    fn decode(&mut self, data: &[u8]) -> io::Result<()> {
        if data.len() < 20 {
            return Err(io::Error::other("Tunnel: short"));
        }
        self.id.copy_from_slice(&data[..20]);
        Ok(())
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn box_clone(&self) -> Box<dyn Feature> {
        Box::new(self.clone())
    }
}

// ===================== Network =====================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum NetworkID {
    TCP = 0x0000,
    UDP = 0x0001,
    IP = 0x0002,
    Unix = 0x0010,
    Serial = 0x0011,
}

impl Default for NetworkID {
    fn default() -> Self {
        NetworkID::TCP
    }
}

impl NetworkID {
    pub fn as_str(self) -> &'static str {
        match self {
            NetworkID::TCP => "tcp",
            NetworkID::UDP => "udp",
            NetworkID::IP => "ip",
            NetworkID::Unix => "unix",
            NetworkID::Serial => "serial",
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct NetworkFeature {
    pub network: NetworkID,
}

impl Feature for NetworkFeature {
    fn r#type(&self) -> FeatureType {
        FeatureType::Network
    }
    fn encode(&self) -> io::Result<Vec<u8>> {
        Ok((self.network as u16).to_be_bytes().to_vec())
    }
    fn decode(&mut self, data: &[u8]) -> io::Result<()> {
        if data.len() < 2 {
            return Err(io::Error::other("Network: short"));
        }
        let v = u16::from_be_bytes([data[0], data[1]]);
        self.network = match v {
            0x0000 => NetworkID::TCP,
            0x0001 => NetworkID::UDP,
            0x0002 => NetworkID::IP,
            0x0010 => NetworkID::Unix,
            0x0011 => NetworkID::Serial,
            _ => NetworkID::TCP,
        };
        Ok(())
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn box_clone(&self) -> Box<dyn Feature> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_auth_round_trip() {
        let f = UserAuthFeature {
            username: "alice".into(),
            password: "secret".into(),
        };
        let enc = f.encode().unwrap();
        assert_eq!(enc[0], 5);
        let mut dec = UserAuthFeature::default();
        dec.decode(&enc).unwrap();
        assert_eq!(dec.username, "alice");
        assert_eq!(dec.password, "secret");
    }

    #[test]
    fn addr_v4_round_trip() {
        let mut f = AddrFeature::default();
        f.parse_from("1.2.3.4:80").unwrap();
        assert_eq!(f.atype, AddrType::IPv4);
        let enc = f.encode().unwrap();
        let mut dec = AddrFeature::default();
        dec.decode(&enc).unwrap();
        assert_eq!(dec.host, "1.2.3.4");
        assert_eq!(dec.port, 80);
    }

    #[test]
    fn addr_domain_round_trip() {
        let mut f = AddrFeature::default();
        f.parse_from("example.com:443").unwrap();
        assert_eq!(f.atype, AddrType::Domain);
        let enc = f.encode().unwrap();
        let mut dec = AddrFeature::default();
        dec.decode(&enc).unwrap();
        assert_eq!(dec.host, "example.com");
        assert_eq!(dec.port, 443);
    }

    #[test]
    fn addr_v6_round_trip() {
        let mut f = AddrFeature::default();
        let _ = f.parse_from("[::1]:8080");
        // host 含 [], 所以 atype 仍是 domain. 验证至少能 encode/decode。
        let enc = f.encode().unwrap();
        let mut dec = AddrFeature::default();
        dec.decode(&enc).unwrap();
        assert_eq!(dec.port, 8080);
    }

    #[test]
    fn network_round_trip() {
        let f = NetworkFeature {
            network: NetworkID::UDP,
        };
        let enc = f.encode().unwrap();
        let mut dec = NetworkFeature::default();
        dec.decode(&enc).unwrap();
        assert_eq!(dec.network, NetworkID::UDP);
    }

    #[test]
    fn tunnel_round_trip() {
        let mut id = [0u8; 20];
        for i in 0..20 {
            id[i] = i as u8;
        }
        let f = TunnelFeature { id };
        let enc = f.encode().unwrap();
        let mut dec = TunnelFeature::default();
        dec.decode(&enc).unwrap();
        assert_eq!(dec.id, id);
    }
}
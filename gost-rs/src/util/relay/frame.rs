//! relay 帧：Request / Response。

use std::io::{self, Read, Write};

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};

use super::feature::Feature;

/// 协议版本。
pub const VERSION_1: u8 = 0x01;

/// relay 命令类型（与 flags 共享一个字节：低 4 位 cmd，高 4 位 flags）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Cmd {
    Connect = 0x01,
    Bind = 0x02,
    Associate = 0x03,
}

impl Cmd {
    pub fn from_byte(b: u8) -> Self {
        match b & 0x0F {
            0x01 => Cmd::Connect,
            0x02 => Cmd::Bind,
            0x03 => Cmd::Associate,
            _ => Cmd::Connect,
        }
    }
    pub fn as_byte(self) -> u8 {
        self as u8
    }
}

/// UDP 标志（高 4 位 flags 中的一位）。
pub const FUDP: u8 = 0x80;

/// 响应状态码。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Status {
    Ok = 0x00,
    BadRequest = 0x01,
    Unauthorized = 0x02,
    Forbidden = 0x03,
    Timeout = 0x04,
    ServiceUnavailable = 0x05,
    HostUnreachable = 0x06,
    NetworkUnreachable = 0x07,
    InternalServerError = 0x08,
}

impl Status {
    pub fn from_byte(b: u8) -> Self {
        match b {
            0x00 => Status::Ok,
            0x01 => Status::BadRequest,
            0x02 => Status::Unauthorized,
            0x03 => Status::Forbidden,
            0x04 => Status::Timeout,
            0x05 => Status::ServiceUnavailable,
            0x06 => Status::HostUnreachable,
            0x07 => Status::NetworkUnreachable,
            0x08 => Status::InternalServerError,
            _ => Status::InternalServerError,
        }
    }
    pub fn as_byte(self) -> u8 {
        self as u8
    }
    pub fn text(self) -> &'static str {
        match self {
            Status::Ok => "OK",
            Status::BadRequest => "Bad Request",
            Status::Unauthorized => "Unauthorized",
            Status::Forbidden => "Forbidden",
            Status::Timeout => "Timeout",
            Status::ServiceUnavailable => "Service Unavailable",
            Status::HostUnreachable => "Host Unreachable",
            Status::NetworkUnreachable => "Network Unreachable",
            Status::InternalServerError => "Internal Server Error",
        }
    }
}

// ===================== Request =====================

/// 下行 Request 帧。
#[derive(Debug, Clone, Default)]
pub struct Request {
    pub version: u8,
    pub cmd: u8,
    pub features: Vec<Box<dyn Feature>>,
}

impl Request {
    pub fn new(cmd: Cmd, flags: u8) -> Self {
        Self {
            version: VERSION_1,
            cmd: cmd.as_byte() | (flags & 0xF0),
            features: Vec::new(),
        }
    }

    pub fn add_feature<F: Feature + 'static>(&mut self, f: F) {
        self.features.push(Box::new(f));
    }

    pub fn read_from(r: &mut dyn Read) -> io::Result<Self> {
        let ver = r.read_u8()?;
        if ver != VERSION_1 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "bad relay version"));
        }
        let cmd = r.read_u8()?;
        let flen = r.read_u16::<BigEndian>()?;
        let mut buf = vec![0u8; flen as usize];
        r.read_exact(&mut buf)?;
        let features = super::feature::decode_features(&buf)?;
        Ok(Self {
            version: ver,
            cmd,
            features,
        })
    }

    pub fn write_to(&self, w: &mut dyn Write) -> io::Result<()> {
        let mut features_buf: Vec<u8> = Vec::new();
        for f in &self.features {
            let data = f.encode().map_err(io::Error::other)?;
            features_buf.write_u8(f.r#type().as_byte())?;
            features_buf.write_u16::<BigEndian>(data.len() as u16)?;
            features_buf.write_all(&data)?;
        }
        w.write_u8(self.version)?;
        w.write_u8(self.cmd)?;
        w.write_u16::<BigEndian>(features_buf.len() as u16)?;
        w.write_all(&features_buf)?;
        Ok(())
    }

    /// 异步读取（需 tokio）。
    pub async fn read_from_async<R>(r: &mut R) -> io::Result<Self>
    where
        R: tokio::io::AsyncReadExt + Unpin,
    {
        let mut header = [0u8; 4];
        r.read_exact(&mut header).await?;
        if header[0] != VERSION_1 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "bad relay version"));
        }
        let flen = u16::from_be_bytes([header[2], header[3]]) as usize;
        let mut buf = vec![0u8; flen];
        r.read_exact(&mut buf).await?;
        let features = super::feature::decode_features(&buf)?;
        Ok(Self {
            version: header[0],
            cmd: header[1],
            features,
        })
    }

    /// 异步写入（需 tokio）。
    pub async fn write_to_async<W>(&self, w: &mut W) -> io::Result<()>
    where
        W: tokio::io::AsyncWriteExt + Unpin,
    {
        use tokio::io::AsyncWriteExt;
        let mut features_buf: Vec<u8> = Vec::new();
        for f in &self.features {
            let data = f.encode().map_err(io::Error::other)?;
            features_buf.push(f.r#type().as_byte());
            let len_be = (data.len() as u16).to_be_bytes();
            features_buf.extend_from_slice(&len_be);
            features_buf.extend_from_slice(&data);
        }
        w.write_u8(self.version).await?;
        w.write_u8(self.cmd).await?;
        w.write_all(&(features_buf.len() as u16).to_be_bytes()).await?;
        w.write_all(&features_buf).await?;
        Ok(())
    }
}

// ===================== Response =====================

/// 上行 Response 帧。
#[derive(Debug, Clone, Default)]
pub struct Response {
    pub version: u8,
    pub status: u8,
    pub features: Vec<Box<dyn Feature>>,
}

impl Response {
    pub fn new(status: Status) -> Self {
        Self {
            version: VERSION_1,
            status: status.as_byte(),
            features: Vec::new(),
        }
    }

    pub fn ok() -> Self {
        Self::new(Status::Ok)
    }

    pub fn add_feature<F: Feature + 'static>(&mut self, f: F) {
        self.features.push(Box::new(f));
    }

    pub fn read_from(r: &mut dyn Read) -> io::Result<Self> {
        let ver = r.read_u8()?;
        if ver != VERSION_1 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "bad relay version"));
        }
        let status = r.read_u8()?;
        let flen = r.read_u16::<BigEndian>()?;
        let mut buf = vec![0u8; flen as usize];
        r.read_exact(&mut buf)?;
        let features = super::feature::decode_features(&buf)?;
        Ok(Self {
            version: ver,
            status,
            features,
        })
    }

    pub fn write_to(&self, w: &mut dyn Write) -> io::Result<()> {
        let mut features_buf: Vec<u8> = Vec::new();
        for f in &self.features {
            let data = f.encode().map_err(io::Error::other)?;
            features_buf.write_u8(f.r#type().as_byte())?;
            features_buf.write_u16::<BigEndian>(data.len() as u16)?;
            features_buf.write_all(&data)?;
        }
        w.write_u8(self.version)?;
        w.write_u8(self.status)?;
        w.write_u16::<BigEndian>(features_buf.len() as u16)?;
        w.write_all(&features_buf)?;
        Ok(())
    }

    /// 异步读取（需 tokio）。
    pub async fn read_from_async<R>(r: &mut R) -> io::Result<Self>
    where
        R: tokio::io::AsyncReadExt + Unpin,
    {
        let mut header = [0u8; 4];
        r.read_exact(&mut header).await?;
        if header[0] != VERSION_1 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "bad relay version"));
        }
        let flen = u16::from_be_bytes([header[2], header[3]]) as usize;
        let mut buf = vec![0u8; flen];
        r.read_exact(&mut buf).await?;
        let features = super::feature::decode_features(&buf)?;
        Ok(Self {
            version: header[0],
            status: header[1],
            features,
        })
    }

    /// 异步写入（需 tokio）。
    pub async fn write_to_async<W>(&self, w: &mut W) -> io::Result<()>
    where
        W: tokio::io::AsyncWriteExt + Unpin,
    {
        use tokio::io::AsyncWriteExt;
        let mut features_buf: Vec<u8> = Vec::new();
        for f in &self.features {
            let data = f.encode().map_err(io::Error::other)?;
            features_buf.push(f.r#type().as_byte());
            let len_be = (data.len() as u16).to_be_bytes();
            features_buf.extend_from_slice(&len_be);
            features_buf.extend_from_slice(&data);
        }
        w.write_u8(self.version).await?;
        w.write_u8(self.status).await?;
        w.write_all(&(features_buf.len() as u16).to_be_bytes()).await?;
        w.write_all(&features_buf).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::relay::feature::{AddrFeature, NetworkFeature, UserAuthFeature};

    #[test]
    fn request_round_trip_no_features() {
        let req = Request::new(Cmd::Connect, 0);
        let mut buf = Vec::new();
        req.write_to(&mut buf).unwrap();
        let mut cur = std::io::Cursor::new(buf);
        let parsed = Request::read_from(&mut cur).unwrap();
        assert_eq!(parsed.version, VERSION_1);
        assert_eq!(parsed.cmd & 0x0F, Cmd::Connect.as_byte());
        assert!(parsed.features.is_empty());
    }

    #[test]
    fn request_round_trip_with_addr_user_auth_network() {
        let mut req = Request::new(Cmd::Connect, FUDP);
        req.add_feature(UserAuthFeature {
            username: "user".into(),
            password: "pass".into(),
        });
        let mut addr = AddrFeature::default();
        addr.parse_from("127.0.0.1:8080").unwrap();
        req.add_feature(addr);
        req.add_feature(NetworkFeature {
            network: super::super::NetworkID::UDP,
        });
        let mut buf = Vec::new();
        req.write_to(&mut buf).unwrap();
        let mut cur = std::io::Cursor::new(buf);
        let parsed = Request::read_from(&mut cur).unwrap();
        assert_eq!(parsed.cmd & 0x0F, Cmd::Connect.as_byte());
        assert_eq!(parsed.cmd & FUDP, FUDP);
        assert_eq!(parsed.features.len(), 3);
    }

    #[test]
    fn response_round_trip() {
        let mut resp = Response::ok();
        resp.add_feature(NetworkFeature {
            network: super::super::NetworkID::TCP,
        });
        let mut buf = Vec::new();
        resp.write_to(&mut buf).unwrap();
        let mut cur = std::io::Cursor::new(buf);
        let parsed = Response::read_from(&mut cur).unwrap();
        assert_eq!(parsed.status, Status::Ok.as_byte());
        assert_eq!(parsed.features.len(), 1);
    }
}
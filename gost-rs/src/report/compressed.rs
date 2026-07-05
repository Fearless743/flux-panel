//! gzip 压缩/解压（与 Java springboot-backend GzipUtils 兼容）。
//!
//! Java 端用 `java.util.zip.GZIPOutputStream` / `GZIPInputStream`，
//! 默认头 `0x1f 0x8b 0x08`（magic1 + magic2 + deflate）。
//! Rust 端 `flate2` 默认行为与 Java 一致。

use anyhow::{Context, Result};
use flate2::read::{DeflateDecoder, GzDecoder};
use flate2::write::{DeflateEncoder, GzEncoder};
use flate2::Compression;
use std::io::{Read, Write};

/// gzip 压缩（与 Java GZIPOutputStream 一致）。
pub fn gzip(data: &[u8]) -> Result<Vec<u8>> {
    let mut enc = GzEncoder::new(Vec::new(), Compression::default());
    enc.write_all(data).context("gzip 写入失败")?;
    enc.finish().context("gzip 完成失败")
}

/// gzip 解压（与 Java GZIPInputStream 一致）。
pub fn gunzip(data: &[u8]) -> Result<Vec<u8>> {
    let mut dec = GzDecoder::new(data);
    let mut out = Vec::new();
    dec.read_to_end(&mut out).context("gzip 解压失败")?;
    Ok(out)
}

/// raw deflate 压缩（不带 gzip 头）。
pub fn deflate(data: &[u8]) -> Result<Vec<u8>> {
    let mut enc = DeflateEncoder::new(Vec::new(), Compression::default());
    enc.write_all(data).context("deflate 写入失败")?;
    enc.finish().context("deflate 完成失败")
}

/// raw deflate 解压。
pub fn inflate(data: &[u8]) -> Result<Vec<u8>> {
    let mut dec = DeflateDecoder::new(data);
    let mut out = Vec::new();
    dec.read_to_end(&mut out).context("deflate 解压失败")?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gzip_round_trip() {
        let data = b"hello world from gost-rs";
        let c = gzip(data).unwrap();
        assert!(c.len() >= 2);
        assert_eq!(c[0], 0x1f);
        assert_eq!(c[1], 0x8b);
        let d = gunzip(&c).unwrap();
        assert_eq!(d, data);
    }

    #[test]
    fn gzip_random() {
        let data: Vec<u8> = (0..1024).map(|i| (i % 251) as u8).collect();
        let c = gzip(&data).unwrap();
        let d = gunzip(&c).unwrap();
        assert_eq!(d, data);
    }

    #[test]
    fn deflate_round_trip() {
        let data = b"raw deflate";
        let c = deflate(data).unwrap();
        let d = inflate(&c).unwrap();
        assert_eq!(d, data);
    }
}
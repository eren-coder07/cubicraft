//! Minimal PNG encoder (RGBA8, filter 0, zlib "stored" blocks).
//! Zero-dependency — used for screenshots and tests.

use std::io::{self, Write};

fn crc32_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    for (n, e) in table.iter_mut().enumerate() {
        let mut c = n as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
        }
        *e = c;
    }
    table
}

fn crc32(data: &[u8]) -> u32 {
    let table = crc32_table();
    let mut c = 0xFFFF_FFFFu32;
    for &b in data {
        c = table[((c ^ b as u32) & 0xFF) as usize] ^ (c >> 8);
    }
    c ^ 0xFFFF_FFFF
}

fn chunk(w: &mut dyn Write, kind: &[u8; 4], data: &[u8]) -> io::Result<()> {
    w.write_all(&(data.len() as u32).to_be_bytes())?;
    let mut full = Vec::with_capacity(4 + data.len());
    full.extend_from_slice(kind);
    full.extend_from_slice(data);
    w.write_all(&full)?;
    w.write_all(&crc32(&full).to_be_bytes())?;
    Ok(())
}

fn adler32(data: &[u8]) -> u32 {
    let mut a: u32 = 1;
    let mut b: u32 = 0;
    for &byte in data {
        a = (a + byte as u32) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

/// Encode RGBA8 pixels as a PNG stream.
pub fn encode(w: &mut dyn Write, width: u32, height: u32, rgba: &[u8]) -> io::Result<()> {
    let stride = (width * 4) as usize;
    let mut raw = Vec::with_capacity((stride + 1) * height as usize);
    for y in 0..height as usize {
        raw.push(0u8);
        raw.extend_from_slice(&rgba[y * stride..(y + 1) * stride]);
    }
    // zlib stream with stored deflate blocks (valid, uncompressed)
    let mut z = Vec::with_capacity(raw.len() + raw.len() / 65535 * 5 + 16);
    z.push(0x78);
    z.push(0x01);
    let mut pos = 0usize;
    loop {
        let remain = raw.len() - pos;
        let n = remain.min(65535);
        let last = if pos + n >= raw.len() { 1u8 } else { 0u8 };
        z.push(last);
        z.extend_from_slice(&(n as u16).to_le_bytes());
        z.extend_from_slice(&(!(n as u16)).to_le_bytes());
        z.extend_from_slice(&raw[pos..pos + n]);
        pos += n;
        if last == 1 {
            break;
        }
    }
    z.extend_from_slice(&adler32(&raw).to_be_bytes());

    w.write_all(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A])?;
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.push(8); // bit depth
    ihdr.push(6); // color type: RGBA
    ihdr.push(0); // compression
    ihdr.push(0); // filter
    ihdr.push(0); // interlace
    chunk(w, b"IHDR", &ihdr)?;
    chunk(w, b"IDAT", &z)?;
    chunk(w, b"IEND", &[])?;
    Ok(())
}

/// Convenience wrapper writing a PNG to a file path.
pub fn write_png_file(
    path: &std::path::Path,
    width: u32,
    height: u32,
    rgba: &[u8],
) -> io::Result<()> {
    let f = std::fs::File::create(path)?;
    let mut w = std::io::BufWriter::new(f);
    encode(&mut w, width, height, rgba)?;
    w.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn png_header_ok() {
        let mut buf = Vec::new();
        encode(&mut buf, 4, 3, &[255u8; 4 * 4 * 3]).unwrap();
        assert_eq!(&buf[..8], &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]);
        let w = u32::from_be_bytes([buf[16], buf[17], buf[18], buf[19]]);
        assert_eq!(w, 4);
        let h = u32::from_be_bytes([buf[20], buf[21], buf[22], buf[23]]);
        assert_eq!(h, 3);
    }
}

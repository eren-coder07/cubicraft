//! Minimal Adobe Photoshop (PSD) file parser — big-endian, RGB/8-bit, layers
//! with raw or PackBits (RLE) channel compression. Clean-room implementation
//! from the publicly documented file format specification.

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Layer {
    pub name: String,
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
    pub visible: bool,
    pub opacity: u8,
    /// true when layer blend mode is multiply (`mltp`).
    pub multiply: bool,
    /// Straight (non-premultiplied) RGBA pixels.
    pub rgba: Option<Vec<u8>>,
}

impl Layer {
    pub fn width(&self) -> u32 {
        (self.right - self.left).max(0) as u32
    }
    pub fn height(&self) -> u32 {
        (self.bottom - self.top).max(0) as u32
    }
}

#[derive(Debug)]
pub struct Psd {
    pub width: u32,
    pub height: u32,
    pub layers: Vec<Layer>,
}

struct Reader<'a> {
    b: &'a [u8],
    p: usize,
}

impl<'a> Reader<'a> {
    fn new(b: &'a [u8]) -> Self {
        Self { b, p: 0 }
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        if self.p + n > self.b.len() {
            return Err("unexpected end of file".into());
        }
        let s = &self.b[self.p..self.p + n];
        self.p += n;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, String> {
        let s = self.take(2)?;
        Ok(u16::from_be_bytes([s[0], s[1]]))
    }
    fn i16(&mut self) -> Result<i16, String> {
        Ok(self.u16()? as i16)
    }
    fn u32(&mut self) -> Result<u32, String> {
        let s = self.take(4)?;
        Ok(u32::from_be_bytes([s[0], s[1], s[2], s[3]]))
    }
    fn i32(&mut self) -> Result<i32, String> {
        Ok(self.u32()? as i32)
    }
    fn skip(&mut self, n: usize) -> Result<(), String> {
        self.take(n)?;
        Ok(())
    }
    fn tag(&mut self) -> Result<[u8; 4], String> {
        let s = self.take(4)?;
        Ok([s[0], s[1], s[2], s[3]])
    }
}

fn packbits_decode(input: &[u8], expected: usize) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(expected);
    let mut i = 0;
    while i < input.len() {
        let n = input[i] as i8;
        i += 1;
        if n >= 0 {
            let count = n as usize + 1;
            if i + count > input.len() {
                return Err("packbits: literal overrun".into());
            }
            out.extend_from_slice(&input[i..i + count]);
            i += count;
        } else if n != -128 {
            let count = (-(n as i32)) as usize + 1;
            if i >= input.len() {
                return Err("packbits: repeat overrun".into());
            }
            let b = input[i];
            i += 1;
            for _ in 0..count {
                out.push(b);
            }
        }
        if out.len() > expected + 8 {
            break;
        }
    }
    if out.len() < expected {
        out.resize(expected, 0);
    }
    Ok(out)
}

impl Psd {
    pub fn parse(bytes: &[u8]) -> Result<Psd, String> {
        let mut r = Reader::new(bytes);
        if r.tag()? != *b"8BPS" {
            return Err("not a PSD file (bad signature)".into());
        }
        let version = r.u16()?;
        if version != 1 {
            return Err("only PSD (version 1) supported, not PSB".into());
        }
        r.skip(6); // reserved
        let _channels = r.u16()?;
        let height = r.u32()?;
        let width = r.u32()?;
        let depth = r.u16()?;
        if depth != 8 {
            return Err("only 8-bit depth supported".into());
        }
        let _mode = r.u16()?;

        let color_len = r.u32()? as usize;
        r.skip(color_len)?;
        let res_len = r.u32()? as usize;
        r.skip(res_len)?;

        let lm_len = r.u32()? as usize;
        let _lm_end = r.p + lm_len;
        if lm_len == 0 {
            return Ok(Psd { width, height, layers: Vec::new() });
        }
        let li_len = r.u32()? as usize;
        let _li_end = r.p + li_len.min(r.b.len() - r.p);

        let count = r.i16()?;
        let count = count.unsigned_abs() as usize;

        struct Rec {
            name: String,
            left: i32,
            top: i32,
            right: i32,
            bottom: i32,
            visible: bool,
            opacity: u8,
            multiply: bool,
            channels: Vec<(i16, u32)>,
        }

        let mut records: Vec<Rec> = Vec::with_capacity(count);
        for _ in 0..count {
            let top = r.i32()?;
            let left = r.i32()?;
            let bottom = r.i32()?;
            let right = r.i32()?;
            let nch = r.u16()? as usize;
            let mut channels = Vec::with_capacity(nch);
            for _ in 0..nch {
                let id = r.i16()?;
                let len = r.u32()?;
                channels.push((id, len));
            }
            if r.tag()? != *b"8BIM" {
                return Err("bad blend mode signature in layer record".into());
            }
            let blend = r.tag()?;
            let multiply = &blend == b"mltp";
            let opacity = r.u8()?;
            let _clipping = r.u8()?;
            let flags = r.u8()?;
            r.skip(1); // filler
            let extra_len = r.u32()? as usize;
            let extra_end = r.p + extra_len;
            let mask_len = r.u32()? as usize;
            r.skip(mask_len)?;
            let ranges_len = r.u32()? as usize;
            r.skip(ranges_len)?;
            // Pascal name (padded to 4-byte boundary including length byte)
            let nlen = r.u8()? as usize;
            let mut name = String::from_utf8_lossy(r.take(nlen)?).trim().to_string();
            let consumed = 1 + nlen;
            let pad = (4 - (consumed % 4)) % 4;
            r.skip(pad)?;
            // Additional layer info blocks; 'luni' holds the Unicode name.
            while r.p < extra_end {
                let sig = r.tag()?;
                if &sig != b"8BIM" && &sig != b"8B64" {
                    break;
                }
                let key = r.tag()?;
                let dlen = r.u32()? as usize;
                let data = r.take(dlen)?.to_vec();
                if dlen % 2 == 1 {
                    r.skip(1).ok();
                }
                if &key == b"luni" && data.len() >= 4 {
                    let n = u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as usize;
                    let mut chars = Vec::with_capacity(n);
                    for c in 0..n {
                        let o = 4 + c * 2;
                        if o + 2 <= data.len() {
                            chars.push(u16::from_be_bytes([data[o], data[o + 1]]));
                        }
                    }
                    let uni: String = String::from_utf16_lossy(&chars);
                    let uni = uni.trim().to_string();
                    if !uni.is_empty() {
                        name = uni;
                    }
                }
            }
            records.push(Rec {
                name,
                left,
                top,
                right,
                bottom,
                visible: (flags & 0x02) == 0 && opacity > 0,
                opacity,
                multiply,
                channels,
            });
        }

        // Channel image data, same order as records.
        let mut layers = Vec::with_capacity(records.len());
        for rec in records {
            let w = (rec.right - rec.left).max(0) as usize;
            let h = (rec.bottom - rec.top).max(0) as usize;
            let mut chan_data: HashMap<i16, Vec<u8>> = HashMap::new();
            for (id, len) in rec.channels.clone() {
                // Always consume the channel's byte block (divider records may
                // carry a length-2 block that holds only the compression field).
                let data = r.take(len as usize)?;
                if len < 2 || w == 0 || h == 0 {
                    continue;
                }
                let comp = u16::from_be_bytes([data[0], data[1]]);
                let payload = &data[2..];
                let row_bytes = w;
                let expected = row_bytes * h;
                let decoded = match comp {
                    0 => {
                        if payload.len() < expected {
                            return Err("raw channel data truncated".into());
                        }
                        payload[..expected].to_vec()
                    }
                    1 => {
                        let mut counts = Vec::with_capacity(h);
                        let mut pos = 0usize;
                        for _ in 0..h {
                            if pos + 2 > payload.len() {
                                return Err("rle row table truncated".into());
                            }
                            counts.push(u16::from_be_bytes([
                                payload[pos],
                                payload[pos + 1],
                            ]));
                            pos += 2;
                        }
                        let total: usize = counts.iter().map(|c| *c as usize).sum();
                        if payload.len() < pos + total {
                            return Err("rle payload truncated".into());
                        }
                        packbits_decode(&payload[pos..pos + total], expected)?
                    }
                    c => return Err(format!("unsupported channel compression {c}")),
                };
                chan_data.insert(id, decoded);
            }
            let rgba = if w > 0 && h > 0 && !rec.channels.is_empty() {
                let sample = |chan: &HashMap<i16, Vec<u8>>, id: i16, i: usize| -> u8 {
                    chan.get(&id).map(|d| d[i]).unwrap_or(0)
                };
                let mut px = vec![0u8; w * h * 4];
                for i in 0..w * h {
                    let a = if rec.channels.iter().any(|(id, _)| *id == -1) {
                        sample(&chan_data, -1, i)
                    } else {
                        255
                    };
                    px[i * 4] = sample(&chan_data, 0, i);
                    px[i * 4 + 1] = sample(&chan_data, 1, i);
                    px[i * 4 + 2] = sample(&chan_data, 2, i);
                    px[i * 4 + 3] = a;
                }
                Some(px)
            } else {
                None
            };
            layers.push(Layer {
                name: rec.name,
                left: rec.left,
                top: rec.top,
                right: rec.right,
                bottom: rec.bottom,
                visible: rec.visible,
                opacity: rec.opacity,
                multiply: rec.multiply,
                rgba,
            });
        }
        Ok(Psd { width, height, layers })
    }

    pub fn find(&self, name: &str) -> Option<&Layer> {
        self.layers.iter().find(|l| l.name == name && l.rgba.is_some())
    }

    /// Composite the named layers (bottom-to-top order) with alpha blending.
    /// Returns the union-bbox cropped RGBA buffer plus its canvas offset.
    pub fn compose(&self, names: &[&str]) -> Result<(Vec<u8>, i32, i32, u32, u32), String> {
        let mut picked: Vec<&Layer> = Vec::with_capacity(names.len());
        for n in names {
            let l = self
                .find(n)
                .ok_or_else(|| format!("layer not found: {n}"))?;
            picked.push(l);
        }
        if picked.is_empty() {
            return Err("no layers to compose".into());
        }
        let x0 = picked.iter().map(|l| l.left).min().unwrap();
        let y0 = picked.iter().map(|l| l.top).min().unwrap();
        let x1 = picked.iter().map(|l| l.right).max().unwrap();
        let y1 = picked.iter().map(|l| l.bottom).max().unwrap();
        let w = (x1 - x0) as usize;
        let h = (y1 - y0) as usize;
        let mut out = vec![0u8; w * h * 4];
        for l in picked {
            blit_layer(&mut out, w, h, x0, y0, l);
        }
        Ok((out, x0, y0, w as u32, h as u32))
    }
}

/// Blend `layer` into `out` (RGBA, size w*h, canvas offset x0,y0) using
/// normal or multiply blending with the layer's opacity.
pub fn blit_layer(out: &mut [u8], w: usize, h: usize, x0: i32, y0: i32, layer: &Layer) {
    let rgba = match &layer.rgba {
        Some(r) => r,
        None => return,
    };
    let lw = layer.width() as usize;
    let lh = layer.height() as usize;
    if lw == 0 || lh == 0 {
        return;
    }
    let ox = (layer.left - x0) as i64;
    let oy = (layer.top - y0) as i64;
    let lo = layer.opacity as f32 / 255.0;
    for ly in 0..lh {
        let dy = oy + ly as i64;
        if dy < 0 || dy >= h as i64 {
            continue;
        }
        for lx in 0..lw {
            let dx = ox + lx as i64;
            if dx < 0 || dx >= w as i64 {
                continue;
            }
            let si = (ly * lw + lx) * 4;
            let sa = rgba[si + 3] as f32 / 255.0 * lo;
            if sa <= 0.0 {
                continue;
            }
            let di = ((dy as usize) * w + dx as usize) * 4;
            let da = out[di + 3] as f32 / 255.0;
            let blend_rgb = |sr: f32, dr: f32| -> f32 {
                if layer.multiply {
                    let mult = dr * sr / 255.0;
                    (1.0 - sa) * dr + sa * mult
                } else {
                    (1.0 - sa) * dr + sa * sr
                }
            };
            let nr = blend_rgb(rgba[si] as f32, out[di] as f32);
            let ng = blend_rgb(rgba[si + 1] as f32, out[di + 1] as f32);
            let nb = blend_rgb(rgba[si + 2] as f32, out[di + 2] as f32);
            let na = sa + da * (1.0 - sa);
            out[di] = nr.clamp(0.0, 255.0) as u8;
            out[di + 1] = ng.clamp(0.0, 255.0) as u8;
            out[di + 2] = nb.clamp(0.0, 255.0) as u8;
            out[di + 3] = (na * 255.0).clamp(0.0, 255.0) as u8;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packbits_roundtrip() {
        let src: Vec<u8> = (0..64u8).collect();
        // encode manually: literal run
        let mut enc = vec![63u8];
        enc.extend_from_slice(&src);
        let dec = packbits_decode(&enc, 64).unwrap();
        assert_eq!(dec.len(), 64);
        assert_eq!(dec, src);
    }

    #[test]
    fn packbits_repeat() {
        let enc = vec![254u8, 7]; // repeat 0x07 three times
        let dec = packbits_decode(&enc, 3).unwrap();
        assert_eq!(dec, vec![7, 7, 7]);
    }

    #[test]
    fn parse_shipped_psd() {
        let bytes = include_bytes!("../../../assets/model/Set1.psd");
        let psd = Psd::parse(bytes).expect("parse");
        assert_eq!(psd.width, 1078);
        assert_eq!(psd.height, 2559);
        let names: Vec<String> = psd
            .layers
            .iter()
            .filter(|l| l.rgba.is_some())
            .map(|l| format!("{} [{}x{}]", l.name, l.width(), l.height()))
            .collect();
        for n in ["Head", "face 3", "hair 3 color", "Body2", "shadow copy 2"] {
            assert!(
                psd.find(n).is_some(),
                "missing layer {n}\navailable: {}",
                names.join(", ")
            );
        }
    }
}

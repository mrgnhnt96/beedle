use std::fs;
use std::io::Read;
use std::path::Path;

use flate2::read::ZlibDecoder;

use super::Level;
use super::html::{attr, tags};
use crate::js;

const BLANK_MIN_PIXELS: u64 = 10_000;
const PNG_FLAT_RANGE: i32 = 2;
const PNG_SIGNATURE_LENGTH: usize = 8;

pub struct Info {
    pub format: &'static str,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug)]
pub struct Blank {
    pub level: Level,
    pub detail: String,
}

// Measured at 1600x1000: a flat fill at high quality lands under these, one line of text on white over them.
fn blank_bytes_per_pixel(format: &str) -> f64 {
    match format {
        "jpeg" => 0.014,
        "webp" => 0.0025,
        _ => 0.0005,
    }
}

// Size alone can't tell a flat fill from a soft gradient in these, so they only warn.
fn is_lossy(format: &str) -> bool {
    matches!(format, "jpeg" | "webp" | "avif")
}

fn be16(bytes: &[u8], at: usize) -> Option<u32> {
    let pair = bytes.get(at..at + 2)?;
    Some(u32::from(pair[0]) << 8 | u32::from(pair[1]))
}

fn be32(bytes: &[u8], at: usize) -> Option<u32> {
    let quad: [u8; 4] = bytes.get(at..at + 4)?.try_into().ok()?;
    Some(u32::from_be_bytes(quad))
}

fn le16(bytes: &[u8], at: usize) -> Option<u32> {
    let pair = bytes.get(at..at + 2)?;
    Some(u32::from(pair[0]) | u32::from(pair[1]) << 8)
}

fn le24(bytes: &[u8], at: usize) -> Option<u32> {
    let triple = bytes.get(at..at + 3)?;
    Some(u32::from(triple[0]) | u32::from(triple[1]) << 8 | u32::from(triple[2]) << 16)
}

fn le32(bytes: &[u8], at: usize) -> Option<u32> {
    let quad: [u8; 4] = bytes.get(at..at + 4)?.try_into().ok()?;
    Some(u32::from_le_bytes(quad))
}

fn has(bytes: &[u8], from: usize, text: &[u8]) -> bool {
    bytes.get(from..from + text.len()) == Some(text)
}

pub fn png_size(path: &Path) -> Option<(u32, u32)> {
    let bytes = fs::read(path).ok()?;
    if !has(&bytes, 1, b"PNG") {
        return None;
    }
    Some((be32(&bytes, 16)?, be32(&bytes, 20)?))
}

fn jpeg_size(bytes: &[u8]) -> Option<Info> {
    let mut at = 2;
    while at + 9 < bytes.len() {
        let marker = bytes[at + 1];
        if bytes[at] != 0xff {
            return None;
        }
        if (0xc0..=0xcf).contains(&marker) && ![0xc4, 0xc8, 0xcc].contains(&marker) {
            return Some(Info {
                format: "jpeg",
                width: be16(bytes, at + 7)?,
                height: be16(bytes, at + 5)?,
            });
        }
        at += 2 + be16(bytes, at + 2)? as usize;
    }
    None
}

fn webp_size(bytes: &[u8]) -> Option<Info> {
    if has(bytes, 12, b"VP8 ") {
        return Some(Info {
            format: "webp",
            width: le16(bytes, 26)? & 0x3fff,
            height: le16(bytes, 28)? & 0x3fff,
        });
    }
    if has(bytes, 12, b"VP8X") {
        return Some(Info {
            format: "webp",
            width: le24(bytes, 24)? + 1,
            height: le24(bytes, 27)? + 1,
        });
    }
    if !has(bytes, 12, b"VP8L") {
        return None;
    }
    let bits = le32(bytes, 21)?;
    Some(Info {
        format: "webp-lossless",
        width: (bits & 0x3fff) + 1,
        height: ((bits >> 14) & 0x3fff) + 1,
    })
}

pub fn image_info(bytes: &[u8]) -> Option<Info> {
    if has(bytes, 1, b"PNG") {
        return Some(Info {
            format: "png",
            width: be32(bytes, 16)?,
            height: be32(bytes, 20)?,
        });
    }
    if has(bytes, 0, b"GIF8") {
        return Some(Info {
            format: "gif",
            width: le16(bytes, 6)?,
            height: le16(bytes, 8)?,
        });
    }
    if has(bytes, 0, b"RIFF") && has(bytes, 8, b"WEBP") {
        return webp_size(bytes);
    }
    if has(bytes, 0, &[0xff, 0xd8]) {
        return jpeg_size(bytes);
    }
    let ispe = bytes.windows(4).position(|name| name == b"ispe")?;
    if !has(bytes, 4, b"ftyp") || ispe == 0 {
        return None;
    }
    Some(Info {
        format: "avif",
        width: be32(bytes, ispe + 8)?,
        height: be32(bytes, ispe + 12)?,
    })
}

#[derive(Default)]
struct PngChunks<'a> {
    image: Vec<u8>,
    palette: Option<&'a [u8]>,
    alpha: Option<&'a [u8]>,
}

fn png_chunks(bytes: &[u8]) -> Option<PngChunks<'_>> {
    let mut chunks = PngChunks::default();
    let mut at = PNG_SIGNATURE_LENGTH;
    while at + 8 <= bytes.len() {
        let length = be32(bytes, at)? as usize;
        let end = (at + 8 + length).min(bytes.len());
        let data = &bytes[at + 8..end];
        match &bytes[at + 4..at + 8] {
            b"IDAT" => chunks.image.extend_from_slice(data),
            b"PLTE" => chunks.palette = Some(data),
            b"tRNS" => chunks.alpha = Some(data),
            _ => {}
        }
        at += 12 + length;
    }
    Some(chunks)
}

fn paeth(left: i32, up: i32, corner: i32) -> i32 {
    let guess = left + up - corner;
    let (dl, du, dc) = (
        (guess - left).abs(),
        (guess - up).abs(),
        (guess - corner).abs(),
    );
    if dl <= du && dl <= dc {
        left
    } else if du <= dc {
        up
    } else {
        corner
    }
}

fn unfilter(row: &[u8], previous: &[u8], filter: u8, step: usize) -> Vec<u8> {
    let mut line = vec![0; row.len()];
    if filter > 4 {
        return line;
    }
    for i in 0..row.len() {
        let left = if i >= step { i32::from(line[i - step]) } else { 0 };
        let up = i32::from(previous[i]);
        let corner = if i >= step {
            i32::from(previous[i - step])
        } else {
            0
        };
        let predicted = match filter {
            0 => 0,
            1 => left,
            2 => up,
            3 => (left + up) >> 1,
            _ => paeth(left, up, corner),
        };
        line[i] = (i32::from(row[i]) + predicted) as u8;
    }
    line
}

struct PngFormat<'a> {
    kind: u8,
    depth: usize,
    samples: usize,
    palette: Option<&'a [u8]>,
    alpha: Option<&'a [u8]>,
}

impl PngFormat<'_> {
    fn raw(&self, line: &[u8], index: usize) -> usize {
        match self.depth {
            16 => usize::from(line[index * 2]),
            8 => usize::from(line[index]),
            depth => {
                let bit = index * depth;
                let shift = 8 - depth - bit % 8;
                (usize::from(line[bit >> 3]) >> shift) & ((1 << depth) - 1)
            }
        }
    }

    fn value(&self, line: &[u8], index: usize) -> i32 {
        let raw = self.raw(line, index);
        if self.depth < 8 {
            js::round(raw as f64 * 255.0 / ((1 << self.depth) - 1) as f64) as i32
        } else {
            raw as i32
        }
    }

    fn pixel(&self, line: &[u8], x: usize) -> Option<[i32; 4]> {
        let at = x * self.samples;
        let value = |index| self.value(line, index);
        Some(match self.kind {
            3 => {
                let entry = self.raw(line, at);
                let colour = self.palette?.get(entry * 3..entry * 3 + 3)?;
                let alpha = self.alpha.and_then(|alpha| alpha.get(entry));
                [
                    i32::from(colour[0]),
                    i32::from(colour[1]),
                    i32::from(colour[2]),
                    alpha.map_or(255, |alpha| i32::from(*alpha)),
                ]
            }
            0 => [value(at), value(at), value(at), 255],
            4 => [value(at), value(at), value(at), value(at + 1)],
            6 => [value(at), value(at + 1), value(at + 2), value(at + 3)],
            _ => [value(at), value(at + 1), value(at + 2), 255],
        })
    }
}

fn png_samples(kind: u8) -> Option<usize> {
    match kind {
        0 | 3 => Some(1),
        2 => Some(3),
        4 => Some(2),
        6 => Some(4),
        _ => None,
    }
}

fn flat_png_colour(bytes: &[u8], info: &Info) -> Option<String> {
    let depth = usize::from(*bytes.get(24)?);
    let kind = *bytes.get(25)?;
    if ![1, 2, 4, 8, 16].contains(&depth) {
        return None;
    }
    let samples = png_samples(kind)?;
    let chunks = png_chunks(bytes)?;
    let format = PngFormat {
        kind,
        depth,
        samples,
        palette: chunks.palette,
        alpha: chunks.alpha,
    };
    let mut pixels = Vec::new();
    ZlibDecoder::new(chunks.image.as_slice())
        .read_to_end(&mut pixels)
        .ok()?;

    let width = info.width as usize;
    let stride = (width * samples * depth).div_ceil(8);
    let step = ((samples * depth) >> 3).max(1);
    let mut low = [255; 4];
    let mut high = [0; 4];
    let mut previous = vec![0; stride];
    for y in 0..info.height as usize {
        let start = y * (stride + 1);
        let row = pixels.get(start + 1..start + 1 + stride)?;
        previous = unfilter(row, &previous, pixels[start], step);
        for x in 0..width {
            let pixel = format.pixel(&previous, x)?;
            for channel in 0..4 {
                low[channel] = low[channel].min(pixel[channel]);
                high[channel] = high[channel].max(pixel[channel]);
                if high[channel] - low[channel] > PNG_FLAT_RANGE {
                    return None;
                }
            }
        }
    }
    Some(format!("#{:02x}{:02x}{:02x}", low[0], low[1], low[2]))
}

pub fn blank_image(file: &Path) -> Option<Blank> {
    let bytes = fs::read(file).ok()?;
    let info = image_info(&bytes)?;
    let pixels = u64::from(info.width) * u64::from(info.height);
    if pixels < BLANK_MIN_PIXELS {
        return None;
    }
    let interlaced = bytes.get(28) != Some(&0);
    if info.format == "png" && !interlaced {
        let colour = flat_png_colour(&bytes, &info)?;
        return Some(Blank {
            level: Level::Error,
            detail: format!("is one flat colour ({colour})"),
        });
    }
    if bytes.len() as f64 / pixels as f64 >= blank_bytes_per_pixel(info.format) {
        return None;
    }
    Some(Blank {
        level: if is_lossy(info.format) {
            Level::Warn
        } else {
            Level::Error
        },
        detail: format!(
            "is {}x{} in {} bytes, too little to hold a picture",
            info.width,
            info.height,
            bytes.len()
        ),
    })
}

pub fn sources<'a>(html: &'a str, share_image: Option<&'a str>) -> Vec<&'a str> {
    let mut found = Vec::new();
    for tag in tags(html, "img").into_iter().chain(tags(html, "source")) {
        found.extend(attr(tag, "src"));
        let srcset = attr(tag, "srcset").unwrap_or("").split(',');
        found.extend(srcset.filter_map(|part| js::trim(part).split(js::is_space).next()));
    }
    found.extend(share_image);
    let mut unique = Vec::new();
    for src in found {
        if !src.is_empty() && !src.starts_with("data:") && !unique.contains(&src) {
            unique.push(src);
        }
    }
    unique
}

use std::fmt::Write as _;
use std::io::{self, BufWriter, Write};

use console::Term;

use crate::ace::{Ace, OutputMode};

const GIF_DATA: &[u8] = include_bytes!("../topgun.gif");

// --- Tuning knobs ---
const COLOR_LEVELS: u8 = 16; // quantization steps per channel (fewer = blockier, more color runs)
const COLOR_MIN: u8 = 20; // black floor — values below this become 0
const COLOR_MAX: u8 = 220; // white ceiling — values above this become 255

pub fn run(ace: &Ace) {
    if ace.mode() != OutputMode::Human {
        return;
    }

    let term = Term::stderr();
    let (th, tw) = (term.size().0 as u32, term.size().1 as u32);

    let frames = match render_frames(tw, th) {
        Ok(f) if !f.is_empty() => f,
        _ => return,
    };

    ace.enter_alt_screen();

    let mut out = BufWriter::new(io::stderr());

    for (text, delay_ms) in &frames {
        let _ = out.write_all(b"\x1b[H");
        let _ = out.write_all(text.as_bytes());
        let _ = out.flush();
        std::thread::sleep(std::time::Duration::from_millis(*delay_ms));
    }
}

// --- Pipeline: decode → blit → scale+quantize → render ---

fn render_frames(tw: u32, th: u32) -> Result<Vec<(String, u64)>, io::Error> {
    let pixel_h = th * 2;
    let color_lut = build_color_lut();

    let mut opts = gif::DecodeOptions::new();
    opts.set_color_output(gif::ColorOutput::RGBA);
    let mut reader = opts
        .read_info(GIF_DATA)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    let gw = reader.width() as u32;
    let gh = reader.height() as u32;
    let (out_w, out_h, x_map, y_map) = build_scale_maps(gw, gh, tw, pixel_h);

    let mut canvas = vec![0u8; gw as usize * gh as usize * 4];
    let mut rgb = vec![0u8; out_w as usize * out_h as usize * 3];
    let mut rendered = Vec::new();

    while let Some(frame) = reader
        .read_next_frame()
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?
    {
        let delay_ms = (frame.delay as u64 * 10).max(1);

        blit_frame(&mut canvas, gw, frame);
        scale_and_quantize(&mut rgb, &canvas, gw, &x_map, &y_map, out_w, &color_lut);
        let text = render_half_blocks(&rgb, out_w, out_h);
        rendered.push((text, delay_ms));
    }

    Ok(rendered)
}

/// Composite a GIF frame onto the persistent canvas (handles sub-region offsets and alpha).
fn blit_frame(canvas: &mut [u8], canvas_w: u32, frame: &gif::Frame) {
    let fx = frame.left as usize;
    let fy = frame.top as usize;
    let fw = frame.width as usize;
    let fh = frame.height as usize;

    for row in 0..fh {
        for col in 0..fw {
            let src = (row * fw + col) * 4;
            let dst = ((fy + row) * canvas_w as usize + (fx + col)) * 4;
            if src + 3 < frame.buffer.len() && dst + 3 < canvas.len() && frame.buffer[src + 3] > 0 {
                canvas[dst..dst + 4].copy_from_slice(&frame.buffer[src..src + 4]);
            }
        }
    }
}

/// 256-byte lookup table for color posterization. Built once, used for every pixel.
fn build_color_lut() -> [u8; 256] {
    let mut lut = [0u8; 256];
    let range = (COLOR_MAX - COLOR_MIN) as u16;
    let step = range / COLOR_LEVELS as u16;

    for (i, slot) in lut.iter_mut().enumerate() {
        let c = i as u8;
        *slot = if c <= COLOR_MIN {
            0
        } else if c >= COLOR_MAX {
            255
        } else {
            let normed = (c - COLOR_MIN) as u16;
            let q = (normed / step) * step + step / 2 + COLOR_MIN as u16;
            q.min(255) as u8
        };
    }

    lut
}

/// Precompute source pixel coordinates for nearest-neighbor scaling.
/// All frames share the same dimensions, so this is built once.
fn build_scale_maps(
    src_w: u32,
    src_h: u32,
    dst_w: u32,
    dst_h: u32,
) -> (u32, u32, Vec<u32>, Vec<u32>) {
    let scale_w = dst_w as f64 / src_w as f64;
    let scale_h = dst_h as f64 / src_h as f64;
    let scale = scale_w.min(scale_h);
    let out_w = ((src_w as f64 * scale) as u32).max(1);
    let out_h = ((src_h as f64 * scale) as u32).max(1);

    let x_map: Vec<u32> = (0..out_w)
        .map(|x| (x as f64 / scale).min((src_w - 1) as f64) as u32)
        .collect();
    let y_map: Vec<u32> = (0..out_h)
        .map(|y| (y as f64 / scale).min((src_h - 1) as f64) as u32)
        .collect();

    (out_w, out_h, x_map, y_map)
}

/// Scale source RGBA pixels to output dimensions and quantize colors in one pass.
/// Writes stride-3 RGB into a reusable buffer (alpha dropped — not needed for terminal).
fn scale_and_quantize(
    rgb: &mut [u8],
    rgba: &[u8],
    src_w: u32,
    x_map: &[u32],
    y_map: &[u32],
    out_w: u32,
    lut: &[u8; 256],
) {
    for (y, &sy) in y_map.iter().enumerate() {
        let src_row = (sy * src_w) as usize;
        let dst_row = y * out_w as usize;
        for (x, &sx) in x_map.iter().enumerate() {
            let si = (src_row + sx as usize) * 4;
            let di = (dst_row + x) * 3;
            rgb[di] = lut[rgba[si] as usize];
            rgb[di + 1] = lut[rgba[si + 1] as usize];
            rgb[di + 2] = lut[rgba[si + 2] as usize];
        }
    }
}

/// Render quantized RGB pixels as Unicode half-block characters with ANSI true-color.
/// Each terminal row packs two pixel rows using ▀ (upper half block).
fn render_half_blocks(rgb: &[u8], w: u32, h: u32) -> String {
    let mut buf = String::with_capacity(w as usize * (h as usize / 2) * 20);

    let px = |x: u32, y: u32| -> [u8; 3] {
        if y < h && x < w {
            let i = (y * w + x) as usize * 3;
            [rgb[i], rgb[i + 1], rgb[i + 2]]
        } else {
            [0, 0, 0]
        }
    };

    let mut y = 0u32;
    while y < h {
        let mut prev_fg: [u8; 3] = [255, 255, 255];
        let mut prev_bg: [u8; 3] = [255, 255, 255];

        for x in 0..w {
            let fg = px(x, y);
            let bg = px(x, y + 1);

            if fg != prev_fg {
                write!(buf, "\x1b[38;2;{};{};{}m", fg[0], fg[1], fg[2]).ok();
                prev_fg = fg;
            }
            if bg != prev_bg {
                write!(buf, "\x1b[48;2;{};{};{}m", bg[0], bg[1], bg[2]).ok();
                prev_bg = bg;
            }
            buf.push('\u{2580}');
        }
        buf.push_str("\x1b[0m\n");
        y += 2;
    }

    buf
}

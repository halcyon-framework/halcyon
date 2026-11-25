//! SPEC-HAL-265 M6: SVG path `d` attribute parser.
//!
//! Converts SVG path data strings into `Path` values.
//! Arc commands (A/a) are not supported and return `SvgPathError::UnknownCommand`.

use crate::renderer::path::Path;

/// Errors that can occur when parsing an SVG path `d` attribute string.
#[derive(Debug, Clone)]
pub enum SvgPathError {
    /// Ran out of input while expecting more parameters.
    UnexpectedEnd,
    /// A token could not be parsed as a valid number.
    InvalidNumber(String),
    /// Encountered a command letter that is not supported.
    UnknownCommand(char),
}

/// Parse an SVG path `d` attribute string into a `Path`.
///
/// Supports: M/m, L/l, H/h, V/v, C/c, S/s, Q/q, T/t, Z/z.
/// Arc commands (A/a) return `SvgPathError::UnknownCommand`.
pub fn parse_svg_path(d: &str) -> Result<Path, SvgPathError> {
    let mut path = Path::new();
    let bytes = d.as_bytes();
    let mut pos = 0usize;
    let mut cx = 0.0f32;
    let mut cy = 0.0f32;
    let mut sx = 0.0f32;
    let mut sy = 0.0f32;
    let mut last_cubic_cp2: Option<(f32, f32)> = None;
    let mut last_quad_cp: Option<(f32, f32)> = None;
    let mut last_cmd: Option<u8> = None;

    loop {
        skip_wsp_comma(bytes, &mut pos);
        if pos >= bytes.len() {
            break;
        }

        let b = bytes[pos];
        let cmd = if b.is_ascii_alphabetic() {
            pos += 1;
            b
        } else if let Some(prev) = last_cmd {
            match prev {
                b'M' => b'L',
                b'm' => b'l',
                b'Z' | b'z' => {
                    return Err(SvgPathError::InvalidNumber(format!(
                        "unexpected number after closepath at position {pos}"
                    )));
                }
                other => other,
            }
        } else {
            let end = (pos + 1).min(d.len());
            return Err(SvgPathError::InvalidNumber(d[pos..end].to_string()));
        };

        match cmd {
            b'M' | b'm' => {
                let rel = cmd == b'm';
                let x = next_number(bytes, &mut pos)?;
                let y = next_number(bytes, &mut pos)?;
                let (ax, ay) = if rel { (cx + x, cy + y) } else { (x, y) };
                path.move_to(ax, ay);
                cx = ax;
                cy = ay;
                sx = ax;
                sy = ay;
                last_cubic_cp2 = None;
                last_quad_cp = None;
            }
            b'L' | b'l' => {
                let rel = cmd == b'l';
                let x = next_number(bytes, &mut pos)?;
                let y = next_number(bytes, &mut pos)?;
                let (ax, ay) = if rel { (cx + x, cy + y) } else { (x, y) };
                path.line_to(ax, ay);
                cx = ax;
                cy = ay;
                last_cubic_cp2 = None;
                last_quad_cp = None;
            }
            b'H' | b'h' => {
                let rel = cmd == b'h';
                let v = next_number(bytes, &mut pos)?;
                let ax = if rel { cx + v } else { v };
                path.line_to(ax, cy);
                cx = ax;
                last_cubic_cp2 = None;
                last_quad_cp = None;
            }
            b'V' | b'v' => {
                let rel = cmd == b'v';
                let v = next_number(bytes, &mut pos)?;
                let ay = if rel { cy + v } else { v };
                path.line_to(cx, ay);
                cy = ay;
                last_cubic_cp2 = None;
                last_quad_cp = None;
            }
            b'C' | b'c' => {
                let rel = cmd == b'c';
                let c1x = next_number(bytes, &mut pos)?;
                let c1y = next_number(bytes, &mut pos)?;
                let c2x = next_number(bytes, &mut pos)?;
                let c2y = next_number(bytes, &mut pos)?;
                let x = next_number(bytes, &mut pos)?;
                let y = next_number(bytes, &mut pos)?;
                let (ac1x, ac1y, ac2x, ac2y, ax, ay) = if rel {
                    (cx + c1x, cy + c1y, cx + c2x, cy + c2y, cx + x, cy + y)
                } else {
                    (c1x, c1y, c2x, c2y, x, y)
                };
                path.cubic_to(ac1x, ac1y, ac2x, ac2y, ax, ay);
                last_cubic_cp2 = Some((ac2x, ac2y));
                last_quad_cp = None;
                cx = ax;
                cy = ay;
            }
            b'S' | b's' => {
                let rel = cmd == b's';
                let c2x = next_number(bytes, &mut pos)?;
                let c2y = next_number(bytes, &mut pos)?;
                let x = next_number(bytes, &mut pos)?;
                let y = next_number(bytes, &mut pos)?;
                let (rc1x, rc1y) = match last_cubic_cp2 {
                    Some((lx, ly)) => (2.0 * cx - lx, 2.0 * cy - ly),
                    None => (cx, cy),
                };
                let (ac2x, ac2y, ax, ay) = if rel {
                    (cx + c2x, cy + c2y, cx + x, cy + y)
                } else {
                    (c2x, c2y, x, y)
                };
                path.cubic_to(rc1x, rc1y, ac2x, ac2y, ax, ay);
                last_cubic_cp2 = Some((ac2x, ac2y));
                last_quad_cp = None;
                cx = ax;
                cy = ay;
            }
            b'Q' | b'q' => {
                let rel = cmd == b'q';
                let qx = next_number(bytes, &mut pos)?;
                let qy = next_number(bytes, &mut pos)?;
                let x = next_number(bytes, &mut pos)?;
                let y = next_number(bytes, &mut pos)?;
                let (aqx, aqy, ax, ay) = if rel {
                    (cx + qx, cy + qy, cx + x, cy + y)
                } else {
                    (qx, qy, x, y)
                };
                let cp1x = cx + 2.0 / 3.0 * (aqx - cx);
                let cp1y = cy + 2.0 / 3.0 * (aqy - cy);
                let cp2x = ax + 2.0 / 3.0 * (aqx - ax);
                let cp2y = ay + 2.0 / 3.0 * (aqy - ay);
                path.cubic_to(cp1x, cp1y, cp2x, cp2y, ax, ay);
                last_quad_cp = Some((aqx, aqy));
                last_cubic_cp2 = None;
                cx = ax;
                cy = ay;
            }
            b'T' | b't' => {
                let rel = cmd == b't';
                let x = next_number(bytes, &mut pos)?;
                let y = next_number(bytes, &mut pos)?;
                let (ax, ay) = if rel { (cx + x, cy + y) } else { (x, y) };
                let (aqx, aqy) = match last_quad_cp {
                    Some((lx, ly)) => (2.0 * cx - lx, 2.0 * cy - ly),
                    None => (cx, cy),
                };
                let cp1x = cx + 2.0 / 3.0 * (aqx - cx);
                let cp1y = cy + 2.0 / 3.0 * (aqy - cy);
                let cp2x = ax + 2.0 / 3.0 * (aqx - ax);
                let cp2y = ay + 2.0 / 3.0 * (aqy - ay);
                path.cubic_to(cp1x, cp1y, cp2x, cp2y, ax, ay);
                last_quad_cp = Some((aqx, aqy));
                last_cubic_cp2 = None;
                cx = ax;
                cy = ay;
            }
            b'Z' | b'z' => {
                path.close();
                cx = sx;
                cy = sy;
                last_cubic_cp2 = None;
                last_quad_cp = None;
            }
            b'A' | b'a' => return Err(SvgPathError::UnknownCommand(cmd as char)),
            _ => return Err(SvgPathError::UnknownCommand(cmd as char)),
        }

        last_cmd = Some(cmd);
    }

    Ok(path)
}

fn skip_wsp_comma(bytes: &[u8], pos: &mut usize) {
    while *pos < bytes.len() {
        match bytes[*pos] {
            b' ' | b'\t' | b'\n' | b'\r' | b',' => *pos += 1,
            _ => break,
        }
    }
}

fn next_number(bytes: &[u8], pos: &mut usize) -> Result<f32, SvgPathError> {
    skip_wsp_comma(bytes, pos);
    if *pos >= bytes.len() {
        return Err(SvgPathError::UnexpectedEnd);
    }

    let start = *pos;
    let mut has_digit = false;

    // Optional sign
    if *pos < bytes.len() && (bytes[*pos] == b'+' || bytes[*pos] == b'-') {
        *pos += 1;
    }

    // Integer part
    while *pos < bytes.len() && bytes[*pos].is_ascii_digit() {
        has_digit = true;
        *pos += 1;
    }

    // Fractional part (only one dot allowed)
    if *pos < bytes.len() && bytes[*pos] == b'.' {
        *pos += 1;
        while *pos < bytes.len() && bytes[*pos].is_ascii_digit() {
            has_digit = true;
            *pos += 1;
        }
    }

    if !has_digit {
        let end = (*pos).max(start + 1).min(bytes.len());
        let s = std::str::from_utf8(&bytes[start..end])
            .unwrap_or("")
            .to_string();
        return Err(SvgPathError::InvalidNumber(s));
    }

    // Exponent part
    if *pos < bytes.len() && (bytes[*pos] == b'e' || bytes[*pos] == b'E') {
        *pos += 1;
        if *pos < bytes.len() && (bytes[*pos] == b'+' || bytes[*pos] == b'-') {
            *pos += 1;
        }
        let exp_start = *pos;
        while *pos < bytes.len() && bytes[*pos].is_ascii_digit() {
            *pos += 1;
        }
        if *pos == exp_start {
            let s = std::str::from_utf8(&bytes[start..*pos])
                .unwrap_or("")
                .to_string();
            return Err(SvgPathError::InvalidNumber(s));
        }
    }

    let s = std::str::from_utf8(&bytes[start..*pos]).unwrap_or("");
    s.parse::<f32>()
        .map_err(|_| SvgPathError::InvalidNumber(s.to_string()))
}

use std::io::BufRead;

use crate::{FaceMode, SurfaceIndices, into_chunks, parse_float3};

fn find_newline(slice: &[u8]) -> Option<usize> {
    slice.iter().position(|&v| v == b'\n')
}

fn get_line_start(data: &[u8]) -> Option<usize> {
    let mut i = data.iter().position(|&c| c != b' ')?;
    while data[i] == b'#' {
        i += 2 + data[i + 1..].iter().position(|&c| c != b'\n')?;
        i += data.iter().position(|&c| c != b' ')?;
    }
    return Some(i);
}

fn parse_int(data: &[u8]) -> Option<(u32, usize)> {
    data.first().map(|&first_b| {
        let start = (first_b == b'+') as usize;
        let (i, acc) = data[start..]
            .iter()
            .take_while(|&val| val.is_ascii_digit())
            .fold((0, 0), |(i, acc), &val| {
                (i + 1, acc * 10 + (val - b'0') as u32)
            });
        (acc, i + start)
    })
}

fn parse_face_indices(
    data: &mut &[u8],
    mode: &mut FaceMode,
    indices: &mut Vec<u32>,
    strides: &mut Vec<u8>,
    nf: usize,
) -> Option<()> {
    // get_line already stripped blanks
    let (face_len, endword) = match parse_int(data) {
        Some(v) => v,
        None => {
            std::hint::cold_path();
            return None;
        }
    };
    *data = &data[endword + 1..];

    if face_len < 3 {
        std::hint::cold_path();
        return None;
    }

    if *mode != FaceMode::Polygon {
        if *mode == FaceMode::Undetermined {
            std::hint::cold_path();
            if face_len == 3 {
                *mode = FaceMode::Triangle;
            } else if face_len == 4 {
                *mode = FaceMode::Quad;
            } else {
                *mode = FaceMode::Polygon;
            }
        } else if *mode == FaceMode::Triangle && face_len != 3 {
            std::hint::cold_path();
            //add missing strides
            *strides = vec![3; indices.len() / 3];
            strides.reserve(nf - strides.len());
            *mode = FaceMode::Polygon;
        } else if *mode == FaceMode::Quad && face_len != 4 {
            std::hint::cold_path();
            //add missing strides
            *strides = vec![4; indices.len() / 4];
            *mode = FaceMode::Polygon;
            strides.reserve(nf - strides.len());
        }
    }
    if *mode == FaceMode::Polygon {
        strides.push(face_len as u8);
    }

    if data[0] == b' ' {
        std::hint::cold_path();
        let endword = 1 + match data[1..].iter().position(|&c| c != b' ') {
            Some(v) => v,
            None => {
                std::hint::cold_path();
                return None;
            }
        };
        *data = &data[endword..];
    }

    let (v, endword) = match parse_int(data) {
        Some(v) => v,
        None => {
            std::hint::cold_path();
            return None;
        }
    };
    *data = &data[endword + 1..];
    indices.push(v);

    if data[0] == b' ' {
        std::hint::cold_path();
        let endword = 1 + match data[1..].iter().position(|&c| c != b' ') {
            Some(v) => v,
            None => {
                std::hint::cold_path();
                return None;
            }
        };
        *data = &data[endword..];
    }

    let (v, endword) = match parse_int(data) {
        Some(v) => v,
        None => {
            std::hint::cold_path();
            return None;
        }
    };
    *data = &data[endword + 1..];
    indices.push(v);

    if data[0] == b' ' {
        std::hint::cold_path();
        let endword = 1 + match data[1..].iter().position(|&c| c != b' ') {
            Some(v) => v,
            None => {
                std::hint::cold_path();
                return None;
            }
        };
        *data = &data[endword..];
    }

    for _ in 0..face_len - 3 {
        let (v, endword) = parse_int(data)?;
        *data = &data[endword + 1..];
        indices.push(v);

        if data[0] == b' ' {
            std::hint::cold_path();
            let endword = 1 + match data[1..].iter().position(|&c| c != b' ') {
                Some(v) => v,
                None => {
                    std::hint::cold_path();
                    return None;
                }
            };
            *data = &data[endword..];
        }
    }

    let (v, endword) = match parse_int(data) {
        Some(v) => v,
        None => {
            std::hint::cold_path();
            return None;
        }
    };
    indices.push(v);
    *data = &data[endword..];
    Some(())
}

fn parse_header(buf: &[u8]) -> Option<(usize, usize, usize)> {
    let (nv, mut endword) = parse_int(buf)?;
    while endword < buf.len() && buf[endword] == b' ' {
        endword += 1;
    }
    let (nf, endword) = parse_int(&buf[endword..])?;
    Some((nv as usize, nf as usize, endword))
}

pub fn load_off_buf<B: BufRead, const BUFFER_SIZE: usize>(
    reader: &mut B,
    buf: &mut [u8; BUFFER_SIZE],
    mut start: usize,
) -> Result<(Vec<[f32; 3]>, SurfaceIndices), ()> {
    let mut line_number = 0;
    let mut nv = 0;
    let mut nf = 0;
    let mut vertices = Vec::new();
    let mut mode = FaceMode::Undetermined;
    let mut indices: Vec<u32> = Vec::new();
    let mut strides: Vec<u8> = Vec::new();
    while let Ok(size) = reader.read(&mut buf[start..]) {
        if size == 0 && start == 0 {
            break;
        }
        let end = start + size;
        let mut last = end - 1;
        while buf[last] != b'\n' && last > 0 {
            last -= 1;
        }
        if buf[last] != b'\n' {
            break;
        }
        last += 1;

        let mut data = &buf[..last];

        if line_number == 0 {
            let line_start = get_line_start(data).ok_or(())?;
            data = &data[line_start..];
            if data.split_first_chunk::<3>().ok_or(())?.0 == b"OFF" {
                let end = find_newline(&data[3..]).ok_or(())? + 4;
                data = &data[end..];
            }
            let endword;
            (nv, nf, endword) = parse_header(data).ok_or(())?;
            data = &data[endword..];
            vertices.reserve(nv);
            indices.reserve(nv + nf - 2);
            line_number += 1;
            let end = find_newline(data).ok_or(())? + 1;
            data = &data[end..];
        }

        while line_number < nv + 1
            && let Some(line_start) = get_line_start(data)
        {
            data = &data[line_start..];
            let (off, pos) = unsafe { parse_float3(data) };
            data = &data[off..];
            line_number += 1;
            vertices.push(pos);
            let off = match data[0] {
                b'\r' => 2,
                b'\n' => 1,
                _ => find_newline(data).ok_or(())? + 1,
            };
            data = &data[off..];
        }

        while line_number < nv + nf + 1
            && let Some(line_start) = get_line_start(data)
        {
            data = &data[line_start..];
            parse_face_indices(&mut data, &mut mode, &mut indices, &mut strides, nf).ok_or(())?;
            line_number += 1;
            let off = match data[0] {
                b'\r' => 2,
                b'\n' => 1,
                _ => find_newline(data).ok_or(())? + 1,
            };
            data = &data[off..];
        }

        if line_number >= nv + nf + 1 {
            break;
        }

        start = end - last;
        buf.copy_within(last..end, 0);
    }

    let indices = if mode == FaceMode::Polygon {
        if strides.len() != nf {
            return Err(());
        }
        (indices, strides).into()
    } else if mode == FaceMode::Quad {
        if indices.len() / 4 != nf {
            return Err(());
        }
        into_chunks::<4>(indices).into()
    } else {
        if indices.len() / 3 != nf {
            return Err(());
        }
        into_chunks::<3>(indices).into()
    };
    Ok((vertices, indices))
}

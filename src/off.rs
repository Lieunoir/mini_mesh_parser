use std::io::BufRead;

use crate::{FaceMode, SurfaceIndices, into_chunks, parse_float3};

fn find_newline(slice: &[u8]) -> Option<usize> {
    slice.iter().position(|&v| v == b'\n')
}

fn get_line_start(slice: &[u8]) -> Option<usize> {
    for (i, char) in slice.iter().enumerate() {
        if *char != b' ' {
            if *char == b'#' || *char == b'\n' || *char == b'\r' {
                return None;
            } else {
                return Some(i);
            }
        }
    }
    None
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
    face_str: &[u8],
    mode: &mut FaceMode,
    indices: &mut Vec<u32>,
    strides: &mut Vec<u8>,
    nf: usize,
) -> Option<usize> {
    let mut off = 0;
    let mut data = face_str;
    while off < data.len() && data[off] == b' ' {
        off += 1;
    }
    data = &data[off..];

    let (face_len, mut endword) = parse_int(data)?;

    if face_len < 3 {
        return None;
    }

    if *mode != FaceMode::Polygon {
        if *mode == FaceMode::Undetermined {
            if face_len == 3 {
                *mode = FaceMode::Triangle;
            } else if face_len == 4 {
                *mode = FaceMode::Quad;
            } else {
                *mode = FaceMode::Polygon;
            }
        } else if *mode == FaceMode::Triangle && face_len != 3 {
            //add missing strides
            *strides = vec![3; indices.len() / 3];
            strides.reserve(nf - strides.len());
            *mode = FaceMode::Polygon;
        } else if *mode == FaceMode::Quad && face_len != 4 {
            //add missing strides
            *strides = vec![4; indices.len() / 4];
            *mode = FaceMode::Polygon;
            strides.reserve(nf - strides.len());
        }
    }
    if *mode == FaceMode::Polygon {
        strides.push(face_len as u8);
    }

    while endword < data.len() && data[endword] == b' ' {
        endword += 1;
    }
    off += endword;
    data = &data[endword..];
    let (v, mut endword) = parse_int(data)?;
    indices.push(v);
    while endword < data.len() && data[endword] == b' ' {
        endword += 1;
    }
    off += endword;
    data = &data[endword..];
    let (v, mut endword) = parse_int(data)?;
    indices.push(v);
    while endword < data.len() && data[endword] == b' ' {
        endword += 1;
    }
    off += endword;
    data = &data[endword..];
    let (v, mut endword) = parse_int(data)?;
    indices.push(v);
    while endword < data.len() && data[endword] == b' ' {
        endword += 1;
    }
    off += endword;
    data = &data[endword..];

    for _ in 0..face_len - 3 {
        let (v, mut endword) = parse_int(data)?;
        indices.push(v);
        while endword < data.len() && data[endword] == b' ' {
            endword += 1;
        }
        off += endword;
        data = &data[endword..];
    }
    Some(off)
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
    'outer: while let Ok(size) = reader.read(&mut buf[start..]) {
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

        let mut i = 0;
        while i < last {
            if let Some(line_start) = get_line_start(&buf[i..]) {
                i += line_start;
                if line_number == 0 {
                    if buf.split_first_chunk::<3>().ok_or(())?.0 == b"OFF" {
                        i += find_newline(&buf[3..]).ok_or(())? + 4;
                    }
                    let endword;
                    (nv, nf, endword) = parse_header(&buf[i..]).ok_or(())?;
                    vertices.reserve(nv);
                    indices.reserve(nv + nf - 2);
                    line_number += 1;
                    i += find_newline(&buf[i + endword..]).ok_or(())? + endword + 1;
                } else if line_number < nv + 1 {
                    let (off, pos) = unsafe { parse_float3(&buf[i..]) };
                    line_number += 1;
                    vertices.push(pos);
                    i += off;
                    i += match buf[i] {
                        b'\r' => 2,
                        b'\n' => 1,
                        _ => find_newline(&buf[i..]).ok_or(())? + 1,
                    }
                } else if line_number < 1 + nv + nf {
                    let off =
                        parse_face_indices(&buf[i..], &mut mode, &mut indices, &mut strides, nf)
                            .ok_or(())?;
                    line_number += 1;
                    i += off;
                    i += match buf[i] {
                        b'\r' => 2,
                        b'\n' => 1,
                        _ => find_newline(&buf[i..]).ok_or(())? + 1,
                    }
                } else {
                    break 'outer;
                }
            } else {
                i += find_newline(&buf[i..]).ok_or(())? + 1;
            }
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

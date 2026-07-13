use std::io::BufRead;

use crate::{FaceMode, SurfaceIndices, into_chunks, parse_float3};

fn find_newline(slice: &[u8]) -> Option<usize> {
    slice.iter().position(|&v| v == b'\n')
}

fn find_blank_or_newline(slice: &[u8]) -> Option<usize> {
    slice
        .iter()
        .position(|&v| v == b' ' || v == b'\n' || v == b'\r')
}

fn parse_int(data: &[u8], pos_sz: u32) -> Option<(u32, usize)> {
    data.first().map(|&first_b| {
        let neg = first_b == b'-';
        let start = (first_b == b'+' || neg) as usize;
        let (i, acc) = data[start..]
            .iter()
            .take_while(|&val| val.is_ascii_digit())
            .fold((0, 0), |(i, acc), &val| {
                (i + 1, acc * 10 + (val - b'0') as u32)
            });
        let res = if !neg { acc - 1 } else { pos_sz - acc };
        (res, i + start)
    })
}

fn parse_face_indices(
    face_str: &[u8],
    mode: &mut FaceMode,
    indices: &mut Vec<u32>,
    strides: &mut Vec<u8>,
    pos_sz: u32,
) -> Option<usize> {
    let mut data = face_str;
    let mut off = 0;

    off += data.iter().position(|&c| c != b' ')?;
    data = &face_str[off..];
    let (f0, end) = parse_int(data, pos_sz)?;
    off += end;
    data = &face_str[off..];
    off += data.iter().position(|&c| c == b' ')? + 1;
    data = &face_str[off..];
    off += data.iter().position(|&c| c != b' ')?;
    data = &face_str[off..];
    let (f1, end) = parse_int(data, pos_sz)?;
    off += end;
    data = &face_str[off..];
    off += data.iter().position(|&c| c == b' ')? + 1;
    data = &face_str[off..];
    off += data.iter().position(|&c| c != b' ')?;
    data = &face_str[off..];
    // let (f2, end) = parse_int(data, pos_sz).unwrap();
    // off += end;
    // let mut i = 3;
    let mut i = 2;
    indices.push(f0);
    indices.push(f1);
    // indices.push(f3);

    while let Some((v_i, mut endword)) = parse_int(data, pos_sz) {
        indices.push(v_i);
        i += 1;
        if endword == data.len() {
            break;
        }
        if data[endword] == b'/' {
            match find_blank_or_newline(&data[(endword + 1)..]) {
                Some(value) => endword += 1 + value,
                None => break,
            }
        }

        endword += data[endword..].iter().position(|&c| c != b' ')?;

        off += endword;
        if data[endword] == b'\r' || data[endword] == b'\n' {
            off += data[endword..]
                .iter()
                .position(|&c| c != b' ' && c != b'\r')?;
            break;
        }
        data = &data[endword..];
    }

    if i >= 3 && *mode != FaceMode::Polygon {
        if *mode == FaceMode::Undetermined {
            if i == 3 {
                *mode = FaceMode::Triangle;
            } else if i == 4 {
                *mode = FaceMode::Quad;
            } else {
                *mode = FaceMode::Polygon;
            }
        } else if *mode == FaceMode::Triangle && i != 3 {
            //add missing strides
            *strides = vec![3; (indices.len() - i) / 3];
            strides.reserve(2 * pos_sz as usize - strides.len());
            *mode = FaceMode::Polygon;
        } else if *mode == FaceMode::Quad && i != 4 {
            //add missing strides
            *strides = vec![4; (indices.len() - i) / 4];
            *mode = FaceMode::Polygon;
            strides.reserve(2 * pos_sz as usize - strides.len());
        }
    }
    if i >= 3 && *mode == FaceMode::Polygon {
        strides.push(i as u8);
    }
    Some(off)
}

pub fn load_obj_buf<B: BufRead, const BUFFER_SIZE: usize>(
    reader: &mut B,
    buf: &mut [u8; BUFFER_SIZE],
    mut start: usize,
) -> Result<(Vec<[f32; 3]>, SurfaceIndices), ()> {
    let mut vertices = Vec::new();
    let mut mode = FaceMode::Undetermined;
    let mut indices: Vec<u32> = Vec::new();
    let mut strides: Vec<u8> = Vec::new();
    let mut encountered_f = false;
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

        let mut i = 0;
        while i < last {
            match buf[i] {
                b'v' => match buf[i + 1] {
                    b' ' => {
                        let (off, pos) = unsafe { parse_float3(&buf[i + 2..]) };
                        vertices.push(pos);
                        i += off + 2;
                    }
                    _ => i += find_newline(&buf[i + 1..]).ok_or(())? + 2,
                },
                b'f' => {
                    if !encountered_f {
                        encountered_f = true;
                        // first estimate that `nf = 2 * nv`
                        indices.reserve(vertices.len() * 2 * 3);
                    }
                    let off = parse_face_indices(
                        &buf[i + 2..],
                        &mut mode,
                        &mut indices,
                        &mut strides,
                        vertices.len() as u32,
                    )
                    .ok_or(())?;
                    i += 2 + off;
                }
                _ => i += find_newline(&buf[i..]).ok_or(())? + 1,
            }
        }

        start = end - last;
        buf.copy_within(last..end, 0);
    }
    let indices = if mode == FaceMode::Polygon {
        (indices, strides).into()
    } else if mode == FaceMode::Quad {
        into_chunks::<4>(indices).into()
    } else {
        into_chunks::<3>(indices).into()
    };
    Ok((vertices, indices))
}

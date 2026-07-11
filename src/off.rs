use std::{
    fs::File,
    io::{BufReader, prelude::*},
    path::Path,
};

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
) -> usize {
    let mut off = 0;
    let mut data = face_str;
    while data.len() > 0 && data[0] == b' ' {
        data = &data[1..];
    }

    let (face_len, mut endword) = parse_int(data).unwrap();
    if face_len >= 3 && *mode != FaceMode::Polygon {
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
            *strides = vec![3; (indices.len() - face_len as usize) / 3];
            strides.reserve(3 * nf - strides.len());
            *mode = FaceMode::Polygon;
        } else if *mode == FaceMode::Quad && face_len != 4 {
            //add missing strides
            *strides = vec![4; (indices.len() - face_len as usize) / 4];
            *mode = FaceMode::Polygon;
            strides.reserve(2 * nf - strides.len());
        }
    }
    if face_len >= 3 && *mode == FaceMode::Polygon {
        strides.push(face_len as u8);
    }

    while endword < data.len() && data[endword] == b' ' {
        endword += 1;
    }
    off += endword;
    data = &data[endword..];

    for _ in 0..face_len {
        let (v, mut endword) = parse_int(data).unwrap();
        indices.push(v);
        endword += 1;
        while endword < data.len() && data[endword] == b' ' {
            endword += 1;
        }
        off += endword;
        data = &data[endword..];
    }
    off
}

fn parse_header(buf: &[u8]) -> (usize, usize, usize) {
    let (nv, mut endword) = parse_int(buf).unwrap();
    while endword < buf.len() && buf[endword] == b' ' {
        endword += 1;
    }
    let (nf, endword) = parse_int(&buf[endword..]).unwrap();
    (nv as usize, nf as usize, endword)
}

pub fn load_off(file_name: impl AsRef<Path>) -> (Vec<[f32; 3]>, SurfaceIndices) {
    let file = match File::open(file_name.as_ref()) {
        Ok(f) => f,
        Err(_e) => {
            panic!()
            //return Err(LoadError::OpenFileFailed);
        }
    };
    let mut reader = BufReader::new(file);
    load_off_buf(&mut reader)
}

pub fn load_off_buf<B>(reader: &mut B) -> (Vec<[f32; 3]>, SurfaceIndices)
where
    B: BufRead,
{
    let mut line_number = 0;
    let mut nv = 0;
    let mut nf = 0;
    let mut vertices = Vec::new();
    let mut mode = FaceMode::Undetermined;
    let mut indices: Vec<u32> = Vec::new();
    let mut strides: Vec<u8> = Vec::new();
    const BUFFER_SIZE: usize = 65536;
    let mut buf = [0; BUFFER_SIZE];
    let mut start = 0;
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
                    if buf.split_first_chunk::<3>().unwrap().0 == b"OFF" {
                        i += find_newline(&buf[3..]).unwrap() + 4;
                        continue;
                    } else {
                        let endword;
                        (nv, nf, endword) = parse_header(&buf[i..]);
                        vertices.reserve(nv);
                        indices.reserve(3 * nf);
                        line_number += 1;
                        i += find_newline(&buf[i + endword..]).unwrap() + endword + 1;
                    }
                } else if line_number < nv + 1 {
                    let (off, pos) = unsafe { parse_float3(&buf[i..]) };
                    line_number += 1;
                    vertices.push(pos);
                    i += off + 1;
                } else if line_number < 1 + nv + nf {
                    let off =
                        parse_face_indices(&buf[i..], &mut mode, &mut indices, &mut strides, nf);
                    line_number += 1;
                    i += 1 + off;
                } else {
                    break 'outer;
                }
            } else {
                i += find_newline(&buf[1..]).unwrap() + 1;
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
    (vertices, indices)
}

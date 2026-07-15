use std::io::BufRead;

use crate::{
    FaceMode, SurfaceIndices, into_chunks, parse_face_indices_list, parse_float3, parse_uint,
};

fn get_line_start(mut data: &[u8]) -> Option<usize> {
    if data.is_empty() {
        None
    } else if data[0] != b' ' && data[0] != b'#' {
        Some(0)
    } else {
        std::hint::cold_path();
        data = &data[1..];
        let mut i = data.iter().position(|&c| c != b' ')?;
        while data[i] == b'#' {
            i += 2 + data[i + 1..].iter().position(|&c| c != b'\n')?;
            i += data.iter().position(|&c| c != b' ')?;
        }
        Some(i + 1)
    }
}

fn parse_header(buf: &[u8]) -> Option<(usize, usize, usize)> {
    let (nv, mut endword) = parse_uint(buf)?;
    while endword < buf.len() && buf[endword] == b' ' {
        endword += 1;
    }
    let (nf, endword) = parse_uint(&buf[endword..])?;
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
                let end = data[3..].iter().position(|&v| v == b'\n').ok_or(())? + 4;
                data = &data[end..];
            }
            let endword;
            (nv, nf, endword) = parse_header(data).ok_or(())?;
            data = &data[endword..];
            vertices.reserve(nv);
            indices.reserve(nv + nf - 2);
            line_number += 1;
            let end = data.iter().position(|&v| v == b'\n').ok_or(())? + 1;
            data = &data[end..];
        }

        while line_number < nv + 1
            && let Some(line_start) = get_line_start(data)
        {
            data = &data[line_start..];
            let (off, pos) = unsafe { parse_float3(data) };
            data = &data[off + 1..];
            line_number += 1;
            vertices.push(pos);
        }

        while line_number < nv + nf + 1
            && let Some(line_start) = get_line_start(data)
        {
            data = &data[line_start..];
            parse_face_indices_list(&mut data, &mut mode, &mut indices, &mut strides, nf)
                .ok_or(())?;
            line_number += 1;
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

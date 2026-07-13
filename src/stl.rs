use std::io::BufRead;

use crate::SurfaceIndices;

pub fn load_stl_buf<B: BufRead, const BUFFER_SIZE: usize>(
    reader: &mut B,
    buf: &mut [u8; BUFFER_SIZE],
    mut start: usize,
) -> Result<(Vec<[f32; 3]>, SurfaceIndices), ()> {
    let mut nf = 0;
    let mut vertices = Vec::new();
    let mut first = true;
    const CHUNK_SIZE: usize = 50;
    'outer: while let Ok(size) = reader.read(&mut buf[start..]) {
        if size == 0 && size + start < CHUNK_SIZE {
            break;
        }

        let i = if first { 84 } else { 0 };
        if first {
            if buf[0..5] == [b's', b'o', b'l', b'i', b'd'] {
                return Err(());
            }
            nf = u32::from_le_bytes(*buf[80..].first_chunk::<4>().ok_or(())?);
            vertices = Vec::with_capacity(3 * nf as usize);
            first = false;
        }

        let (chunks, rem) = buf[i..].as_chunks::<CHUNK_SIZE>();
        for chunk in chunks {
            let off = 12;
            for i in 0..3 {
                let off = off * (i + 1);
                let vx = f32::from_le_bytes([
                    chunk[off],
                    chunk[off + 1],
                    chunk[off + 2],
                    chunk[off + 3],
                ]);
                let vy = f32::from_le_bytes([
                    chunk[off + 4],
                    chunk[off + 5],
                    chunk[off + 6],
                    chunk[off + 7],
                ]);
                let vz = f32::from_le_bytes([
                    chunk[off + 8],
                    chunk[off + 9],
                    chunk[off + 10],
                    chunk[off + 11],
                ]);
                vertices.push([vx, vy, vz]);
                if vertices.len() * 3 == nf as usize {
                    break 'outer;
                }
            }
        }

        let end = start + size;
        start = rem.len();
        let last = end - start;
        buf.copy_within(last..end, 0);
    }

    if vertices.len() * 3 != nf as usize {
        return Err(());
    }

    let indices = (0..nf)
        .map(|i| [3 * i, 3 * i + 1, 3 * i + 2])
        .collect::<Vec<_>>()
        .into();
    Ok((vertices, indices))
}

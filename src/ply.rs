use std::{io::BufRead, str::FromStr};

use crate::{
    FaceMode, SurfaceIndices, find_blank_or_newline, find_blank_space, find_newline, into_chunks,
    parse_face_indices_list, parse_uint,
};

enum Format {
    Ascii,
    BigEndian,
    LittleEndian,
}

#[derive(Clone, Copy)]
enum RawType {
    Char,
    UChar,
    Short,
    UShort,
    Int,
    UInt,
    Float,
    Double,
}

#[derive(Clone, Copy)]
enum Type {
    Single(RawType),
    List(RawType, RawType),
}

impl RawType {
    fn parse(data: &[u8]) -> Option<(RawType, usize)> {
        match data.split_first_chunk::<4>() {
            Some((b"char", s)) if s.starts_with(b" ") => Some((RawType::Char, 5)),
            Some((b"ucha", s)) if s.starts_with(b"r ") => Some((RawType::UChar, 6)),
            Some((b"shor", s)) if s.starts_with(b"t ") => Some((RawType::Short, 6)),
            Some((b"usho", s)) if s.starts_with(b"rt ") => Some((RawType::UShort, 7)),
            Some((b"int ", _s)) => Some((RawType::Int, 4)),
            Some((b"uint", s)) if s.starts_with(b" ") => Some((RawType::UInt, 5)),
            Some((b"floa", s)) if s.starts_with(b"t ") => Some((RawType::Float, 6)),
            Some((b"doub", s)) if s.starts_with(b"le") => Some((RawType::Double, 7)),
            _ => None,
        }
    }

    fn len(&self) -> u8 {
        match self {
            RawType::Char | RawType::UChar => 1,
            RawType::Short | RawType::UShort => 2,
            RawType::Int | RawType::UInt | RawType::Float => 4,
            RawType::Double => 8,
        }
    }

    fn parse_binary_uint(&self, data: &[u8], big_endian: bool) -> Result<Option<u32>, ()> {
        match self {
            RawType::Char | RawType::UChar => Ok(data.first_chunk::<1>().map(|&b| {
                if big_endian {
                    u8::from_be_bytes(b) as u32
                } else {
                    u8::from_le_bytes(b) as u32
                }
            })),
            RawType::Short | RawType::UShort => Ok(data.first_chunk::<2>().map(|&b| {
                if big_endian {
                    u16::from_be_bytes(b) as u32
                } else {
                    u16::from_le_bytes(b) as u32
                }
            })),
            RawType::Int | RawType::UInt => Ok(data.first_chunk::<4>().map(|&b| {
                if big_endian {
                    u32::from_be_bytes(b)
                } else {
                    u32::from_le_bytes(b)
                }
            })),
            RawType::Double | RawType::Float => Err(()),
        }
    }

    fn parse_binary_float(&self, data: &[u8], big_endian: bool) -> Option<f32> {
        match self {
            RawType::Char | RawType::UChar => data.first_chunk::<1>().map(|&b| {
                if big_endian {
                    u8::from_be_bytes(b) as f32
                } else {
                    u8::from_le_bytes(b) as f32
                }
            }),
            RawType::Short | RawType::UShort => data.first_chunk::<2>().map(|&b| {
                if big_endian {
                    u16::from_be_bytes(b) as f32
                } else {
                    u16::from_le_bytes(b) as f32
                }
            }),
            RawType::Int | RawType::UInt => data.first_chunk::<4>().map(|&b| {
                if big_endian {
                    u32::from_be_bytes(b) as f32
                } else {
                    u32::from_le_bytes(b) as f32
                }
            }),
            RawType::Float => data.first_chunk::<4>().map(|&b| {
                if big_endian {
                    f32::from_be_bytes(b)
                } else {
                    f32::from_le_bytes(b)
                }
            }),
            RawType::Double => data.first_chunk::<8>().map(|&b| {
                if big_endian {
                    f64::from_be_bytes(b) as f32
                } else {
                    f64::from_le_bytes(b) as f32
                }
            }),
        }
    }
}

impl Type {
    fn parse(data: &[u8]) -> Option<(Type, usize)> {
        match data.split_first_chunk::<4>() {
            Some((b"char", s)) if s.starts_with(b" ") => Some((Type::Single(RawType::Char), 5)),
            Some((b"ucha", s)) if s.starts_with(b"r ") => Some((Type::Single(RawType::UChar), 6)),
            Some((b"shor", s)) if s.starts_with(b"t ") => Some((Type::Single(RawType::Short), 6)),
            Some((b"usho", s)) if s.starts_with(b"rt ") => Some((Type::Single(RawType::UShort), 7)),
            Some((b"int ", _s)) => Some((Type::Single(RawType::Int), 4)),
            Some((b"uint", s)) if s.starts_with(b" ") => Some((Type::Single(RawType::UInt), 5)),
            Some((b"floa", s)) if s.starts_with(b"t ") => Some((Type::Single(RawType::Float), 6)),
            Some((b"doub", s)) if s.starts_with(b"le ") => Some((Type::Single(RawType::Double), 7)),
            Some((b"list", s)) if s.starts_with(b" ") => {
                let mut parsed_len = 1 + s[1..].iter().position(|&c| c != b' ')?;
                let (type1, l1) = RawType::parse(&s[parsed_len..])?;
                parsed_len += l1;
                parsed_len += s[parsed_len..].iter().position(|&c| c != b' ')?;
                let (type2, l2) = RawType::parse(&s[parsed_len..])?;
                parsed_len += l2;
                Some((Type::List(type1, type2), 4 + parsed_len))
            }
            _ => None,
        }
    }

    fn skip_ascii(&self, data: &[u8]) -> Option<usize> {
        match self {
            Type::Single(_) => find_blank_space(data),
            Type::List(_, _) => {
                let (n, mut i) = parse_uint(data)?;
                for _ in 0..n {
                    let mut found = false;
                    i += data[i..].iter().position(|&c| {
                        found |= c != b' ';
                        found && c == b' '
                    })?;
                }
                i += data[i..].iter().position(|&c| c != b' ')?;
                Some(i)
            }
        }
    }

    fn skip_binary(&self, data: &[u8], big_endian: bool) -> Result<Option<usize>, ()> {
        match self {
            Type::Single(t) => {
                if data.len() >= t.len() as usize {
                    Ok(Some(t.len() as usize))
                } else {
                    Ok(None)
                }
            }
            Type::List(t1, t2) => {
                let n = t1.parse_binary_uint(data, big_endian)?;
                Ok(n.and_then(|n| {
                    let list_byte_len = t1.len() as usize + n as usize * t2.len() as usize;
                    if data.len() >= list_byte_len {
                        Some(list_byte_len)
                    } else {
                        None
                    }
                }))
            }
        }
    }
}

#[derive(Default)]
struct HeadingInfos {
    format: Option<Format>,
    nv: u32,
    v_x_stride_i: Option<u32>,
    v_y_stride_i: Option<u32>,
    v_z_stride_i: Option<u32>,
    v_stride: Vec<Type>,
    nf: u32,
    i_stride_i: Option<u32>,
    i_stride: Vec<Type>,

    v_first_over_f: bool,
    useless_before: Vec<(u32, Vec<Type>)>,
    useless_between: Vec<(u32, Vec<Type>)>,
}

struct AsciiInfos {
    useless_before: u32,
    vertex_start: u32,
    nv: u32,
    v_x_stride_i: u32,
    v_y_stride_i: u32,
    v_z_stride_i: u32,
    v_stride: Vec<Type>,
    face_start: u32,
    nf: u32,
    i_stride_i: u32,
    i_stride: Vec<Type>,
}

struct BinaryInfos {
    big_endian: bool,
    nv: u32,
    v_x_stride_i: u32,
    v_y_stride_i: u32,
    v_z_stride_i: u32,
    v_stride: Vec<Type>,
    nf: u32,
    i_stride_i: u32,
    i_stride: Vec<Type>,

    v_first_over_f: bool,
    useless_before: Vec<(u32, Vec<Type>)>,
    useless_between: Vec<(u32, Vec<Type>)>,
}

enum ParsingState {
    Header(HeadingInfos),
    Ascii(AsciiInfos),
    Binary(BinaryInfos),
}

impl ParsingState {
    fn new() -> Self {
        ParsingState::Header(HeadingInfos::default())
    }

    fn finalize(self) -> Result<Self, ()> {
        if let ParsingState::Header(infos) = self {
            let nv = infos.nv;
            let nf = infos.nf;
            let v_stride = infos.v_stride;
            let v_x_stride_i = infos.v_x_stride_i.ok_or(())?;
            let v_y_stride_i = infos.v_y_stride_i.ok_or(())?;
            let v_z_stride_i = infos.v_z_stride_i.ok_or(())?;
            let i_stride = infos.i_stride;
            let i_stride_i = infos.i_stride_i.ok_or(())?;
            match infos.format.ok_or(())? {
                Format::Ascii => {
                    let useless_before = infos.useless_before.iter().fold(0, |acc, (n, _)| acc + n);
                    let useless_between =
                        infos.useless_between.iter().fold(0, |acc, (n, _)| acc + n);
                    let vertex_start = if infos.v_first_over_f {
                        useless_before
                    } else {
                        useless_before + useless_between + infos.nf
                    };
                    let face_start = if !infos.v_first_over_f {
                        useless_before
                    } else {
                        useless_before + useless_between + infos.nv
                    };
                    Ok(ParsingState::Ascii(AsciiInfos {
                        useless_before,
                        vertex_start,
                        face_start,
                        nv,
                        nf,
                        v_stride,
                        v_x_stride_i,
                        v_y_stride_i,
                        v_z_stride_i,
                        i_stride,
                        i_stride_i,
                    }))
                }
                format @ Format::BigEndian | format @ Format::LittleEndian => {
                    let big_endian = matches!(format, Format::BigEndian);
                    Ok(ParsingState::Binary(BinaryInfos {
                        big_endian,
                        nv,
                        v_x_stride_i,
                        v_y_stride_i,
                        v_z_stride_i,
                        v_stride,
                        nf,
                        i_stride_i,
                        i_stride,
                        v_first_over_f: infos.v_first_over_f,
                        useless_before: infos.useless_before,
                        useless_between: infos.useless_between,
                    }))
                }
            }
        } else {
            Err(())
        }
    }
}

enum HeaderSection {
    Format,
    Vertex,
    Face,
    Useless,
}

fn parse_header(
    mut data: &[u8],
    cursor: &mut usize,
    head: &mut HeadingInfos,
    section: &mut HeaderSection,
) -> Result<bool, ()> {
    while let Some((off, line_end)) = get_next_line_start_and_end_header(data, cursor) {
        data = &data[off..];
        *cursor += line_end;
        match section {
            HeaderSection::Format => match data.split_first_chunk::<7>() {
                Some((b"format ", data)) => {
                    let format = if data.starts_with(b"ascii 1.0") {
                        Format::Ascii
                    } else if data.starts_with(b"binary_little_endian 1.0") {
                        Format::LittleEndian
                    } else if data.starts_with(b"binary_big_endian 1.0") {
                        Format::BigEndian
                    } else {
                        return Err(());
                    };
                    head.format = Some(format);
                }
                Some((b"element", s)) if s.first() == Some(&b' ') => {
                    if s[1..].starts_with(b"vertex ") {
                        head.nv = parse_uint(&s[8..]).ok_or(())?.0;
                        head.v_first_over_f = true;
                        *section = HeaderSection::Vertex;
                    } else if s[1..].starts_with(b"face ") {
                        head.nf = parse_uint(&s[6..]).ok_or(())?.0;
                        head.v_first_over_f = false;
                        *section = HeaderSection::Face;
                    } else {
                        let mut found_blank = false;
                        let int_start = s
                            .iter()
                            .position(|&c| {
                                found_blank |= c == b' ';
                                found_blank && c != b' '
                            })
                            .ok_or(())?;
                        let n = parse_uint(&s[int_start..]).ok_or(())?.0;
                        head.useless_before.push((n, Vec::new()));
                        *section = HeaderSection::Useless;
                    }
                }
                _ => return Err(()),
            },
            HeaderSection::Vertex => match data.split_first_chunk::<8>() {
                Some((b"element ", s)) => {
                    if s.starts_with(b"vertex ") {
                        return Err(());
                    } else if s[..].starts_with(b"face ") {
                        if head.nf != 0 {
                            return Err(());
                        }
                        head.nf = parse_uint(&s[5..]).ok_or(())?.0;
                        *section = HeaderSection::Face;
                    } else {
                        if head.nf == 0 {
                            let mut found_blank = false;
                            let int_start = s
                                .iter()
                                .position(|&c| {
                                    found_blank |= c == b' ';
                                    found_blank && c != b' '
                                })
                                .ok_or(())?;
                            let n = parse_uint(&s[int_start..]).ok_or(())?.0;
                            head.useless_between.push((n, Vec::new()));
                        }
                        *section = HeaderSection::Useless;
                    }
                }
                Some((b"property", s)) if s.starts_with(b" ") => {
                    let (typ, l) = Type::parse(&s[1..]).ok_or(())?;
                    let name_end = s[1 + l..]
                        .iter()
                        .position(|&c| c == b' ' || c == b'\r' || c == b'\n')
                        .ok_or(())?;
                    let name = &s[1 + l..1 + l + name_end];
                    if let Type::Single(_) = typ
                        && (name == b"x" || name == b"y" || name == b"z")
                    {
                        match name {
                            b"x" => {
                                head.v_x_stride_i = Some(head.v_stride.len() as u32);
                            }
                            b"y" => {
                                head.v_y_stride_i = Some(head.v_stride.len() as u32);
                            }
                            b"z" => {
                                head.v_z_stride_i = Some(head.v_stride.len() as u32);
                            }
                            _ => unreachable!(),
                        }
                    }
                    head.v_stride.push(typ);
                }
                Some((b"end_head", s)) if s.starts_with(b"er") => return Ok(true),
                _ => return Err(()),
            },
            HeaderSection::Face => match data.split_first_chunk::<8>() {
                Some((b"element ", s)) => {
                    if s[..].starts_with(b"face ") {
                        return Err(());
                    } else if s[..].starts_with(b"vertex ") {
                        if head.nv != 0 {
                            return Err(());
                        }
                        head.nv = parse_uint(&s[7..]).ok_or(())?.0;
                        *section = HeaderSection::Vertex;
                    } else {
                        if head.nv == 0 {
                            let mut found_blank = false;
                            let int_start = s
                                .iter()
                                .position(|&c| {
                                    found_blank |= c == b' ';
                                    found_blank && c != b' '
                                })
                                .ok_or(())?;
                            let n = parse_uint(&s[int_start..]).ok_or(())?.0;
                            head.useless_between.push((n, Vec::new()));
                        }
                        *section = HeaderSection::Useless;
                    }
                }
                Some((b"property", s)) if s.starts_with(b" ") => {
                    let (typ, l) = Type::parse(&s[1..]).ok_or(())?;
                    let name_end = s[1 + l..]
                        .iter()
                        .position(|&c| c == b' ' || c == b'\r' || c == b'\n')
                        .ok_or(())?;
                    let name = &s[1 + l..1 + l + name_end];
                    if let Type::List(_, _) = typ
                        && (name == b"vertex_index" || name == b"vertex_indices")
                    {
                        head.i_stride_i = Some(head.i_stride.len() as u32);
                    }
                    head.i_stride.push(typ);
                }
                Some((b"end_head", s)) if s.starts_with(b"er") => return Ok(true),
                _ => return Err(()),
            },
            HeaderSection::Useless => match data.split_first_chunk::<8>() {
                Some((b"element ", s)) => {
                    if s[..].starts_with(b"face ") {
                        if head.nf != 0 {
                            return Err(());
                        }
                        if head.nv != 0 {
                            head.v_first_over_f = true;
                        }
                        head.nf = parse_uint(&s[5..]).ok_or(())?.0;
                        *section = HeaderSection::Face;
                    } else if s[..].starts_with(b"vertex ") {
                        if head.nv != 0 {
                            return Err(());
                        }
                        if head.nf != 0 {
                            head.v_first_over_f = false;
                        }
                        head.nv = parse_uint(&s[8..]).ok_or(())?.0;
                        *section = HeaderSection::Vertex;
                    } else {
                        let mut found_blank = false;
                        let int_start = s
                            .iter()
                            .position(|&c| {
                                found_blank |= c == b' ';
                                found_blank && c != b' '
                            })
                            .ok_or(())?;
                        let n = parse_uint(&s[int_start..]).ok_or(())?.0;
                        if !head.useless_between.is_empty() {
                            head.useless_between.push((n, Vec::new()));
                        } else {
                            head.useless_before.push((n, Vec::new()));
                        }
                        *section = HeaderSection::Useless;
                    }
                }
                Some((b"property", s)) => {
                    if s.starts_with(b" ") && head.nf == 0 || head.nv == 0 {
                        let (typ, _l) = Type::parse(&s[1..]).ok_or(())?;
                        if let Some((_n, strides)) = head.useless_between.last_mut() {
                            strides.push(typ);
                        } else {
                            head.useless_before.last_mut().unwrap().1.push(typ);
                        }
                    }
                }
                Some((b"end_head", s)) if s.starts_with(b"er") => return Ok(true),
                _ => return Err(()),
            },
        }
        data = &data[line_end..];
    }
    Ok(false)
}

fn get_next_line_start_and_end_header(data: &[u8], cursor: &mut usize) -> Option<(usize, usize)> {
    let mut i = 0;
    while i < data.len() {
        let char = data[i];
        if char != b' ' {
            if char == b'c' || char == b'o' || char == b'\n' || char == b'\r' {
                match data[i..].iter().position(|&c| c == b'\n') {
                    Some(off) => i += off + 1,
                    None => {
                        *cursor += i;
                        return None;
                    }
                }
            } else {
                *cursor += i;
                return data[i..]
                    .iter()
                    .position(|&c| c == b'\n')
                    .map(|off| (i, off + 1));
            }
        } else {
            i += 1;
        }
    }
    *cursor += i;
    None
}

fn parse_ascii(
    mut data: &[u8],
    cursor: &mut usize,
    infos: &AsciiInfos,
    line: &mut usize,
    vertices: &mut Vec<[f32; 3]>,
    indices: &mut Vec<u32>,
    strides: &mut Vec<u8>,
    mode: &mut FaceMode,
) -> Result<bool, ()> {
    let mut last = data.len() - 1;
    while data[last] != b'\n' && last > 0 {
        last -= 1;
    }
    if data[last] != b'\n' {
        return Err(());
    }
    last += 1;
    data = &data[..last];
    *cursor += last;

    while !data.is_empty() {
        if (*line as u32) < infos.useless_before {
            while (*line as u32) < u32::min(infos.face_start, infos.vertex_start)
                && !data.is_empty()
            {
                if data[0] == b'\n' {
                    *line += 1;
                }
                data = &data[1..];
            }
        } else {
            if (*line as u32) >= infos.face_start && (*line as u32) < infos.face_start + infos.nf {
                while (*line as u32) < infos.face_start + infos.nf && !data.is_empty() {
                    let mut i = 0;
                    for typ in &infos.i_stride[..infos.i_stride_i as usize] {
                        i += typ.skip_ascii(&data[i..]).ok_or(())?;
                    }
                    data = &data[i..];
                    parse_face_indices_list(&mut data, mode, indices, strides, infos.nf as usize)
                        .ok_or(())?;
                    *line += 1;
                }
            } else if (*line as u32) >= infos.vertex_start
                && (*line as u32) < infos.vertex_start + infos.nv
            {
                while (*line as u32) < infos.vertex_start + infos.nv && !data.is_empty() {
                    let mut res = [0., 0., 0.];
                    let max_i = infos
                        .v_x_stride_i
                        .max(infos.v_y_stride_i)
                        .max(infos.v_z_stride_i)
                        + 1;
                    let mut i = 0;

                    for (index, typ) in infos.v_stride[..max_i as usize].iter().enumerate() {
                        if index == infos.v_x_stride_i as usize {
                            let (f, acc) = unsafe { parse_float(&data[i..]).ok_or(())? };
                            res[0] = f;
                            i += acc;
                        } else if index == infos.v_y_stride_i as usize {
                            let (f, acc) = unsafe { parse_float(&data[i..]).ok_or(())? };
                            res[1] = f;
                            i += acc;
                        } else if index == infos.v_z_stride_i as usize {
                            let (f, acc) = unsafe { parse_float(&data[i..]).ok_or(())? };
                            res[2] = f;
                            i += acc;
                        } else {
                            i += typ.skip_ascii(&data[i..]).ok_or(())?;
                        }
                    }
                    vertices.push(res);
                    *line += 1;
                    i += find_newline(&data[i..]).ok_or(())? + 1;
                    data = &data[i..];
                }
            } else if (*line as u32) < u32::max(infos.vertex_start, infos.face_start) {
                while (*line as u32) < u32::max(infos.face_start, infos.vertex_start)
                    && !data.is_empty()
                {
                    if data[0] == b'\n' {
                        *line += 1;
                    }
                    data = &data[1..];
                }
            } else {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn skip_elem_binary(
    data: &mut &[u8],
    types: &[Type],
    infos: &BinaryInfos,
) -> Result<Option<usize>, ()> {
    let mut elem_bytes = 0;
    for typ in types {
        if let Some(size) = typ.skip_binary(data, infos.big_endian)? {
            *data = &data[size..];
            elem_bytes += size;
        } else {
            return Ok(None);
        }
    }
    Ok(Some(elem_bytes))
}

fn parse_vertex_binary(
    data: &mut &[u8],
    infos: &BinaryInfos,
    vertices: &mut Vec<[f32; 3]>,
) -> Result<Option<usize>, ()> {
    let mut elem_bytes = 0;
    let mut res = [0., 0., 0.];
    for (i, typ) in infos.v_stride.iter().enumerate() {
        if i == infos.v_x_stride_i as usize {
            match typ {
                Type::Single(t) => {
                    res[0] = match t.parse_binary_float(data, infos.big_endian) {
                        Some(v) => v,
                        None => return Ok(None),
                    };
                    elem_bytes += t.len() as usize;
                    *data = &data[t.len() as usize..];
                }
                Type::List(_, _) => return Err(()),
            }
        } else if i == infos.v_y_stride_i as usize {
            match typ {
                Type::Single(t) => {
                    res[1] = match t.parse_binary_float(data, infos.big_endian) {
                        Some(v) => v,
                        None => return Ok(None),
                    };
                    elem_bytes += t.len() as usize;
                    *data = &data[t.len() as usize..];
                }
                Type::List(_, _) => return Err(()),
            }
        } else if i == infos.v_z_stride_i as usize {
            match typ {
                Type::Single(t) => {
                    res[2] = match t.parse_binary_float(data, infos.big_endian) {
                        Some(v) => v,
                        None => return Ok(None),
                    };
                    elem_bytes += t.len() as usize;
                    *data = &data[t.len() as usize..];
                }
                Type::List(_, _) => return Err(()),
            }
        } else {
            if let Some(size) = typ.skip_binary(data, infos.big_endian)? {
                *data = &data[size..];
                elem_bytes += size;
            } else {
                return Ok(None);
            }
        }
    }
    vertices.push(res);
    Ok(Some(elem_bytes))
}

fn parse_face_binary(
    data: &mut &[u8],
    infos: &BinaryInfos,
    indices: &mut Vec<u32>,
    strides: &mut Vec<u8>,
    mode: &mut FaceMode,
) -> Result<Option<usize>, ()> {
    let mut elem_bytes = 0;
    let orig_len = indices.len();
    let mut face_len = 0;
    for (i, typ) in infos.i_stride.iter().enumerate() {
        if i == infos.i_stride_i as usize {
            match typ {
                Type::Single(_) => return Err(()),
                Type::List(t1, t2) => {
                    if let Some(n) = t1.parse_binary_uint(data, infos.big_endian)? {
                        *data = &data[t1.len() as usize..];
                        face_len = n;
                        for _ in 0..n {
                            if let Some(index) = t2.parse_binary_uint(data, infos.big_endian)? {
                                *data = &data[t2.len() as usize..];
                                indices.push(index);
                            } else {
                                indices.truncate(orig_len);
                                return Ok(None);
                            }
                        }
                        elem_bytes += t1.len() as usize + t2.len() as usize * n as usize;
                    } else {
                        return Ok(None);
                    }
                }
            }
        } else {
            if let Some(size) = typ.skip_binary(data, infos.big_endian)? {
                *data = &data[size..];
                elem_bytes += size;
            } else {
                indices.truncate(orig_len);
                return Ok(None);
            }
        }
    }
    if face_len < 3 {
        return Err(());
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
            *strides = vec![3; (indices.len() - face_len as usize) / 3];
            strides.reserve(infos.nf as usize - strides.len());
            *mode = FaceMode::Polygon;
        } else if *mode == FaceMode::Quad && face_len != 4 {
            *strides = vec![4; (indices.len() - face_len as usize) / 4];
            *mode = FaceMode::Polygon;
            strides.reserve(infos.nf as usize - strides.len());
        }
    }
    if *mode == FaceMode::Polygon {
        strides.push(face_len as u8);
    }
    Ok(Some(elem_bytes))
}

fn parse_binary(
    mut data: &[u8],
    cursor: &mut usize,
    infos: &BinaryInfos,
    n_elem: &mut usize,
    vertices: &mut Vec<[f32; 3]>,
    indices: &mut Vec<u32>,
    strides: &mut Vec<u8>,
    mode: &mut FaceMode,
) -> Result<bool, ()> {
    let mut n_tot = 0;
    for (n, types) in &infos.useless_before {
        n_tot += n;
        while *n_elem < n_tot as usize {
            if let Some(elem_bytes) = skip_elem_binary(&mut data, types, infos)? {
                *cursor += elem_bytes;
                *n_elem += 1;
            } else {
                return Ok(false);
            }
        }
    }
    if infos.v_first_over_f {
        n_tot += infos.nv;
        while *n_elem < n_tot as usize {
            if let Some(elem_bytes) = parse_vertex_binary(&mut data, infos, vertices)? {
                *cursor += elem_bytes;
                *n_elem += 1;
            } else {
                return Ok(false);
            }
        }
    } else {
        n_tot += infos.nv;
        while *n_elem < n_tot as usize {
            if let Some(elem_bytes) = parse_face_binary(&mut data, infos, indices, strides, mode)? {
                *cursor += elem_bytes;
                *n_elem += 1;
            } else {
                return Ok(false);
            }
        }
    }
    for (n, types) in &infos.useless_between {
        n_tot += n;
        while *n_elem < n_tot as usize {
            if let Some(elem_bytes) = skip_elem_binary(&mut data, types, infos)? {
                *cursor += elem_bytes;
                *n_elem += 1;
            } else {
                return Ok(false);
            }
        }
    }
    if !infos.v_first_over_f {
        n_tot += infos.nv;
        while *n_elem < n_tot as usize {
            if let Some(elem_bytes) = parse_vertex_binary(&mut data, infos, vertices)? {
                *cursor += elem_bytes;
                *n_elem += 1;
            } else {
                return Ok(false);
            }
        }
    } else {
        n_tot += infos.nv;
        while *n_elem < n_tot as usize {
            if let Some(elem_bytes) = parse_face_binary(&mut data, infos, indices, strides, mode)? {
                *cursor += elem_bytes;
                *n_elem += 1;
            } else {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

unsafe fn parse_float(slice: &[u8]) -> Option<(f32, usize)> {
    unsafe {
        let mut i = 0;
        while slice[i] == b' ' {
            i += 1;
        }
        let sep = find_blank_or_newline(&slice[i + 1..])? + 1;
        let f = FromStr::from_str(std::str::from_utf8_unchecked(&slice[i..(i + sep)])).ok()?;
        i += sep + 1;
        if let Some(b' ') = slice.get(i) {
            i += slice[i..].iter().position(|&c| c != b' ')?;
        }

        Some((f, i))
    }
}

pub fn load_ply_buf<B: BufRead, const BUFFER_SIZE: usize>(
    reader: &mut B,
    buf: &mut [u8; BUFFER_SIZE],
    mut start: usize,
) -> Result<(Vec<[f32; 3]>, SurfaceIndices), ()> {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let mut strides: Vec<u8> = Vec::new();
    let mut mode = FaceMode::Undetermined;
    let mut parsing_tracker = 0;
    let mut first = true;
    let mut parsing_state = ParsingState::new();
    let mut header_parsing_state = HeaderSection::Format;

    'outer: while let Ok(size) = reader.read(&mut buf[start..]) {
        if size == 0 && start == 0 {
            break;
        }
        let end = size + start;
        let mut last = 0;

        let mut data = if first {
            first = false;
            let (prelude, data) = buf.split_first_chunk::<3>().ok_or(())?;
            assert!(prelude == b"ply");
            last += 3;
            &data[..end - 3]
        } else {
            &buf[..end]
        };

        loop {
            match &mut parsing_state {
                ParsingState::Header(head) => {
                    let old_last = last;
                    let done = parse_header(data, &mut last, head, &mut header_parsing_state)?;
                    let advanced = last - old_last;
                    if done {
                        let nv = head.nv as usize;
                        let nf = head.nf as usize;
                        vertices.reserve(nv);
                        indices.reserve(nv + nf - 2);
                        parsing_state = parsing_state.finalize()?;
                        data = &data[advanced..];
                    } else {
                        break;
                    }
                }
                ParsingState::Ascii(infos) => {
                    let done = parse_ascii(
                        data,
                        &mut last,
                        infos,
                        &mut parsing_tracker,
                        &mut vertices,
                        &mut indices,
                        &mut strides,
                        &mut mode,
                    )?;
                    if done {
                        let valid = {
                            (infos.nv as usize == vertices.len())
                                && match mode {
                                    FaceMode::Undetermined => panic!(),
                                    FaceMode::Triangle => infos.nf as usize == indices.len() / 3,
                                    FaceMode::Quad => infos.nf as usize == indices.len() / 4,
                                    FaceMode::Polygon => infos.nf as usize == strides.len(),
                                }
                        };
                        if !valid {
                            return Err(());
                        }
                        break 'outer;
                    } else {
                        break;
                    }
                }
                ParsingState::Binary(infos) => {
                    let done = parse_binary(
                        data,
                        &mut last,
                        infos,
                        &mut parsing_tracker,
                        &mut vertices,
                        &mut indices,
                        &mut strides,
                        &mut mode,
                    )?;
                    if done {
                        let valid = {
                            (infos.nv as usize == vertices.len())
                                && match mode {
                                    FaceMode::Undetermined => panic!(),
                                    FaceMode::Triangle => infos.nf as usize == indices.len() / 3,
                                    FaceMode::Quad => infos.nf as usize == indices.len() / 4,
                                    FaceMode::Polygon => infos.nf as usize == strides.len(),
                                }
                        };
                        if !valid {
                            return Err(());
                        }
                        break 'outer;
                    } else {
                        break;
                    }
                }
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

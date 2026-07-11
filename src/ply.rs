use std::{
    fs::File,
    io::{BufReader, prelude::*},
    path::Path,
    str::FromStr,
};

use crate::{SurfaceIndices, ply::Type::Single};

enum Format {
    Ascii,
    BigEndian,
    LittleEndian,
}

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

enum Type {
    Single(RawType),
    List(RawType, RawType),
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
    useless_before_first: Vec<(u32, Vec<Type>)>,
    useless_between: Vec<(u32, Vec<Type>)>,
}

#[derive(Default)]
struct AsciiInfos {
    useless_before_first: Vec<(u32, Vec<Type>)>,
    vertex_start: u32,
    nv: u32,
    v_x_stride_i: u32,
    v_y_stride_i: u32,
    v_z_stride_i: u32,
    v_lines: u32,
    face_start: u32,
    nf: u32,
    i_stride_i: u32,
    i_stride: u32,
}

#[derive(Default)]
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
    useless_before_first: Vec<(u32, Vec<Type>)>,
    useless_between: Vec<(u32, Vec<Type>)>,
}

enum HeaderParsingState {
    Format,
    Vertex,
    Face,
    Useless,
}

fn parse_header(
    mut data: &[u8],
    cursor: &mut usize,
    head: &mut HeadingInfos,
    state: &mut HeaderParsingState,
) -> Result<bool, ()> {
    while let Some((off, line_end)) = get_next_line_start_and_end(data, cursor) {
        data = &data[off..];
        match state {
            HeaderParsingState::Format => match data.split_first_chunk::<7>() {
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
                        head.nv = parse_int(&s[8..]).ok_or(())?.0;
                        head.v_first_over_f = true;
                        *state = HeaderParsingState::Vertex;
                    } else if s[1..].starts_with(b"face ") {
                        head.nf = parse_int(&s[6..]).ok_or(())?.0;
                        head.v_first_over_f = false;
                        *state = HeaderParsingState::Face;
                    } else {
                        let mut found_blank = false;
                        let int_start = s
                            .iter()
                            .position(|&c| {
                                found_blank |= c == b' ';
                                found_blank && c != b' '
                            })
                            .ok_or(())?;
                        let n = parse_int(&s[int_start..]).ok_or(())?.0;
                        head.useless_before_first.push((n, Vec::new()));
                        *state = HeaderParsingState::Useless;
                    }
                }
                _ => return Err(()),
            },
            HeaderParsingState::Vertex => match data.split_first_chunk::<8>() {
                Some((b"element ", s)) => {
                    if s[..].starts_with(b"vertex ") {
                        return Err(());
                    } else if s[..].starts_with(b"face ") {
                        if head.nf != 0 {
                            return Err(());
                        }
                        head.nf = parse_int(&s[5..]).ok_or(())?.0;
                        *state = HeaderParsingState::Face;
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
                            let n = parse_int(&s[int_start..]).ok_or(())?.0;
                            head.useless_between.push((n, Vec::new()));
                        }
                        *state = HeaderParsingState::Useless;
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
            HeaderParsingState::Face => match data.split_first_chunk::<8>() {
                Some((b"element ", s)) => {
                    if s[..].starts_with(b"face ") {
                        return Err(());
                    } else if s[..].starts_with(b"vertex ") {
                        if head.nv != 0 {
                            return Err(());
                        }
                        head.nv = parse_int(&s[7..]).ok_or(())?.0;
                        *state = HeaderParsingState::Vertex;
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
                            let n = parse_int(&s[int_start..]).ok_or(())?.0;
                            head.useless_between.push((n, Vec::new()));
                        }
                        *state = HeaderParsingState::Useless;
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
            HeaderParsingState::Useless => match data.split_first_chunk::<8>() {
                Some((b"element ", s)) => {
                    if s[..].starts_with(b"face ") {
                        if head.nf != 0 {
                            return Err(());
                        }
                        if head.nv != 0 {
                            head.v_first_over_f = true;
                        }
                        head.nf = parse_int(&s[6..]).ok_or(())?.0;
                        *state = HeaderParsingState::Face;
                    } else if s[..].starts_with(b"vertex ") {
                        if head.nv != 0 {
                            return Err(());
                        }
                        if head.nf != 0 {
                            head.v_first_over_f = false;
                        }
                        head.nv = parse_int(&s[8..]).ok_or(())?.0;
                        *state = HeaderParsingState::Vertex;
                    } else {
                        let mut found_blank = false;
                        let int_start = s
                            .iter()
                            .position(|&c| {
                                found_blank |= c == b' ';
                                found_blank && c != b' '
                            })
                            .ok_or(())?;
                        let n = parse_int(&s[int_start..]).ok_or(())?.0;
                        if head.useless_between.len() != 0 {
                            head.useless_between.push((n, Vec::new()));
                        } else {
                            head.useless_before_first.push((n, Vec::new()));
                        }
                        *state = HeaderParsingState::Useless;
                    }
                }
                Some((b"property", s)) => {
                    if s.starts_with(b" ") {
                        if head.nf == 0 || head.nv == 0 {
                            let (typ, _l) = Type::parse(&s[1..]).ok_or(())?;
                            if let Some((_n, strides)) = head.useless_between.last_mut() {
                                strides.push(typ);
                            } else {
                                head.useless_before_first.last_mut().unwrap().1.push(typ);
                            }
                        }
                    }
                }
                Some((b"end_head", s)) if s.starts_with(b"er") => return Ok(true),
                _ => return Err(()),
            },
        }
        *cursor += line_end;
        data = &data[line_end..];
    }
    Ok(false)
}

fn get_next_line_start_and_end(data: &[u8], cursor: &mut usize) -> Option<(usize, usize)> {
    let mut i = 0;
    while i < data.len() {
        let char = data[i];
        if char != b' ' {
            if char == b'c' || char == b'\n' || char == b'\r' {
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
                    .map(|off| (i, off));
            }
        } else {
            i += 1;
        }
    }
    *cursor += i;
    None
}

pub fn load_ply(file_name: impl AsRef<Path>) -> (Vec<[f32; 3]>, SurfaceIndices) {
    let file = match File::open(file_name.as_ref()) {
        Ok(f) => f,
        Err(_e) => {
            panic!()
            //return Err(LoadError::OpenFileFailed);
        }
    };
    let mut reader = BufReader::new(file);
    load_ply_buf(&mut reader)
}

pub fn load_ply_buf<B>(reader: &mut B) -> (Vec<[f32; 3]>, SurfaceIndices)
where
    B: BufRead,
{
    let mut nf = 0;
    let mut nv = 0;
    let mut vertices = Vec::new();
    const BUFFER_SIZE: usize = 65536;
    let mut buf = [0; BUFFER_SIZE];
    let mut start = 0;
    let mut header_parsed = false;
    let mut first = true;
    let mut head = HeadingInfos::default();
    let mut heading_state = HeaderParsingState::Format;

    while let Ok(size) = reader.read(&mut buf[start..]) {
        if size == 0 && start == 0 {
            break;
        }
        let end = size + start;
        let mut last = 0;

        let data = if first {
            first = false;
            let (prelude, data) = buf.split_first_chunk::<3>().unwrap();
            assert!(prelude == b"ply");
            last += 3;
            &data[..end - 3]
        } else {
            &buf[..end]
        };

        if !header_parsed {
            if let Ok(parsed) = parse_header(data, &mut last, &mut head, &mut heading_state) {
                header_parsed = parsed;
                if parsed {
                    break;
                }
            } else {
                panic!();
            }
        }

        start = end - last;
        buf.copy_within(last..end, 0);
    }

    let indices = (0..nf)
        .into_iter()
        .map(|i| [3 * i, 3 * i + 1, 3 * i + 2])
        .collect::<Vec<_>>()
        .into();
    (vertices, indices)
}

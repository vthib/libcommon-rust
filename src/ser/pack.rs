use integer_encoding::VarInt;

pub fn get_mut_slice(out: &mut Vec<u8>, size: usize) -> &mut [u8] {
    let len = out.len();
    out.resize(len + size, 0);
    &mut out[len..(len + size)]
}

pub fn push_integer<V>(tag: u16, value: V, out: &mut Vec<u8>)
where
    V: VarInt,
{
    let space = value.required_space();
    let mut zeroes_to_add = 0;

    match space {
        1 => push_tag(Wire::INT1, tag, out),
        2 => push_tag(Wire::INT2, tag, out),
        3 | 4 => {
            push_tag(Wire::INT4, tag, out);
            zeroes_to_add = 4 - space;
        }
        5 | 6 | 7 | 8 => {
            push_tag(Wire::QUAD, tag, out);
            zeroes_to_add = 8 - space;
        }
        _ => unreachable!(),
    }
    value.encode_var(get_mut_slice(out, space));
    for _ in 0..zeroes_to_add {
        out.push(0);
    }
}

pub fn push_f32(tag: u16, value: f32, out: &mut Vec<u8>) {
    push_tag(Wire::INT4, tag, out);
    out.extend_from_slice(&value.to_le_bytes());
}

pub fn push_f64(tag: u16, value: f64, out: &mut Vec<u8>) {
    push_tag(Wire::QUAD, tag, out);
    out.extend_from_slice(&value.to_le_bytes());
}

pub fn push_bytes(tag: u16, bytes: &[u8], out: &mut Vec<u8>) {
    push_len(tag, bytes.len() + 1, out);
    out.reserve(bytes.len() + 1);
    for b in bytes {
        out.push(*b);
    }
    // pack a trailing \0
    out.push(0);
}

pub fn push_repeated_len(tag: u16, len: usize, out: &mut Vec<u8>) {
    push_tag(Wire::REPEAT, tag, out);
    /* TODO: properly handle overflow */
    assert!(len < std::u32::MAX as usize);
    push_le32(len as u32, out);
}

fn push_le32(v: u32, out: &mut Vec<u8>) {
    set_le32(v, get_mut_slice(out, 4));
}

fn set_le32(v: u32, out: &mut [u8]) {
    let v = v.to_le();

    out[0] = (v & 0xFF) as u8;
    out[1] = ((v >> 8) & 0xFF) as u8;
    out[2] = ((v >> 16) & 0xFF) as u8;
    out[3] = ((v >> 24) & 0xFF) as u8;
}

pub fn push_len(tag: u16, len: usize, out: &mut Vec<u8>) {
    if len < std::u8::MAX as usize {
        push_tag(Wire::BLK1, tag, out);
        out.push(len as u8);
    } else if len <= std::u16::MAX as usize {
        let len = (len as u16).to_le();

        push_tag(Wire::BLK2, tag, out);
        out.push((len & 0xFF) as u8);
        out.push(((len >> 8) & 0xFF) as u8);
    } else {
        /* TODO: properly handle overflow */
        assert!(len < std::u32::MAX as usize);

        push_tag(Wire::BLK4, tag, out);
        push_le32(len as u32, out);
    }
}

pub fn tag_len(tag: u16) -> usize {
    if tag <= 29 {
        0
    } else if tag <= 255 {
        1
    } else {
        2
    }
}

fn push_tag(wiretype: Wire, tag: u16, out: &mut Vec<u8>) {
    set_tag(wiretype, tag, get_mut_slice(out, tag_len(tag) + 1));
}

pub fn set_len32(tag: u16, len: usize, out: &mut [u8]) {
    set_le32(len as u32, set_tag(Wire::BLK4, tag, out));
}

fn set_tag(wiretype: Wire, tag: u16, out: &mut [u8]) -> &mut [u8] {
    let wiretype = wiretype as u8;

    if tag <= 29 {
        out[0] = wiretype | tag as u8;
        &mut out[1..]
    } else if tag <= 255 {
        out[0] = wiretype | 30;
        out[1] = tag as u8;
        &mut out[2..]
    } else {
        let tag = tag.to_le();

        out[0] = wiretype | 31;
        out[1] = (tag & 0xFF) as u8;
        out[2] = (tag >> 8) as u8;
        &mut out[3..]
    }
}

#[repr(u8)]
enum Wire {
    BLK1 = 0 << 5,
    BLK2 = 1 << 5,
    BLK4 = 2 << 5,
    QUAD = 3 << 5,
    INT1 = 4 << 5,
    INT2 = 5 << 5,
    INT4 = 6 << 5,
    REPEAT = 7 << 5,
}

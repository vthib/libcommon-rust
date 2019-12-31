use integer_encoding::VarInt;

pub fn push_integer<V>(tag: u16, value: V, out: &mut Vec<u8>)
    where V: VarInt
{
    let space = value.required_space();
    let mut zeroes_to_add = 0;
 
    match space {
        1 => push_tag(Wire::INT1, tag, out),
        2 => push_tag(Wire::INT2, tag, out),
        3 | 4 => {
            push_tag(Wire::INT4, tag, out); 
            zeroes_to_add = 4 - space;
        },
        5 | 6 | 7 | 8 => {
            push_tag(Wire::QUAD, tag, out);
            zeroes_to_add = 8 - space;
        },
        _ => unreachable!(),
    }
    let len = out.len();
    out.resize(len + space, 0);
    value.encode_var(&mut out[len..]);
    for _ in 0..zeroes_to_add {
        out.push(0);
    }
}

pub fn push_f32(tag: u16, value: f32, out: &mut Vec<u8>)
{
    push_tag(Wire::INT4, tag, out); 
    out.extend_from_slice(&value.to_le_bytes());
}

pub fn push_f64(tag: u16, value: f64, out: &mut Vec<u8>)
{
    push_tag(Wire::QUAD, tag, out); 
    out.extend_from_slice(&value.to_le_bytes());
}

pub fn push_bytes(tag: u16, bytes: &[u8], out: &mut Vec<u8>)
{
    push_len(tag, bytes.len() + 1, out);
    out.reserve(bytes.len() + 1);
    for b in bytes {
        out.push(*b);
    }
    // pack a trailing \0
    out.push(0);
}

pub fn push_repeated_len(tag: u16, len: usize, out: &mut Vec<u8>)
{
    push_tag(Wire::REPEAT, tag, out);
    /* TODO: properly handle overflow */
    assert!(len < std::u32::MAX as usize);
    push_le32(len as u32, out);
}

fn push_le32(v: u32, out: &mut Vec<u8>) {
    let v = v.to_le();

    out.push((v & 0xFF) as u8);
    out.push(((v >> 8) & 0xFF) as u8);
    out.push(((v >> 16) & 0xFF) as u8);
    out.push(((v >> 24) & 0xFF) as u8);
}

fn push_len(tag: u16, len: usize, out: &mut Vec<u8>)
{
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

fn push_tag(wiretype: Wire, tag: u16, out: &mut Vec<u8>) {
    let wiretype = wiretype as u8;

    if tag <= 29 {
        out.push(wiretype | tag as u8);
    } else
    if tag <= 255 {
        out.push(wiretype | 30);
        out.push(tag as u8);
    } else {
        let tag = tag.to_le();

        out.push(wiretype | 31);
        out.push((tag & 0xFF) as u8);
        out.push((tag >> 8) as u8);
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

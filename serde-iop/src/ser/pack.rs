use crate::wire::Wire;

// FIXME: use proc ctz
fn required_space_for_i32(value: i32) -> u8 {
    // compute zigzag encoding
    let value = ((value >> 31) ^ (value << 1)) as u32;

    // make sure a bit is at least set to avoid returning 0 bytes
    let mut value = value | 1;

    let mut cnt = 0;
    while value != 0 {
        cnt += 1;
        value >>= 8;
    }

    cnt
}

pub fn get_mut_slice(out: &mut Vec<u8>, size: usize) -> &mut [u8] {
    let len = out.len();
    out.resize(len + size, 0);
    &mut out[len..(len + size)]
}

pub fn push_byte(tag: u16, value: u8, out: &mut Vec<u8>) {
    push_tag(Wire::INT1, tag, out);
    out.push(value);
}

pub fn push_i32(tag: u16, value: i32, out: &mut Vec<u8>) {
    let space = required_space_for_i32(value);

    match space {
        1 => {
            push_tag(Wire::INT1, tag, out);
            out.extend_from_slice(&(value as i8).to_le_bytes());
        }
        2 => {
            push_tag(Wire::INT2, tag, out);
            out.extend_from_slice(&(value as i16).to_le_bytes());
        }
        _ => {
            push_tag(Wire::INT4, tag, out);
            out.extend_from_slice(&value.to_le_bytes());
        }
    }
}

pub fn push_quad(tag: u16, value: u64, out: &mut Vec<u8>) {
    push_tag(Wire::QUAD, tag, out);
    out.extend_from_slice(&value.to_le_bytes());
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
    out.extend_from_slice(&v.to_le_bytes());
}

pub fn push_len(tag: u16, len: usize, out: &mut Vec<u8>) {
    if len <= std::u8::MAX as usize {
        push_tag(Wire::BLK1, tag, out);
        out.push(len as u8);
    } else if len <= std::u16::MAX as usize {
        push_tag(Wire::BLK2, tag, out);
        out.extend_from_slice(&(len as u16).to_le_bytes());
    } else {
        /* TODO: properly handle overflow */
        assert!(len <= std::u32::MAX as usize);

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
    let out = set_tag(Wire::BLK4, tag, out);
    out.copy_from_slice(&(len as u32).to_le_bytes());
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
        out[0] = wiretype | 31;
        out[1..3].copy_from_slice(&tag.to_le_bytes());
        &mut out[3..]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_push_tag() {
        fn test(w: Wire, tag: u16, expected: &[u8]) {
            let mut vec = Vec::new();

            push_tag(w, tag, &mut vec);
            assert_eq!(vec, expected);
        }

        test(Wire::BLK1, 0, &[0x00]); // BLK1 | 0
        test(Wire::BLK2, 9, &[0x29]); // BLK2 | 9
        test(Wire::BLK4, 25, &[0x59]); // BLK4 | 16
        test(Wire::QUAD, 29, &[0x7D]); // QUAD | 29
        test(Wire::INT1, 30, &[0x9E, 0x1E]); // INT1 | 30, 30
        test(Wire::INT2, 225, &[0xBE, 0xE1]); // INT2 | 30, 225
        test(Wire::INT4, 255, &[0xDE, 0xFF]); // INT4 | 30, 255
        test(Wire::REPEAT, 256, &[0xFF, 0x00, 0x01]); // REP | 31, 256 LE
        test(Wire::BLK1, std::u16::MAX, &[0x1F, 0xFF, 0xFF]); // BLK1 | 31, 65535 LE
    }

    #[test]
    fn test_push_byte() {
        fn test(tag: u16, v: u8, expected: &[u8]) {
            let mut vec = Vec::new();

            push_byte(tag, v, &mut vec);
            assert_eq!(vec, expected);
        }

        test(0, 0, &[0x80, 0x00]); // INT1 | 0, 0
        test(130, ' ' as u8, &[0x9E, 0x82, 0x20]); // INT1 | 30, 130, 32
        test(257, 0xFF, &[0x9F, 0x01, 0x01, 0xFF]); // INT1 | 31, 257, 0xFF
    }

    #[test]
    fn test_push_i32() {
        fn test(tag: u16, v: i32, expected: &[u8]) {
            let mut vec = Vec::new();

            push_i32(tag, v, &mut vec);
            assert_eq!(vec, expected);
        }

        // value in int8 range
        test(258, 0, &[0x9F, 0x02, 0x01, 0x00]); // INT1 | 31, 258, 0
        test(258, -1, &[0x9F, 0x02, 0x01, 0xFF]); // INT1 | 31, 258, -1
        test(128, 7, &[0x9E, 0x80, 0x07]); // INT1 | 30, 128, 7
        test(128, -7, &[0x9E, 0x80, 0xF9]); // INT1 | 30, 128, -7
        test(129, 127, &[0x9E, 0x81, 0x7F]); // INT1 | 30, 129, 127
        test(129, -128, &[0x9E, 0x81, 0x80]); // INT1 | 30, 128, -128

        // value in int16 range
        test(129, 128, &[0xBE, 0x81, 0x80, 0x00]); // INT2 | 30, 129, 128 LE
        test(129, -129, &[0xBE, 0x81, 0x7F, 0xFF]); // INT2 | 30, 128, -129 LE
        test(192, 255, &[0xBE, 0xC0, 0xFF, 0x00]); // INT2 | 30, 192, 255 LE
        test(192, 256, &[0xBE, 0xC0, 0x00, 0x01]); // INT2 | 30, 192, 256 LE
        test(193, 32767, &[0xBE, 0xC1, 0xFF, 0x7F]); // INT2 | 30, 193, INT16_MAX LE
        test(193, -32768, &[0xBE, 0xC1, 0x00, 0x80]); // INT2 | 30, 193, INT16_MIN LE

        // value in int32 range
        test(194, 32768, &[0xDE, 0xC2, 0x00, 0x80, 0x00, 0x00]); // INT4 | 30, 193, 32768 LE
        test(194, -32769, &[0xDE, 0xC2, 0xFF, 0x7F, 0xFF, 0xFF]); // INT4 | 30, 193, -32769 LE
        test(194, 32768, &[0xDE, 0xC2, 0x00, 0x80, 0x00, 0x00]); // INT4 | 30, 193, 32768 LE
        test(194, -32769, &[0xDE, 0xC2, 0xFF, 0x7F, 0xFF, 0xFF]); // INT4 | 30, 193, -32769 LE
        test(224, std::i32::MAX, &[0xDE, 0xE0, 0xFF, 0xFF, 0xFF, 0x7F]); // INT4 | 30, 224, I32_MAX LE
        test(224, std::i32::MIN, &[0xDE, 0xE0, 0x00, 0x00, 0x00, 0x80]); // INT4 | 30, 224, I32_MIN LE
    }

    #[test]
    fn test_push_quad() {
        fn test(tag: u16, v: u64, expected: &[u8]) {
            let mut vec = Vec::new();

            push_quad(tag, v, &mut vec);
            assert_eq!(vec, expected);
        }

        test(
            1,
            0,
            &[0x61, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
        ); // QUAD | 1, 0 LE
        test(
            128,
            (std::i32::MAX as u64) + 1,
            &[0x7E, 0x80, 0x00, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00],
        ); // QUAD | 30, 128, I32_MAX + 1 LE
        test(
            255,
            ((std::i32::MIN as i64) - 1) as u64,
            &[0x7E, 0xFF, 0xFF, 0xFF, 0xFF, 0x7F, 0xFF, 0xFF, 0xFF, 0xFF],
        ); // QUAD | 30, 255, I32_MIN - 1 LE
        test(
            256,
            std::i64::MIN as u64,
            &[
                0x7F, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80,
            ],
        ); // QUAD | 31, 256, I64_MIN LE
        test(
            256,
            std::u64::MAX / 2 + 1,
            &[
                0x7F, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80,
            ],
        ); // QUAD | 31, 256, U64_MAX / 2 LE
        test(
            256,
            std::u64::MAX,
            &[
                0x7F, 0x00, 0x01, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
            ],
        ); // QUAD | 31, 256, U64_MAX / 2 LE
    }

    #[test]
    fn test_push_len() {
        fn test(tag: u16, len: usize, expected: &[u8]) {
            let mut vec = Vec::new();

            push_len(tag, len, &mut vec);
            assert_eq!(vec, expected);
        }

        test(0, 0, &[0x00, 0x00]); // BLK1 | 0, 0
        test(5, 1, &[0x05, 0x01]); // BLK1 | 5, 1
        test(5, 255, &[0x05, 0xFF]); // BLK1 | 5, 255
        test(5, 256, &[0x25, 0x00, 0x01]); // BLK2 | 5, 256
        test(5, 65535, &[0x25, 0xFF, 0xFF]); // BLK2 | 5, 65536
        test(5, 65536, &[0x45, 0x00, 0x00, 0x01, 0x00]); // BLK4 | 5, 65537
        test(5, std::u32::MAX as usize, &[0x45, 0xFF, 0xFF, 0xFF, 0xFF]);
    }

    #[test]
    fn test_push_repeated_len() {
        fn test(tag: u16, len: usize, expected: &[u8]) {
            let mut vec = Vec::new();

            push_repeated_len(tag, len, &mut vec);
            assert_eq!(vec, expected);
        }

        test(0, 0, &[0xE0, 0x00, 0x00, 0x00, 0x00]); // REPEAT | 0, 0
        test(128, 255, &[0xFE, 0x80, 0xFF, 0x00, 0x00, 0x00]); // REPEAT | 30, 128, 255
        test(1024, 2048, &[0xFF, 0x00, 0x04, 0x00, 0x08, 0x00, 0x00]); // REPEAT | 31, 1024, 2048
    }

    #[test]
    fn test_push_bytes() {
        fn test(tag: u16, inp: &[u8], expected: &[u8]) {
            let mut vec = Vec::new();

            push_bytes(tag, inp, &mut vec);
            assert_eq!(vec, expected);
        }

        test(8, &[0xDE, 0xAD], &[0x08, 0x03, 0xDE, 0xAD, 0x00]); // BLK1 | 8, 3, payload, 0
        test(128, &[], &[0x1E, 0x80, 0x01, 0x00]); // BLK1 | 30, 1, payload, 0

        let inp = vec![0xCC; 300];
        let mut expected = Vec::new();
        expected.extend(&[0x27, 0x2D, 0x01]); // BLK2 | 7, 301
        expected.extend(&inp); // payload
        expected.extend(&[0x00]); // 0
        test(7, &inp, &expected);

        let inp = vec![0xDC; 70000];
        let mut expected = Vec::new();
        expected.extend(&[0x47, 0x71, 0x11, 0x01, 0x00]); // BLK4 | 7, 70001
        expected.extend(&inp); // payload
        expected.extend(&[0x00]); // 0
        test(7, &inp, &expected);
    }
}

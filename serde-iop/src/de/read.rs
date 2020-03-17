use crate::error::{Error, Result};
use crate::wire::Wire;
use serde::de::Visitor;
use std::mem::size_of;

#[derive(Clone, Copy)]
struct Header {
    wire: Wire,
    tag: u16,
}

pub struct BinReader<'de> {
    slice: &'de [u8],
    total_read_len: usize,
    current_hdr: Option<Header>,
}

macro_rules! read_integer_method {
    ($method:ident, $type:ty) => {
        fn $method(&mut self) -> Result<$type> {
            let mut arr: [u8; size_of::<$type>()] = Default::default();
            arr.copy_from_slice(self.get_slice(size_of::<$type>())?);

            Ok(<$type>::from_le_bytes(arr))
        }
    }
}

impl<'de> BinReader<'de> {
    pub fn new(slice: &'de [u8]) -> Self {
        Self {
            slice,
            total_read_len: 0,
            current_hdr: None,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.slice.is_empty()
    }

    pub fn get_total_read_len(&self) -> usize {
        self.total_read_len
    }

    fn read_hdr(&mut self) -> Result<Header> {
        let slice = self.get_slice(1)?;

        let wire = Wire::from(slice[0]);

        let byte = slice[0] & 0x1F;
        let tag = {
            if byte < 30 {
                byte as u16
            } else if byte == 30 {
                self.read_u8()? as u16
            } else {
                assert!(byte == 31);
                self.read_u16()?
            }
        };

        Ok(Header { wire, tag })
    }

    pub fn get_next_tag_value(&mut self) -> Result<u16> {
        if let Some(hdr) = self.current_hdr {
            Ok(hdr.tag)
        } else {
            let hdr = self.read_hdr()?;
            self.current_hdr.replace(hdr);
            Ok(hdr.tag)
        }
    }

    fn skip_upto_tag(&mut self, target_tag: u16) -> Result<Header> {
        let mut hdr = match self.current_hdr.take() {
            Some(h) => h,
            None => self.read_hdr()?,
        };
        while hdr.tag < target_tag {
            self.skip_data(hdr.wire)?;
            hdr = self.read_hdr()?;
        }
        Ok(hdr)
    }

    pub fn get_optional_tag(&mut self, target_tag: u16) -> Result<Option<Wire>> {
        let hdr = match self.skip_upto_tag(target_tag) {
            Ok(hdr) => hdr,
            Err(e) => {
                match e {
                    Error::InputTooShort => return Ok(None),
                    _ => return Err(e),
                }
            }
        };
        self.current_hdr.replace(hdr);

        if hdr.tag > target_tag {
            Ok(None)
        } else {
            Ok(Some(hdr.wire))
        }
    }

    pub fn get_tag(&mut self, target_tag: u16) -> Result<Wire> {
        let hdr = self.skip_upto_tag(target_tag)?;
        if hdr.tag > target_tag {
            Err(Error::InvalidEncoding)
        } else {
            Ok(hdr.wire)
        }
    }

    pub fn skip_data(&mut self, wire: Wire) -> Result<()> {
        match wire {
            Wire::QUAD => {
                self.get_slice(8)?;
            }
            Wire::INT1 => {
                self.get_slice(1)?;
            }
            Wire::INT2 => {
                self.get_slice(2)?;
            }
            Wire::INT4 => {
                self.get_slice(4)?;
            }
            Wire::BLK1 | Wire::BLK2 | Wire::BLK4 => {
                let len = self.read_len(wire)?;
                self.get_slice(len)?;
            }
            Wire::REPEAT => {
                let len = self.read_len(wire)?;
                for _ in 0..len {
                    let new_hdr = self.read_hdr()?;
                    if new_hdr.tag != 0 {
                        return Err(Error::InvalidEncoding);
                    }
                    self.skip_data(new_hdr.wire)?;
                }
            }
        };
        Ok(())
    }

    read_integer_method!(read_u8, u8);
    read_integer_method!(read_i8, i8);
    read_integer_method!(read_u16, u16);
    read_integer_method!(read_i16, i16);
    read_integer_method!(read_i32, i32);
    read_integer_method!(read_i64, i64);

    pub fn visit_integer<V>(&mut self, wire: Wire, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        match wire {
            Wire::INT1 => visitor.visit_i8(self.read_i8()?),
            Wire::INT2 => visitor.visit_i16(self.read_i16()?),
            Wire::INT4 => visitor.visit_i32(self.read_i32()?),
            Wire::QUAD => visitor.visit_i64(self.read_i64()?),
            _ => Err(Error::InvalidEncoding),
        }
    }

    pub fn read_u64(&mut self, wire: Wire) -> Result<u64> {
        Ok(match wire {
            Wire::INT1 => self.read_i8()? as u64,
            Wire::INT2 => self.read_i16()? as u64,
            Wire::INT4 => self.read_i32()? as u64,
            Wire::QUAD => self.read_i64()? as u64,
            _ => return Err(Error::InvalidEncoding),
        })
    }

    pub fn read_f32(&mut self, wire: Wire) -> Result<f32> {
        match wire {
            Wire::INT4 => {
                let mut arr: [u8; 4] = Default::default();
                arr.copy_from_slice(self.get_slice(4)?);

                Ok(f32::from_le_bytes(arr))
            }
            _ => Err(Error::InvalidEncoding),
        }
    }

    pub fn read_f64(&mut self, wire: Wire) -> Result<f64> {
        match wire {
            Wire::QUAD => {
                let mut arr: [u8; 8] = Default::default();
                arr.copy_from_slice(self.get_slice(8)?);

                Ok(f64::from_le_bytes(arr))
            }
            _ => Err(Error::InvalidEncoding),
        }
    }

    pub fn read_len(&mut self, wire: Wire) -> Result<usize> {
        Ok(match wire {
            Wire::BLK1 => self.read_i8()? as u8 as usize,
            Wire::BLK2 => self.read_i16()? as u16 as usize,
            Wire::BLK4 => self.read_i32()? as u32 as usize,
            Wire::QUAD => self.read_i64()? as u64 as usize,
            _ => return Err(Error::InvalidEncoding),
        })
    }

    pub fn read_repeated_len(&mut self, wire: Wire) -> Result<usize> {
        match wire {
            Wire::REPEAT => Ok(self.read_i32()? as usize),
            _ => Err(Error::InvalidEncoding),
        }
    }

    pub fn read_bytes(&mut self, wire: Wire) -> Result<&'de [u8]> {
        let len = self.read_len(wire)?;

        // a packed string ends with a trailing 0, so len should be > 0
        // and end with a 0.
        if len < 1 {
            return Err(Error::InvalidEncoding);
        }

        let slice = self.get_slice(len - 1)?;
        if self.get_slice(1)?[0] != 0 {
            return Err(Error::InvalidEncoding);
        }

        Ok(slice)
    }

    fn get_slice(&mut self, len: usize) -> Result<&'de [u8]> {
        if self.slice.len() < len {
            Err(Error::InputTooShort)
        } else {
            let slice = &self.slice[..len];

            self.slice = &self.slice[len..];
            self.total_read_len += len;

            Ok(slice)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // symmetric of test_push_tag in ser mod
    #[test]
    fn test_read_tag() {
        fn test(slice: &[u8], tag: u16, expected_res: Result<Wire>) {
            let mut reader = BinReader::new(slice);
            let res = reader.get_tag(tag);
            match &expected_res {
                Ok(w) => assert_eq!(res.unwrap(), *w),
                Err(e) => assert_eq!(res.unwrap_err(), *e),
            }

            let mut reader = BinReader::new(slice);
            let res = reader.get_optional_tag(tag).unwrap();
            match expected_res {
                Ok(w) => assert_eq!(res.unwrap(), w),
                Err(_) => assert!(res.is_none()),
            }
        }

        test(&[0x00], 0, Ok(Wire::BLK1));
        test(&[0x00], 1, Err(Error::InputTooShort));
        test(&[0x00, 0x00, 0x29], 9, Ok(Wire::BLK2));
        test(&[0x00, 0x00, 0x29], 8, Err(Error::InvalidEncoding));
        test(&[0x00, 0x00, 0x29], 10, Err(Error::InputTooShort));
        test(&[0x59], 25, Ok(Wire::BLK4));
        test(&[0x7D], 29, Ok(Wire::QUAD));
        test(&[0x9E, 0x1E], 30, Ok(Wire::INT1));
        test(&[0xBE, 0xE1], 225, Ok(Wire::INT2));
        test(&[0xDE, 0xFF], 255, Ok(Wire::INT4));
        test(&[0xFF, 0x00, 0x01], 256, Ok(Wire::REPEAT));
        test(&[0x1F, 0xFF, 0xFF], std::u16::MAX, Ok(Wire::BLK1));
    }

    // symmetric of test_push_byte in ser mod
    #[test]
    fn test_read_i8() {
        fn test(slice: &[u8], tag: u16, expected_res: Result<u8>) {
            let mut reader = BinReader::new(slice);
            assert_eq!(Wire::INT1, reader.get_tag(tag).unwrap());
            let res = reader.read_i8();
            match &expected_res {
                Ok(w) => assert_eq!(res.unwrap() as u8, *w),
                Err(e) => assert_eq!(res.unwrap_err(), *e),
            }
        }

        test(&[0x80, 0x00], 0, Ok(0));
        test(&[0x9E, 0x82, 0x20], 130, Ok(' ' as u8));
        test(&[0x9F, 0x01, 0x01, 0xFF], 257, Ok(0xFF));

        test(&[0x9F, 0x01, 0x01], 257, Err(Error::InputTooShort));
    }

    // symmetric of test_push_i32 in ser mod
    #[test]
    fn test_read_i32() {
        fn test_int1(slice: &[u8], tag: u16, expected_res: i8) {
            let mut reader = BinReader::new(slice);
            assert_eq!(Wire::INT1, reader.get_tag(tag).unwrap());
            assert_eq!(expected_res, reader.read_i8().unwrap());
        }

        fn test_int2(slice: &[u8], tag: u16, expected_res: i16) {
            let mut reader = BinReader::new(slice);
            assert_eq!(Wire::INT2, reader.get_tag(tag).unwrap());
            assert_eq!(expected_res, reader.read_i16().unwrap());
        }

        fn test_int4(slice: &[u8], tag: u16, expected_res: i32) {
            let mut reader = BinReader::new(slice);
            assert_eq!(Wire::INT4, reader.get_tag(tag).unwrap());
            assert_eq!(expected_res, reader.read_i32().unwrap());
        }

        // value in int8 range
        test_int1(&[0x9F, 0x02, 0x01, 0x00], 258, 0); // INT1 | 31, 258, 0
        test_int1(&[0x9F, 0x02, 0x01, 0xFF], 258, -1); // INT1 | 31, 258, -1
        test_int1(&[0x9E, 0x80, 0x07], 128, 7); // INT1 | 30, 128, 7
        test_int1(&[0x9E, 0x80, 0xF9], 128, -7); // INT1 | 30, 128, -7
        test_int1(&[0x9E, 0x81, 0x7F], 129, 127); // INT1 | 30, 129, 127
        test_int1(&[0x9E, 0x81, 0x80], 129, -128); // INT1 | 30, 128, -128

        // value in int16 range
        test_int2(&[0xBE, 0x81, 0x80, 0x00], 129, 128); // INT2 | 30, 129, 128 LE
        test_int2(&[0xBE, 0x81, 0x7F, 0xFF], 129, -129); // INT2 | 30, 128, -129 LE
        test_int2(&[0xBE, 0xC0, 0xFF, 0x00], 192, 255); // INT2 | 30, 192, 255 LE
        test_int2(&[0xBE, 0xC0, 0x00, 0x01], 192, 256); // INT2 | 30, 192, 256 LE
        test_int2(&[0xBE, 0xC1, 0xFF, 0x7F], 193, 32767); // INT2 | 30, 193, INT16_MAX LE
        test_int2(&[0xBE, 0xC1, 0x00, 0x80], 193, -32768); // INT2 | 30, 193, INT16_MIN LE

        // value in int32 range
        test_int4(&[0xDE, 0xC2, 0x00, 0x80, 0x00, 0x00], 194, 32768); // INT4 | 30, 193, 32768 LE
        test_int4(&[0xDE, 0xC2, 0xFF, 0x7F, 0xFF, 0xFF], 194, -32769); // INT4 | 30, 193, -32769 LE
        test_int4(&[0xDE, 0xC2, 0x00, 0x80, 0x00, 0x00], 194, 32768); // INT4 | 30, 193, 32768 LE
        test_int4(&[0xDE, 0xC2, 0xFF, 0x7F, 0xFF, 0xFF], 194, -32769); // INT4 | 30, 193, -32769 LE
        test_int4(&[0xDE, 0xE0, 0xFF, 0xFF, 0xFF, 0x7F], 224, std::i32::MAX); // INT4 | 30, 224, I32_MAX LE
        test_int4(&[0xDE, 0xE0, 0x00, 0x00, 0x00, 0x80], 224, std::i32::MIN); // INT4 | 30, 224, I32_MIN LE
    }

    // symmetric of test_push_quad in ser mod
    #[test]
    fn test_read_quad() {
        fn test(slice: &[u8], tag: u16, exp_i64: i64, exp_u64: u64) {
            let mut reader = BinReader::new(slice);
            let wire = reader.get_tag(tag).unwrap();

            assert_eq!(Wire::QUAD, wire);
            assert_eq!(exp_i64, reader.read_i64().unwrap());

            let mut reader = BinReader::new(slice);
            let wire = reader.get_tag(tag).unwrap();

            assert_eq!(exp_u64, reader.read_u64(wire).unwrap());
        }

        test(
            &[0x61, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
            1,
            0,
            0,
        ); // QUAD | 1, 0 LE
        test(
            &[0x7E, 0x80, 0x00, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00],
            128,
            (std::i32::MAX as i64) + 1,
            (std::i32::MAX as u64) + 1,
        ); // QUAD | 30, 128, I32_MAX + 1 LE
        test(
            &[0x7E, 0xFF, 0xFF, 0xFF, 0xFF, 0x7F, 0xFF, 0xFF, 0xFF, 0xFF],
            255,
            (std::i32::MIN as i64) - 1,
            ((std::i32::MIN as i64) - 1) as u64,
        ); // QUAD | 30, 255, I32_MIN - 1 LE
        test(
            &[
                0x7F, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80,
            ],
            256,
            std::i64::MIN,
            std::i64::MIN as u64,
        ); // QUAD | 31, 256, I64_MIN LE
        test(
            &[
                0x7F, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80,
            ],
            256,
            (std::u64::MAX / 2 + 1) as i64,
            std::u64::MAX / 2 + 1,
        ); // QUAD | 31, 256, U64_MAX / 2 LE
        test(
            &[
                0x7F, 0x00, 0x01, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
            ],
            256,
            std::u64::MAX as i64,
            std::u64::MAX,
        ); // QUAD | 31, 256, U64_MAX
    }

    // symmetric of test_push_len in ser mod
    #[test]
    fn test_read_len() {
        fn test(slice: &[u8], tag: u16, expected_res: Result<usize>) {
            let mut reader = BinReader::new(slice);
            let wire = reader.get_tag(tag).unwrap();
            let res = reader.read_len(wire);
            match &expected_res {
                Ok(w) => assert_eq!(res.unwrap(), *w),
                Err(e) => assert_eq!(res.unwrap_err(), *e),
            }
        }

        test(&[0x00, 0x00], 0, Ok(0)); // BLK1 | 0, 0
        test(&[0x05, 0x01], 5, Ok(1)); // BLK1 | 5, 1
        test(&[0x05, 0xFF], 5, Ok(255)); // BLK1 | 5, 255
        test(&[0x25, 0x00, 0x01], 5, Ok(256)); // BLK2 | 5, 256
        test(&[0x25, 0xFF, 0xFF], 5, Ok(65535)); // BLK2 | 5, 65536
        test(&[0x45, 0x00, 0x00, 0x01, 0x00], 5, Ok(65536)); // BLK4 | 5, 65537
        test(&[0x45, 0xFF, 0xFF, 0xFF, 0xFF], 5, Ok(std::u32::MAX as usize));
        test(&[0x7F, 0x00, 0x01, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF], 256,
             Ok(std::u64::MAX as usize)); // QUAD | 31, 256, U64_MAX 

        test(&[0xFE, 0x80, 0xFF, 0x00, 0x00, 0x00], 128,
             Err(Error::InvalidEncoding)); // REPEAT | 30, 128, 255
    }

    // symmetric of test_push_repeated_len in ser mod
    #[test]
    fn test_push_repeated_len() {
        fn test(slice: &[u8], tag: u16, expected_res: Result<usize>) {
            let mut reader = BinReader::new(slice);
            let wire = reader.get_tag(tag).unwrap();
            let res = reader.read_repeated_len(wire);
            match &expected_res {
                Ok(w) => assert_eq!(res.unwrap(), *w),
                Err(e) => assert_eq!(res.unwrap_err(), *e),
            }
        }

        test(&[0xE0, 0x00, 0x00, 0x00, 0x00], 0, Ok(0)); // REPEAT | 0, 0
        test(&[0xFE, 0x80, 0xFF, 0x00, 0x00, 0x00], 128, Ok(255)); // REPEAT | 30, 128, 255
        test(&[0xFF, 0x00, 0x04, 0x00, 0x08, 0x00, 0x00], 1024,
             Ok(2048)); // REPEAT | 31, 1024, 2048

        test(&[0x05, 0x01], 5, Err(Error::InvalidEncoding)); // BLK1 | 5, 1
    }

    // symmetric of test_push_bytes in ser mod
    #[test]
    fn test_push_bytes() {
        fn test(slice: &[u8], tag: u16, expected_res: Result<&[u8]>) {
            let mut reader = BinReader::new(slice);
            let wire = reader.get_tag(tag).unwrap();
            let res = reader.read_bytes(wire);
            match &expected_res {
                Ok(w) => assert_eq!(res.unwrap(), *w),
                Err(e) => assert_eq!(res.unwrap_err(), *e),
            }
        }

        test(&[0x08, 0x03, 0xDE, 0xAD, 0x00], 8, Ok(&[0xDE, 0xAD])); // BLK1 | 8, 3, payload, 0
        test(&[0x1E, 0x80, 0x01, 0x00], 128, Ok(&[])); // BLK1 | 30, 1, payload, 0

        let inp = vec![0xCC; 300];
        let mut expected = Vec::new();
        expected.extend(&[0x27, 0x2D, 0x01]); // BLK2 | 7, 301
        expected.extend(&inp); // payload
        expected.extend(&[0x00]); // 0
        test(&expected, 7, Ok(&inp));

        let inp = vec![0xDC; 70000];
        let mut expected = Vec::new();
        expected.extend(&[0x47, 0x71, 0x11, 0x01, 0x00]); // BLK4 | 7, 70001
        expected.extend(&inp); // payload
        expected.extend(&[0x00]); // 0
        test(&expected, 7, Ok(&inp));

        test(&[0x00, 0x00], 0, Err(Error::InvalidEncoding)); // len = 0
        test(&[0x1E, 0x80, 0x01, 0x01], 128, Err(Error::InvalidEncoding)); // not ending with 0
    }
}

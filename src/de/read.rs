use crate::error::{Error, Result};
use crate::wire::Wire;
use integer_encoding::VarInt;
use serde::de::Visitor;

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
        let hdr = self.skip_upto_tag(target_tag)?;
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

    pub fn visit_integer<V>(&mut self, wire: Wire, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        match wire {
            Wire::INT1 => visitor.visit_u8(u8::decode_var(self.get_slice(1)?).0),
            Wire::INT2 => visitor.visit_u16(u16::decode_var(self.get_slice(2)?).0),
            Wire::INT4 => visitor.visit_u32(u32::decode_var(self.get_slice(4)?).0),
            Wire::QUAD => visitor.visit_u64(u64::decode_var(self.get_slice(8)?).0),
            _ => Err(Error::InvalidEncoding),
        }
    }

    pub fn read_u64(&mut self, wire: Wire) -> Result<u64> {
        match wire {
            Wire::INT1 => Ok(u8::decode_var(self.get_slice(1)?).0 as u64),
            Wire::INT2 => Ok(u16::decode_var(self.get_slice(2)?).0 as u64),
            Wire::INT4 => Ok(u32::decode_var(self.get_slice(4)?).0 as u64),
            Wire::QUAD => Ok(u64::decode_var(self.get_slice(8)?).0),
            _ => Err(Error::InvalidEncoding),
        }
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
        match wire {
            Wire::BLK1 => Ok(u8::decode_var(self.get_slice(1)?).0 as usize),
            Wire::BLK2 => Ok(u16::decode_var(self.get_slice(2)?).0 as usize),
            Wire::BLK4 => Ok(u32::decode_var(self.get_slice(4)?).0 as usize),
            Wire::QUAD => Ok(u64::decode_var(self.get_slice(8)?).0 as usize),
            _ => return Err(Error::InvalidEncoding),
        }
    }

    pub fn read_repeated_len(&mut self, wire: Wire) -> Result<usize> {
        match wire {
            Wire::REPEAT => Ok(u8::decode_var(self.get_slice(4)?).0 as usize),
            _ => return Err(Error::InvalidEncoding),
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

    fn read_hdr(&mut self) -> Result<Header> {
        let slice = self.get_slice(1)?;

        let wire = Wire::from(slice[0]);

        let byte = slice[0] & 0x1F;
        let tag = {
            if byte < 30 {
                byte as u16
            } else if byte == 30 {
                let slice = self.get_slice(1)?;
                slice[0] as u16
            } else {
                assert!(byte == 31);
                let slice = self.get_slice(2)?;
                (slice[0] as u16) | ((slice[1] << 8) as u16)
            }
        };

        Ok(Header { wire, tag })
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

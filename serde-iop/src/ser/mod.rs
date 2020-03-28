mod pack;

use super::error::{Error, Result};
use serde::{ser, Serialize};

pub struct Serializer {
    output: Vec<u8>,
    current_tag: Option<u16>,
}

pub fn to_bytes<T>(value: &T) -> Result<Vec<u8>>
where
    T: Serialize,
{
    let mut serializer = Serializer {
        output: Vec::new(),
        current_tag: None,
    };
    value.serialize(&mut serializer)?;
    Ok(serializer.output)
}

// {{{ Serializer

impl Serializer {
    fn get_tag(&mut self) -> Result<u16> {
        self.current_tag.ok_or(Error::MissingTag)
    }
}

impl<'a> ser::Serializer for &'a mut Serializer {
    type Ok = ();
    type Error = Error;

    type SerializeSeq = Self;
    type SerializeTuple = Self;
    type SerializeTupleStruct = Self;
    type SerializeTupleVariant = Self;
    type SerializeMap = Self;
    type SerializeStruct = StructSerializer<'a>;
    type SerializeStructVariant = Self;

    fn serialize_bool(self, v: bool) -> Result<()> {
        self.serialize_i8(if v { 1 } else { 0 })
    }

    fn serialize_i8(self, v: i8) -> Result<()> {
        let tag = self.get_tag()?;

        pack::push_byte(tag, v as u8, &mut self.output);
        Ok(())
    }

    fn serialize_u8(self, v: u8) -> Result<()> {
        self.serialize_i32(v as i32)
    }

    fn serialize_i16(self, v: i16) -> Result<()> {
        self.serialize_i32(v as i32)
    }

    fn serialize_u16(self, v: u16) -> Result<()> {
        self.serialize_i32(v as i32)
    }

    fn serialize_i32(self, v: i32) -> Result<()> {
        let tag = self.get_tag()?;

        if v == 0 {
            pack::push_byte(tag, 0, &mut self.output);
        } else {
            pack::push_i32(tag, v, &mut self.output);
        }
        Ok(())
    }

    fn serialize_u32(self, v: u32) -> Result<()> {
        self.serialize_i64(v as i64)
    }

    fn serialize_i64(self, v: i64) -> Result<()> {
        let tag = self.get_tag()?;

        if v <= (std::i32::MAX as i64) {
            pack::push_i32(tag, v as i32, &mut self.output);
        } else {
            pack::push_quad(tag, v as u64, &mut self.output);
        }
        Ok(())
    }

    fn serialize_u64(self, v: u64) -> Result<()> {
        self.serialize_i64(v as i64)
    }

    fn serialize_f32(self, v: f32) -> Result<()> {
        let tag = self.get_tag()?;

        pack::push_f32(tag, v, &mut self.output);
        Ok(())
    }

    fn serialize_f64(self, v: f64) -> Result<()> {
        let tag = self.get_tag()?;

        pack::push_f64(tag, v, &mut self.output);
        Ok(())
    }

    fn serialize_char(self, v: char) -> Result<()> {
        self.serialize_u32(v as u32)
    }

    fn serialize_str(self, v: &str) -> Result<()> {
        let tag = self.get_tag()?;

        pack::push_bytes(tag, v.as_bytes(), &mut self.output);
        Ok(())
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<()> {
        let tag = self.get_tag()?;

        pack::push_bytes(tag, v, &mut self.output);
        Ok(())
    }

    fn serialize_none(self) -> Result<()> {
        Ok(())
    }

    fn serialize_some<T>(self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<()> {
        // FIXME: must fill output if boxed
        Ok(())
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<()> {
        Err(Error::Unimplemented("unit struct"))
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
    ) -> Result<()> {
        Err(Error::Unimplemented("unit variant"))
    }

    fn serialize_newtype_struct<T>(self, _name: &'static str, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        variant_index: u32,
        _variant: &'static str,
        value: &T,
    ) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        let tag = self.get_tag()?;

        /* reserve space for len */
        let pos = self.output.len();
        let slice_len = pack::tag_len(tag) + 1 + 4;
        pack::get_mut_slice(&mut self.output, slice_len);

        /* use tag for variant index, and pack value. */
        self.current_tag = Some(variant_index as u16);
        value.serialize(&mut *self)?;
        self.current_tag = Some(tag);

        /* then write length */
        let len = self.output.len() - pos - slice_len;
        let slice = &mut self.output[pos..(pos + slice_len)];
        pack::set_len32(tag, len, slice);
        Ok(())
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq> {
        let tag = self.get_tag()?;

        let len = len.ok_or(Error::UnknownLen)?;
        pack::push_repeated_len(tag, len, &mut self.output);
        Ok(self)
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple> {
        Err(Error::Unimplemented("tuple"))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct> {
        Err(Error::Unimplemented("tuple struct"))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        Err(Error::Unimplemented("tuple variant"))
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap> {
        Err(Error::Unimplemented("map"))
    }

    fn serialize_struct(self, _name: &'static str, _len: usize) -> Result<Self::SerializeStruct> {
        match self.get_tag() {
            Ok(tag) => {
                let pos = self.output.len();
                pack::get_mut_slice(&mut self.output, pack::tag_len(tag) + 1 + 4);

                Ok(StructSerializer {
                    ser: self,
                    tag: 1,
                    struct_pos: pos,
                    struct_tag: tag,
                })
            }
            Err(_) => Ok(StructSerializer {
                ser: self,
                tag: 1,
                struct_pos: 0,
                struct_tag: 0,
            }),
        }
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant> {
        Err(Error::Unimplemented("struct variant"))
    }
}

// }}}
// {{{ Seq

impl<'a> ser::SerializeSeq for &'a mut Serializer {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        self.current_tag.replace(0);
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<()> {
        Ok(())
    }
}

// }}}
// {{{ Tuple

impl<'a> ser::SerializeTuple for &'a mut Serializer {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T>(&mut self, _value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        Err(Error::Unimplemented("tuple element"))
    }

    fn end(self) -> Result<()> {
        Err(Error::Unimplemented("tuple end"))
    }
}

// }}}
// {{{ Tuple Struct

impl<'a> ser::SerializeTupleStruct for &'a mut Serializer {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, _value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        Err(Error::Unimplemented("tuple struct field"))
    }

    fn end(self) -> Result<()> {
        Err(Error::Unimplemented("tuple struct end"))
    }
}

// }}}
// {{{ Tuple Variant

impl<'a> ser::SerializeTupleVariant for &'a mut Serializer {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, _value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        Err(Error::Unimplemented("tuple variant field"))
    }

    fn end(self) -> Result<()> {
        Err(Error::Unimplemented("tuple variant end"))
    }
}

// }}}
// {{{ Map

impl<'a> ser::SerializeMap for &'a mut Serializer {
    type Ok = ();
    type Error = Error;

    fn serialize_key<T>(&mut self, _key: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        Err(Error::Unimplemented("map key"))
    }

    fn serialize_value<T>(&mut self, _value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        Err(Error::Unimplemented("map value"))
    }

    fn end(self) -> Result<()> {
        Err(Error::Unimplemented("map end"))
    }
}

// }}}
// {{{ Struct

pub struct StructSerializer<'a> {
    ser: &'a mut Serializer,
    tag: u16,
    struct_pos: usize,
    struct_tag: u16,
}

impl<'a> ser::SerializeStruct for StructSerializer<'a> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, _key: &'static str, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        self.ser.current_tag.replace(self.tag);
        self.tag += 1;
        value.serialize(&mut *self.ser)
    }

    fn end(self) -> Result<()> {
        if self.struct_pos != 0 {
            let slice_len = pack::tag_len(self.struct_tag) + 1 + 4;
            let struct_len = self.ser.output.len() - self.struct_pos - slice_len;
            let slice = &mut self.ser.output[self.struct_pos..(self.struct_pos + slice_len)];

            pack::set_len32(self.struct_tag, struct_len, slice);
        }
        Ok(())
    }
}

// }}}
// {{{ Struct Variant

impl<'a> ser::SerializeStructVariant for &'a mut Serializer {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, _key: &'static str, _value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        Err(Error::Unimplemented("struct variant field"))
    }

    fn end(self) -> Result<()> {
        Err(Error::Unimplemented("struct variant end"))
    }
}

// }}}

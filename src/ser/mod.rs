mod error;
mod pack;

use error::{Error, Result};
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

    fn serialize_integer<T>(&mut self, v: T) -> Result<()>
    where
        T: integer_encoding::VarInt,
    {
        let tag = self.get_tag()?;

        pack::push_integer(tag, v, &mut self.output);
        Ok(())
    }
}

macro_rules! serialize_integer {
    ($t:ty, $name:ident) => {
        fn $name(self, v: $t) -> Result<()> {
            self.serialize_integer(v)
        }
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
        self.serialize_integer(if v { 1 } else { 0 } as u8)
    }

    serialize_integer!(i8, serialize_i8);
    serialize_integer!(i16, serialize_i16);
    serialize_integer!(i32, serialize_i32);
    serialize_integer!(i64, serialize_i64);
    serialize_integer!(u8, serialize_u8);
    serialize_integer!(u16, serialize_u16);
    serialize_integer!(u32, serialize_u32);
    serialize_integer!(u64, serialize_u64);

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
        self.serialize_integer(v as u32)
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
        Err(Error::Unimplemented("none"))
    }

    fn serialize_some<T>(self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<()> {
        Err(Error::Unimplemented("unit"))
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
        _variant_index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        Err(Error::Unimplemented("newtype variant"))
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
        Ok(StructSerializer { ser: self, tag: 1 })
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
// {{{ Tests

#[test]
fn test_struct() {
    use serde::Serialize;

    #[derive(Serialize)]
    struct Test {
        int: u32,
        seq: Vec<&'static str>,
    }

    let test = Test {
        int: 1,
        seq: vec!["a", "b"],
    };
    let expected = [
        0x81, // INT1 | 1
        0x01, // value: 1
        0xE2, // REPEAT | 2
        0x02, 0x00, 0x00, 0x00, // len = 2
        0x00, // BLK1 | 0
        0x02, // len = 2
        b'a', b'\0', // "a"
        0x00,  // BLK1 | 0
        0x02,  // len = 2
        b'b', b'\0', // "b"
    ];
    assert_eq!(to_bytes(&test).unwrap(), expected);
}

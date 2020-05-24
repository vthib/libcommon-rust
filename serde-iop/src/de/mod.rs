use serde::de::{
    self, DeserializeSeed, EnumAccess, IntoDeserializer, SeqAccess, VariantAccess, Visitor,
};
use serde::{forward_to_deserialize_any, Deserialize};

mod read;
use read::BinReader;

use crate::error::{Error, Result};
use crate::wire::Wire;

/* {{{ Deserializer */

pub struct Deserializer<'de> {
    reader: BinReader<'de>,
    current_tag: Option<u16>,
}

impl<'de> Deserializer<'de> {
    pub fn from_bytes(input: &'de [u8]) -> Self {
        Self {
            reader: BinReader::new(input),
            current_tag: None,
        }
    }
}

pub fn from_bytes<'a, T>(input: &'a [u8]) -> Result<T>
where
    T: Deserialize<'a>,
{
    let mut deserializer = Deserializer::from_bytes(input);
    let t = T::deserialize(&mut deserializer)?;
    if deserializer.reader.is_empty() {
        Ok(t)
    } else {
        Err(Error::TrailingCharacters)
    }
}

impl<'de> Deserializer<'de> {
    pub fn get_wire(&mut self) -> Result<Wire> {
        let tag = self.current_tag.ok_or(Error::MissingTag)?;
        self.reader.get_tag(tag)
    }

    pub fn get_optional_wire(&mut self) -> Result<Option<Wire>> {
        let tag = self.current_tag.ok_or(Error::MissingTag)?;
        self.reader.get_optional_tag(tag)
    }
}

macro_rules! deserialize_int_method {
    ($method:ident) => {
        fn $method<V>(self, visitor: V) -> Result<V::Value>
        where
            V: Visitor<'de>,
        {
            let wire = self.get_wire()?;
            self.reader.visit_integer(wire, visitor)
        }
    };
}

impl<'de, 'a> de::Deserializer<'de> for &'a mut Deserializer<'de> {
    type Error = Error;

    fn deserialize_any<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        Err(Error::Unimplemented("any"))
    }

    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let wire = self.get_wire()?;

        let v = self.reader.read_u64(wire)?;
        visitor.visit_bool(if v == 0 { false } else { true })
    }

    deserialize_int_method!(deserialize_i8);
    deserialize_int_method!(deserialize_i16);
    deserialize_int_method!(deserialize_i32);
    deserialize_int_method!(deserialize_i64);
    deserialize_int_method!(deserialize_u8);
    deserialize_int_method!(deserialize_u16);
    deserialize_int_method!(deserialize_u32);
    deserialize_int_method!(deserialize_u64);

    fn deserialize_f32<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let wire = self.get_wire()?;

        visitor.visit_f32(self.reader.read_f32(wire)?)
    }

    fn deserialize_f64<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let wire = self.get_wire()?;

        visitor.visit_f64(self.reader.read_f64(wire)?)
    }

    fn deserialize_char<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let wire = self.get_wire()?;

        let v = self.reader.read_u64(wire)?;
        let v = if v < std::u32::MAX as u64 {
            std::char::from_u32(v as u32)
        } else {
            None
        };

        match v {
            Some(c) => visitor.visit_char(c),
            None => Err(Error::InvalidEncoding),
        }
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_bytes(visitor)
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_bytes(visitor)
    }

    fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let wire = self.get_wire()?;

        visitor.visit_borrowed_bytes(self.reader.read_bytes(wire)?)
    }

    fn deserialize_byte_buf<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_bytes(visitor)
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let wire = self.get_optional_wire()?;

        match wire {
            Some(_w) => visitor.visit_some(self),
            None => visitor.visit_none(),
        }
    }

    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_unit()
    }

    fn deserialize_unit_struct<V>(self, _name: &'static str, _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        Err(Error::Unimplemented("unit struct"))
    }

    fn deserialize_newtype_struct<V>(self, _name: &'static str, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_seq<V>(mut self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let wire = self.get_wire()?;

        let len = self.reader.read_repeated_len(wire)?;
        visitor.visit_seq(SeqDeserializer::new(&mut self, len))
    }

    fn deserialize_tuple<V>(self, _len: usize, _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        Err(Error::Unimplemented("tuple"))
    }

    fn deserialize_tuple_struct<V>(
        self,
        _name: &'static str,
        _len: usize,
        _visitor: V,
    ) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        Err(Error::Unimplemented("tuple struct"))
    }

    fn deserialize_map<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        Err(Error::Unimplemented("tuple struct"))
    }

    fn deserialize_struct<V>(
        mut self,
        _name: &'static str,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        match self.current_tag {
            Some(_) => {
                let wire = self.get_wire()?;

                let len = self.reader.read_len(wire)?;
                visitor.visit_seq(StructDeserializer::new(&mut self, fields.len(), Some(len)))
            }
            None => visitor.visit_seq(StructDeserializer::new(&mut self, fields.len(), None)),
        }
    }

    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        // This is actually for variants, ie unions
        let mut deserializer = match self.current_tag {
            Some(_) => {
                let wire = self.get_wire()?;

                let len = self.reader.read_len(wire)?;
                UnionDeserializer::new(self, Some(len))
            }
            None => UnionDeserializer::new(self, None),
        };
        visitor.visit_enum(&mut deserializer)
    }

    fn deserialize_identifier<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        Err(Error::Unimplemented("identifier"))
    }

    fn deserialize_ignored_any<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        Err(Error::Unimplemented("ignored any"))
    }
}

/* }}} */
/* {{{ Seq */

struct SeqDeserializer<'a, 'de: 'a> {
    de: &'a mut Deserializer<'de>,
    remaining_elements: usize,
}

impl<'a, 'de> SeqDeserializer<'a, 'de> {
    fn new(de: &'a mut Deserializer<'de>, elements: usize) -> Self {
        SeqDeserializer {
            de,
            remaining_elements: elements,
        }
    }
}

impl<'de, 'a> SeqAccess<'de> for SeqDeserializer<'a, 'de> {
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>>
    where
        T: DeserializeSeed<'de>,
    {
        if self.remaining_elements == 0 {
            return Ok(None);
        }
        self.remaining_elements -= 1;
        self.de.current_tag.replace(0);
        seed.deserialize(&mut *self.de).map(Some)
    }
}

/* }}} */
/* {{{ Struct */

struct StructDeserializer<'a, 'de: 'a> {
    de: &'a mut Deserializer<'de>,
    nb_fields: usize,
    struct_len: Option<usize>,
    current_tag: u16,
}

impl<'a, 'de> StructDeserializer<'a, 'de> {
    fn new(de: &'a mut Deserializer<'de>, nb_fields: usize, struct_len: Option<usize>) -> Self {
        let current_read_len = de.reader.get_total_read_len();

        StructDeserializer {
            de,
            nb_fields,
            struct_len: struct_len.map(|v| v + current_read_len),
            current_tag: 1,
        }
    }
}

impl<'de, 'a> SeqAccess<'de> for StructDeserializer<'a, 'de> {
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>>
    where
        T: DeserializeSeed<'de>,
    {
        let stop = match self.struct_len {
            Some(max_len) => self.de.reader.get_total_read_len() >= max_len,
            None => self.de.reader.is_empty(),
        };
        if stop && self.nb_fields == 0 {
            return Ok(None);
        }
        self.de.current_tag.replace(self.current_tag);
        self.current_tag += 1;
        self.nb_fields -= 1;
        seed.deserialize(&mut *self.de).map(Some)
    }
}

/* }}} */
/* {{{ Union */

struct UnionDeserializer<'a, 'de: 'a> {
    de: &'a mut Deserializer<'de>,
    _union_len: Option<usize>,
}

impl<'a, 'de> UnionDeserializer<'a, 'de> {
    fn new(de: &'a mut Deserializer<'de>, union_len: Option<usize>) -> Self {
        let current_read_len = de.reader.get_total_read_len();

        UnionDeserializer {
            de,
            _union_len: union_len.map(|v| v + current_read_len),
        }
    }
}

impl<'de, 'a> de::Deserializer<'de> for &'a mut UnionDeserializer<'a, 'de> {
    type Error = Error;

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map struct enum identifier ignored_any
    }

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let tag = self.de.reader.get_next_tag_value()?;
        // TODO: map tag to index
        self.de.current_tag.replace(tag);
        visitor.visit_u16(tag)
    }
}

impl<'de, 'a> EnumAccess<'de> for &'a mut UnionDeserializer<'a, 'de> {
    type Error = Error;
    type Variant = Self;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant)>
    where
        V: DeserializeSeed<'de>,
    {
        let tag = self.de.reader.get_next_tag_value()?;
        // TODO: map tag to index
        self.de.current_tag.replace(tag);
        let v = seed.deserialize(tag.into_deserializer())?;
        Ok((v, self))
    }
}

impl<'de, 'a> VariantAccess<'de> for &'a mut UnionDeserializer<'a, 'de> {
    type Error = Error;

    fn unit_variant(self) -> Result<()> {
        Err(Error::Unimplemented("unit variant"))
    }

    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value>
    where
        T: DeserializeSeed<'de>,
    {
        seed.deserialize(&mut *self.de)
    }

    fn tuple_variant<V>(self, _len: usize, _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        Err(Error::Unimplemented("tuple variant"))
    }

    fn struct_variant<V>(self, _fields: &'static [&'static str], _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        Err(Error::Unimplemented("struct variant"))
    }
}

/* }}} */

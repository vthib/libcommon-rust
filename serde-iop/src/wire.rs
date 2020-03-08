#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Wire {
    // len in 1 byte followed by payload
    BLK1 = 0 << 5,
    // len in 2 bytes (LE) followed by payload
    BLK2 = 1 << 5,
    // len in 4 bytes (LE) followed by payload
    BLK4 = 2 << 5,
    // integer in 8 bytes (LE)
    QUAD = 3 << 5,
    // integer in 1 byte
    INT1 = 4 << 5,
    // integer in 2 bytes (LE)
    INT2 = 5 << 5,
    // integer in 4 bytes (LE)
    INT4 = 6 << 5,
    // len in 4 bytes, followed by len packets marked with tag 0
    REPEAT = 7 << 5,
}

impl From<u8> for Wire {
    fn from(v: u8) -> Self {
        match v >> 5 {
            0 => Wire::BLK1,
            1 => Wire::BLK2,
            2 => Wire::BLK4,
            3 => Wire::QUAD,
            4 => Wire::INT1,
            5 => Wire::INT2,
            6 => Wire::INT4,
            7 => Wire::REPEAT,
            _ => unreachable!(),
        }
    }
}

// IOP Format:
//
// every packet is packed a wire, tag, then payload
//
// The 3 higher bits of the first byte are the wire. Then:
// * if the 5 lower bits are < 30, the value is the tag
// * if == 30, the tag is in the next byte
// * if == 31, the tag is in the next 2 bytes (LE)
//
// pack len ::= BLK1, BLK2 or BLK4 of len, packed as little-endian
//
// i8:
//   INT1, packed as a byte
// u8, i16, u16, i32:
//   compute zigzag-encoding as an i32, find how many bytes are needed
//   for 1 byte, INT1, packed as a byte
//   for 2 bytes, INT2, packed as i16 in little-endian
//   for 3 or 4 bytes, INT4, packed as i32 in little-endian
// u32, i64, u64:
//   if <= INT32_MAX, see above
//   else, QUAD of little-endian repr
// bool:
//   INT1, packed as a byte
// void:
//   if union or optional, BLK1 of len 0
//   else, do not pack
// double:
//   QUAD of little-endian repr
// string/bytes:
//   pack len + 1
//   then payload
//   then byte 0
// union:
//   pack len that the packing of the next value takes in the output
//   then pack value
// array:
//   i8, u8, i16, u16, bool:
//     pack len = array size, ie array_len * sizeof(type)
//     then pack array as is in payload
//   normal case:
//     REPEAT, then len of array packed in i32 little-endian
//     for every value, pack it using tag 0
// struct:
//   pack len that the packing of the struct takes in the output
//   then pack fields, in increasing tag order
// class:
//   pack len that the packing of the class takes in the output
//   pack class_id as an int32 in tag 0
//   if len of packing of current class non empty:
//     pack fields of class, in increasing tag order
//   then for every parent:
//     if len of packing of parent is non empty:
//       pack class_id
//       pack fields of parent, in increasing tag order
//

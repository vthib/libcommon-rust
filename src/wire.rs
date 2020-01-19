#[repr(u8)]
#[derive(Clone, Copy)]
pub enum Wire {
    BLK1 = 0 << 5,
    BLK2 = 1 << 5,
    BLK4 = 2 << 5,
    QUAD = 3 << 5,
    INT1 = 4 << 5,
    INT2 = 5 << 5,
    INT4 = 6 << 5,
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

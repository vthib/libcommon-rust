use serde::{Deserialize, Serialize};
use serde_iop::{from_bytes, to_bytes};
use serde_repr::{Deserialize_repr, Serialize_repr};

#[test]
fn test_basic() {
    #[derive(Serialize, Deserialize, PartialEq, Debug, Default)]
    struct Inner {
        v1: Option<bool>,
        _dummy2: (),
        _dummy3: (),
        v2: Option<bool>,
        c: char,
    }
    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct Test {
        int: u32,
        seq: Vec<String>,
        _dummy3: (),
        inner: Inner,
    }
    impl Default for Test {
        fn default() -> Self {
            Test {
                int: 3,
                seq: Default::default(),
                _dummy3: Default::default(),
                inner: Default::default(),
            }
        }
    }

    let test = Test {
        int: 1,
        seq: vec!["a".to_owned(), "b".to_owned()],
        inner: Inner {
            v1: Some(true),
            c: '\n',
            ..Default::default()
        },
        ..Default::default()
    };
    let expected_bytes = [
        // int:
        0x81, // INT1 | 1
        0x01, // value: 1
        // seq:
        0xE2, // REPEAT | 2
        0x02, 0x00, 0x00, 0x00, // len = 2
        // "a"
        0x00, // BLK1 | 0
        0x02, // len = 2
        b'a', b'\0', // "a"
        // "b"
        0x00, // BLK1 | 0
        0x02, // len = 2
        b'b', b'\0', // "b"
        // inner:
        0x44, // BLK4 | 4
        0x04, 0x00, 0x00, 0x00, // len: 4
        // v1:
        0x81, // INT1 | 1
        0x01, // value: 1
        // v2 is skipped
        // c:
        0x85, // INT1 | 5
        0x0A, // '\n'
    ];
    assert_eq!(to_bytes(&test).unwrap(), expected_bytes);
    assert_eq!(test, from_bytes(&expected_bytes).unwrap());
}

#[test]
fn test_all_types() {
    #[derive(Serialize_repr, Deserialize_repr, PartialEq, Debug)]
    #[repr(i32)]
    enum EnumA {
        A = 0x0,
        B = 0x1,
        C = 0x2,
        D = 0x10,
    };

    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    enum VariantA {
        Ua(i32),
        Ub(i8),
        Us(String),
    };

    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct StructA {
        a: i32,
        b: u32,
        c: i8,
        d: u8,
        e: i16,
        f: u16,
        g: i64,
        h: u64,
        htab: Vec<u64>,
        i: Vec<u8>,
        j: String,
        k: EnumA,
        l: VariantA,
        lr: Box<VariantA>,
        //cls2: Class2,
        m: f64,
        n: bool,
        u: (),
    }

    let test = StructA {
        a: 42,
        b: 5,
        c: 120,
        d: 230,
        e: 540,
        f: 2000,
        g: 10000,
        h: 20000,
        htab: Vec::new(),
        i: "foo".to_owned().into_bytes(),
        j: "baré© \" foo .".to_owned(),
        k: EnumA::B,
        l: VariantA::Ub(42),
        lr: Box::new(VariantA::Ua(1)),
        //cls2: &cls2,
        m: 3.14159265,
        n: true,
        u: (),
    };
    let bytes = to_bytes(&test).unwrap();
    let unpacked = from_bytes(&bytes).unwrap();
    assert_eq!(test, unpacked);
}

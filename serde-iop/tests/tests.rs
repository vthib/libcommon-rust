use serde::{Deserialize, Serialize};
use serde_iop::{from_bytes, to_bytes};

#[test]
fn test_struct() {
    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct Inner {
        v1: Option<bool>,
        v2: Option<bool>,
        c: char,
    }
    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct Test {
        int: u32,
        seq: Vec<String>,
        inner: Inner,
    }

    let test = Test {
        int: 1,
        seq: vec!["a".to_owned(), "b".to_owned()],
        inner: Inner {
            v1: Some(true),
            v2: None,
            c: '\n',
        },
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
        0x43, // BLK4 | 3
        0x04, 0x00, 0x00, 0x00, // len: 4
        // v1:
        0x81, // INT1 | 1
        0x01, // value: 1
        // v2 is skipped
        // c:
        0x83, // INT1 | 3
        0x0A, // '\n'
    ];
    assert_eq!(to_bytes(&test).unwrap(), expected_bytes);
    assert_eq!(test, from_bytes(&expected_bytes).unwrap());
}

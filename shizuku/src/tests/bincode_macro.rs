use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};
use shizuku_macros::{BincodeByteDes, BincodeByteSer};
use crate as shizuku;

#[derive(Clone, Debug, Encode, Decode, BincodeByteSer, BincodeByteDes)]
struct ExampleData {
    pub number: i32,
    pub st: String
}
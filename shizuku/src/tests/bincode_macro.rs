use serde::{Deserialize, Serialize};
use shizuku_macros::{BincodeByteDes, BincodeByteSer};

#[derive(Clone, Debug, Serialize, Deserialize, BincodeByteSer, BincodeByteDes)]
struct ExampleData {
    pub number: i32,
    pub st: String
}
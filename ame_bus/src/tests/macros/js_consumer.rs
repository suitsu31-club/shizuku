#![allow(unused_imports)]

use crate as ame_bus;
use ame_bus_macros::{jet, jet_consumer};

#[jet(name = "test", description = "test stream")]
#[jet_consumer(name = "test", durable)]
#[allow(dead_code)]
struct MyStruct {
    field: String,
}

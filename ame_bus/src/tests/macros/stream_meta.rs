use crate as ame_bus;
use ame_bus_macros::jet;

#[jet(name = "test", description = "test stream")]
#[allow(dead_code)]
struct MyStruct {
    field: String,
}

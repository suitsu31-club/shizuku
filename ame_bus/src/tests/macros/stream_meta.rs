use crate as ame_bus;
use ame_bus_macros::jetstream;

#[jetstream(name = "test", description = "test stream")]
#[allow(dead_code)]
struct MyStruct {
    field: String,
}

use ame_bus_macros::jetstream;
use crate as ame_bus;

#[jetstream(name = "test", description = "test stream")]
struct MyStruct {
    field: String,
}
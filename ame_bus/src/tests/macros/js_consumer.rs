use ame_bus_macros::{jetstream, jetstream_consumer};
use crate as ame_bus;

#[jetstream(name = "test", description = "test stream")]
#[jetstream_consumer(name = "test", durable)]
#[allow(dead_code)]
struct MyStruct {
    field: String,
}

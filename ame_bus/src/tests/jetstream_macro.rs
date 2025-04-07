use std::sync::Arc;
use crate::jetstream::FinalJetStreamProcessor;
use crate::{jet_route, Error, FinalProcessor, Processor};
use crate as ame_bus;
use async_nats::Message;
use tokio::sync::Mutex;

macro_rules! create_mock_processor {
    ($name:ident, $record:literal) => {
        struct $name {
            pub record: Arc<Mutex<Vec<i32>>>,
        }

        impl Processor<Message, Result<(), Error>> for $name {
            async fn process(&self, input: Message) -> Result<(), Error> {
                self.record.lock().await.push($record);
                Ok(())
            }
        }
    }
}

create_mock_processor!(InvoiceCreateProcessor, 1);
create_mock_processor!(InvoiceUpdateProcessor, 2);
create_mock_processor!(InvoiceDeleteProcessor, 3);
create_mock_processor!(UserRegisterProcessor, 4);
create_mock_processor!(UserBannedProcessor, 5);

struct FinalMockedProcessor {
    invoice_create_processor: InvoiceCreateProcessor,
    invoice_update_processor: InvoiceUpdateProcessor,
    invoice_delete_processor: InvoiceDeleteProcessor,
    user_register_processor: UserRegisterProcessor,
    user_banned_processor: UserBannedProcessor,
    pub record: Arc<Mutex<Vec<i32>>>,
}

impl FinalMockedProcessor {
    pub fn new(record: Arc<Mutex<Vec<i32>>>) -> Self {
        Self {
            invoice_create_processor: InvoiceCreateProcessor { record: record.clone() },
            invoice_update_processor: InvoiceUpdateProcessor { record: record.clone() },
            invoice_delete_processor: InvoiceDeleteProcessor { record: record.clone() },
            user_register_processor: UserRegisterProcessor { record: record.clone() },
            user_banned_processor: UserBannedProcessor { record: record.clone() },
            record,
        }
    }
}

pub(super) fn generate_empty_message(name: &str) -> Message {
    Message {
        subject: name.into(),
        reply: None,
        payload: Default::default(),
        headers: None,
        status: None,
        description: None,
        length: 0,
    }
}

impl Processor<Message, Result<(), Error>> for FinalMockedProcessor {
    async fn process(&self, input: Message) -> Result<(), Error> {
        jet_route![
            input,
        ]
    }
}
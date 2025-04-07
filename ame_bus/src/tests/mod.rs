use crate::jetstream::FinalJetStreamProcessor;
use crate::{jet_route, Error, FinalProcessor, Processor};
use crate as ame_bus;
use async_nats::Message;

struct OrderPaidProcessor; // example
impl Processor<Message, Result<(), Error>> for OrderPaidProcessor {
    async fn process(&self, input: Message) -> Result<(), Error> {
        // Handle paid order message
        unimplemented!()
    }
}

struct OrderCancelledProcessor; // example
impl Processor<Message, Result<(), Error>> for OrderCancelledProcessor {
    async fn process(&self, input: Message) -> Result<(), Error> {
        // Handle cancelled order message
        unimplemented!()
    }
}

struct InvoiceProcessor; // example
impl Processor<Message, Result<(), Error>> for InvoiceProcessor {
    async fn process(&self, input: Message) -> Result<(), Error> {
        // Handle invoice message
        unimplemented!()
    }
}

struct OrderService {
    order_paid_processor: OrderPaidProcessor,
    order_cancelled_processor: OrderCancelledProcessor,
    invoice_processor: InvoiceProcessor,
}

impl Processor<Message, Result<(), Error>> for OrderService {
    async fn process(&self, input: Message) -> Result<(), Error> {
        jet_route![
            input,
            ["invoice", *, "pro"] => (&self.invoice_processor),
            ["order", "paid"] => (&self.order_paid_processor),
            ["order", "cancelled"] => (&self.order_cancelled_processor)
        ]
    }
}

impl FinalJetStreamProcessor for OrderService {}

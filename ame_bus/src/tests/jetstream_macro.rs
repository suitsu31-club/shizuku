use std::sync::Arc;
use crate::jetstream::FinalJetStreamProcessor;
use crate::{jet_route, Error, Processor};
use async_nats::Message;
use tokio::sync::Mutex;

macro_rules! create_mock_processor {
    ($name:ident, $record:literal) => {
        struct $name {
            pub record: Arc<Mutex<Vec<i32>>>,
        }

        impl Processor<Message, Result<(), Error>> for $name {
            async fn process(&self, _: Message) -> Result<(), Error> {
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
            // Test basic static routes
            ["invoice", "create"] => (&self.invoice_create_processor),
            ["invoice", "update"] => (&self.invoice_update_processor),
            ["invoice", "delete"] => (&self.invoice_delete_processor),

            // Test single wildcard in different positions
            ["user", "*", "register"] => (&self.user_register_processor),
            ["*", "user", "banned"] => (&self.user_banned_processor),
            
            // Test multiple wildcards
            ["invoice", "*", "*"] => (&self.invoice_create_processor),
            
            // Test recursive wildcard
            ["user", "banned", ">"] => (&self.user_banned_processor),
            
            // Test nested routes with static paths
            ["invoice"] => [
                ["create"] => (&self.invoice_create_processor),
                ["update"] => (&self.invoice_update_processor)
            ],
            
            // Test nested routes with wildcards
            ["user"] => [
                ["*", "register"] => (&self.user_register_processor),
                ["banned", "*"] => (&self.user_banned_processor)
            ],
            
            // Test deeply nested routes
            ["deep"] => [
                ["nested"] => [
                    ["route"] => (&self.invoice_create_processor)
                ]
            ],
            
            // Test wildcard with nested routes
            ["*"] => [
                ["create"] => (&self.invoice_create_processor),
                ["update"] => (&self.invoice_update_processor)
            ]
        ]
    }
}

impl FinalJetStreamProcessor for FinalMockedProcessor {}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_route(processor: &FinalMockedProcessor, subject: &str, expected_value: i32) {
        let msg = generate_empty_message(subject);
        processor.process(msg).await.unwrap();
        let record = processor.record.lock().await;
        assert_eq!(record.last(), Some(&expected_value));
    }

    #[tokio::test]
    async fn test_static_routes() {
        let record = Arc::new(Mutex::new(Vec::new()));
        let processor = FinalMockedProcessor::new(record);

        test_route(&processor, "invoice.create", 1).await;
        test_route(&processor, "invoice.update", 2).await;
        test_route(&processor, "invoice.delete", 3).await;
    }

    #[tokio::test]
    async fn test_single_wildcard_routes() {
        let record = Arc::new(Mutex::new(Vec::new()));
        let processor = FinalMockedProcessor::new(record);

        // Wildcard in middle
        test_route(&processor, "user.abc.register", 4).await;
        // Wildcard at start
        test_route(&processor, "user.banned.something", 5).await;
    }

    #[tokio::test]
    async fn test_multiple_wildcard_routes() {
        let record = Arc::new(Mutex::new(Vec::new()));
        let processor = FinalMockedProcessor::new(record);

        test_route(&processor, "invoice.foo.bar", 1).await;
    }

    #[tokio::test]
    async fn test_recursive_wildcard_routes() {
        let record = Arc::new(Mutex::new(Vec::new()));
        let processor = FinalMockedProcessor::new(record);

        test_route(&processor, "user.banned.foo", 5).await;
        test_route(&processor, "user.banned.foo.bar", 5).await;
        test_route(&processor, "user.banned.foo.bar.baz", 5).await;
    }

    #[tokio::test]
    async fn test_nested_static_routes() {
        let record = Arc::new(Mutex::new(Vec::new()));
        let processor = FinalMockedProcessor::new(record);

        test_route(&processor, "invoice.create", 1).await;
        test_route(&processor, "invoice.update", 2).await;
    }

    #[tokio::test]
    async fn test_nested_wildcard_routes() {
        let record = Arc::new(Mutex::new(Vec::new()));
        let processor = FinalMockedProcessor::new(record);

        test_route(&processor, "user.foo.register", 4).await;
        test_route(&processor, "user.banned.something", 5).await;
    }

    #[tokio::test]
    async fn test_deeply_nested_routes() {
        let record = Arc::new(Mutex::new(Vec::new()));
        let processor = FinalMockedProcessor::new(record);

        test_route(&processor, "deep.nested.route", 1).await;
    }

    #[tokio::test]
    async fn test_wildcard_with_nested_routes() {
        let record = Arc::new(Mutex::new(Vec::new()));
        let processor = FinalMockedProcessor::new(record);

        test_route(&processor, "anything.create", 1).await;
        test_route(&processor, "something.update", 2).await;
    }

    #[tokio::test]
    async fn test_unmatched_routes() {
        let record = Arc::new(Mutex::new(Vec::new()));
        let processor = FinalMockedProcessor::new(record);

        // Test various unmatched patterns
        let test_cases = vec![
            "unmatched.route",
            "invoice",  // Incomplete path
            "user.register",  // Missing segment
            "unknown.path.here",
            "deep.nested",  // Incomplete nested path
        ];

        for subject in test_cases {
            let msg = generate_empty_message(subject);
            let result = processor.process(msg).await;
            assert!(
                matches!(result, Err(Error::PreProcessError(_))),
                "Expected PreProcessError for subject: {}", subject
            );
        }
    }
}

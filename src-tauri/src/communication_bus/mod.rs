use tokio::sync::{broadcast, mpsc};
use crate::common_types::Message;

const BUS_CHANNEL_CAPACITY: usize = 100; // Define a capacity for the broadcast channel

#[derive(Debug)] // Add Debug trait
pub struct CommunicationBus {
    sender: broadcast::Sender<Message>,
    request_sender: mpsc::Sender<BusRequest>,
}

impl CommunicationBus {
    /// Creates a new instance of the CommunicationBus.
    pub fn new() -> Self {
        let (sender, _receiver) = broadcast::channel(BUS_CHANNEL_CAPACITY);
        let (request_sender, _request_receiver) = mpsc::channel(BUS_CHANNEL_CAPACITY);
        CommunicationBus { sender, request_sender }
    }

    /// Allows an agent or component to subscribe to messages on the bus.
    pub fn subscribe(&self) -> broadcast::Receiver<Message> {
        self.sender.subscribe()
    }

    /// Publishes a message onto the bus.
    /// Returns Ok(()) if the message was sent successfully, or an error if the channel is closed.
    pub fn publish(&self, message: Message) -> Result<(), broadcast::error::SendError<Message>> {
        // broadcast::send returns the number of receivers that received the message,
        // but we only care if it succeeded or failed.
        self.sender.send(message).map(|_| ())
    }

    // Optional: Add a method to get a clone of the sender for spawning tasks
    pub fn get_sender(&self) -> broadcast::Sender<Message> {
        self.sender.clone()
    }

    // Add a method to get an mpsc sender for BusRequests
    pub fn get_bus_request_sender(&self) -> tokio::sync::mpsc::Sender<BusRequest> {
        self.request_sender.clone()
    }
}

// Define the types of requests that can be sent TO the communication bus
#[derive(Debug)] // Derive Debug for the enum
pub enum BusRequest {
    GeneralMessage { message: Message }, // Request to publish a general message
    AgentResponse { message: crate::common_types::message_defs::AgentResponse },
}

// Add tests later if needed
#[cfg(test)]
mod tests {
    use super::*;
    use crate::common_types::{MessageContent, generate_id};
    use tokio::time::{self, Duration};

    #[tokio::test]
    async fn test_communication_bus() {
        let bus = CommunicationBus::new();
        let mut receiver1 = bus.subscribe();
        let mut receiver2 = bus.subscribe();

        let message = Message {
            id: generate_id(),
            sender_id: "test_sender".to_string(),
            receiver_id: None,
            content: MessageContent::DataFragment { data: "Hello, HIVE!".to_string() },
        };

        // Publish the message
        let publish_result = bus.publish(message.clone());
        assert!(publish_result.is_ok());

        // Wait for receivers to get the message
        time::sleep(Duration::from_millis(10)).await;

        // Check if receivers received the message
        let received1 = receiver1.recv().await;
        assert!(received1.is_ok());
        assert_eq!(received1.unwrap().content, message.content);

        let received2 = receiver2.recv().await;
        assert!(received2.is_ok());
        assert_eq!(received2.unwrap().content, message.content);
    }
}

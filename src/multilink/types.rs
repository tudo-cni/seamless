use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc::{Receiver, Sender, UnboundedReceiver, UnboundedSender};
use tokio::sync::RwLock;

use crate::multilink::{link::Link, packet};

// Mostly for clearance and readability
// TxQueue and RxQueues for byte vectors aka packet
// Example Topology:
// Multilink -> TxQueue --> RxQueue -> Link1
pub type TxQueue = Sender<(packet::MultiLinkHeader, Vec<u8>)>;
pub type RxQueue = Receiver<Vec<u8>>;

// Tuple of Two TxQueue
// In case of the Link struct the first Queue is considered the system message
// while the second one is considered the data queue

// HashMap mapping the link name to a MutexGuard of the Link Struct
// and the TxQueues to send data to the Link
pub type SharedLink = Arc<RwLock<Link>>;
pub type LinkHashMap = HashMap<String, SharedLink>;

// The multilink needs to know from which link he received the packet.
// Therefor we queue both the links name and the actual packet
// Example Topology:
// Link1 -> MultiLinkTxQueue --> MultiLinkRxQueue -> Multilink
pub type MultiLinkTxQueue = UnboundedSender<(String, Vec<u8>)>;
pub type MultiLinkRxQueue = UnboundedReceiver<(String, Vec<u8>)>;

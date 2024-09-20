use anyhow::Result;
use std::vec;
use std::{collections::HashSet, collections::VecDeque, sync::Arc};
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio::time::Duration;

#[cfg(feature = "tracing")]
use tracing::info;

use crate::multilink::reorder::{ReorderBuffer, ReorderResults};

struct Deduplication {
    _set: HashSet<u32>,
    _order: VecDeque<u32>,
}

impl Deduplication {
    fn new(size: usize) -> Self {
        let set = HashSet::with_capacity(size);
        let order = VecDeque::with_capacity(size);
        Deduplication {
            _set: set,
            _order: order,
        }
    }

    fn _push(&mut self, id: u32) {
        if self._len() == self._capacity() {
            self._pop();
        }
        if self._set.insert(id) {
            self._order.push_back(id);
        }
    }

    fn _pop(&mut self) -> bool {
        match self._order.pop_front() {
            Some(id) => self._set.remove(&id),
            None => false,
        }
    }

    fn _contains(&self, id: u32) -> bool {
        self._set.contains(&id)
    }

    fn _len(&self) -> usize {
        self._set.len()
    }
    fn _capacity(&self) -> usize {
        self._set.capacity()
    }
    fn is_duplicate(&mut self, id: u32) -> bool {
        if self._contains(id) {
            true
        } else {
            self._push(id);
            false
        }
    }
}

pub struct Stream {
    _reorder_to_tun_sender: UnboundedSender<Vec<u8>>,
    _reorder: Arc<Mutex<ReorderBuffer>>,
    _reorder_drain_event: Option<JoinHandle<()>>,
    _used_links: Vec<String>,
    _reorder_timeout: Duration,
    _deduplication: Deduplication,
}

impl Stream {
    pub fn new(
        sender: UnboundedSender<Vec<u8>>,
        reorder_size: Option<u32>,
        reorder_timeout: Option<Duration>,
    ) -> Self {
        Stream {
            _reorder_to_tun_sender: sender.clone(),
            _reorder: Arc::new(Mutex::new(ReorderBuffer::new(
                reorder_size.unwrap_or(128),
                sender,
            ))),
            _reorder_drain_event: None,
            _used_links: vec![],
            _reorder_timeout: reorder_timeout.unwrap_or(Duration::from_millis(100)),
            _deduplication: Deduplication::new(1000),
        }
    }

    pub fn insert_used_link(&mut self, link: String) {
        if !self._used_links.contains(&link) {
            self._used_links.push(link);
        }
    }

    pub fn set_reorder_timeout(&mut self, reorder_timeout: u64) {
        self._reorder_timeout = Duration::from_millis(reorder_timeout * 2);
    }

    pub fn get_used_links(&self) -> Vec<String> {
        self._used_links.clone()
    }

    #[cfg_attr(feature = "deep_tracing", tracing::instrument(skip_all))]
    pub async fn handle_rx(
        &mut self,
        new_sequence: u32,
        packet: Vec<u8>,
        dedup: bool,
    ) -> Result<()> {
        if dedup && self._deduplication.is_duplicate(new_sequence) {
            return Ok(());
        }
        if let Some(x) = &self._reorder_drain_event {
            if x.is_finished() {
                self._reorder_drain_event = None
            }
        }

        match self
            ._reorder
            .lock()
            .await
            .handle_packet(packet, new_sequence)?
        {
            ReorderResults::Initialized => {}
            ReorderResults::InOrderPassthrough => {}
            ReorderResults::OutofOrderPassthrough => {}
            ReorderResults::Inserted => {
                if self._reorder_drain_event.is_none() {
                    let arc = Arc::clone(&self._reorder);
                    let timeout = self._reorder_timeout;
                    let task = tokio::spawn(async move {
                        tokio::time::sleep(timeout).await;
                        let mut locked = arc.lock().await;

                        #[cfg(feature = "tracing")]
                        info!("drained from timeout");

                        let _ = locked.drain_complete_buffer().unwrap();
                        drop(locked);
                    });
                    self._reorder_drain_event = Some(task);
                }
            }
            ReorderResults::ArrivalDrain
            | ReorderResults::CapacityDrain
            | ReorderResults::TimeoutDrain => {
                if let Some(x) = &self._reorder_drain_event {
                    x.abort();
                    self._reorder_drain_event = None;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::multilink::mpsc;

    // Unit Tests for Deduplication
    #[test]
    fn test_new_deduplication() {
        let dedup = Deduplication::new(5);
        assert_eq!(dedup._len(), 0);
        assert!(dedup._capacity() >= 5);
    }

    #[test]
    fn test_push_pop() {
        let mut dedup = Deduplication::new(2);
        // Test pushing elements
        dedup._push(1);
        dedup._push(2);
        dedup._push(3);
        assert_eq!(dedup._len(), 3);
        assert!(dedup._contains(1));
        assert!(dedup._contains(2));
        assert!(dedup._contains(3));

        // Test popping element
        assert!(dedup._pop());
        assert_eq!(dedup._len(), 2);
        assert!(!dedup._contains(1));
        assert!(dedup._contains(2));
        assert!(dedup._contains(3));
    }

    #[test]
    fn test_capacity() {
        let mut dedup = Deduplication::new(3);
        assert_eq!(dedup._capacity(), 3);

        // Fill the deduplication set
        dedup.is_duplicate(1);
        dedup.is_duplicate(2);
        dedup.is_duplicate(3);

        // At capacity, new ID (4) should push out the oldest (1)
        dedup.is_duplicate(4);
        assert_eq!(dedup.is_duplicate(1), false); // 1 is not a duplicate and therefore is added to the set -> it pushes out the oldest (2)
        assert_eq!(dedup.is_duplicate(3), true); // 3, 4 should still be in
        assert_eq!(dedup.is_duplicate(4), true);
    }

    #[test]
    fn test_is_duplicate() {
        let mut dedup = Deduplication::new(5);

        // Initially, no ID is a duplicate
        assert_eq!(dedup.is_duplicate(1), false);
        assert_eq!(dedup.is_duplicate(2), false);

        // Same IDs should now be duplicates
        assert_eq!(dedup.is_duplicate(1), true);
        assert_eq!(dedup.is_duplicate(2), true);

        // New ID should not be a duplicate
        assert_eq!(dedup.is_duplicate(3), false);
    }

    // Unit Tests for Stream
    #[tokio::test]
    async fn test_handle_rx_no_deduplication() {
        let (channel_leaving_multilink_tx, _channel_leaving_multilink_rx): (
            mpsc::UnboundedSender<Vec<u8>>,
            mpsc::UnboundedReceiver<Vec<u8>>,
        ) = mpsc::unbounded_channel::<Vec<u8>>();
        let mut stream = Stream::new(channel_leaving_multilink_tx, None, None);

        stream.insert_used_link("link1".to_string());

        // Test handle_rx with no deduplication
        assert!(
            stream.handle_rx(1, vec![0], true).await.is_ok(),
            "Expected Ok(())"
        );

        assert_eq!(
            stream.get_used_links(),
            vec!["link1".to_string()],
            "Used links should remain unchanged"
        );
    }

    #[tokio::test]
    async fn test_handle_rx_deduplication_duplicate() {
        let (channel_leaving_multilink_tx, _channel_leaving_multilink_rx): (
            mpsc::UnboundedSender<Vec<u8>>,
            mpsc::UnboundedReceiver<Vec<u8>>,
        ) = mpsc::unbounded_channel::<Vec<u8>>();
        let mut stream = Stream::new(channel_leaving_multilink_tx, None, None);

        stream.insert_used_link("link1".to_string());

        // Test handle_rx with deduplication enabled and a duplicate sequence
        assert!(
            stream.handle_rx(1, vec![0], true).await.is_ok(),
            "Expected Ok(()) for first packet"
        );
        assert!(
            stream.handle_rx(1, vec![0], true).await.is_ok(),
            "Expected Ok(()) for duplicate packet"
        );

        assert_eq!(
            stream.get_used_links(),
            vec!["link1".to_string()],
            "Used links should remain unchanged"
        );
    }

    #[tokio::test]
    async fn test_handle_rx_deduplication_no_duplicate() {
        let (channel_leaving_multilink_tx, _channel_leaving_multilink_rx): (
            mpsc::UnboundedSender<Vec<u8>>,
            mpsc::UnboundedReceiver<Vec<u8>>,
        ) = mpsc::unbounded_channel::<Vec<u8>>();
        let mut stream = Stream::new(channel_leaving_multilink_tx, None, None);

        stream.insert_used_link("link1".to_string());

        // Test handle_rx with deduplication enabled and a new sequence
        assert!(
            stream.handle_rx(1, vec![0], true).await.is_ok(),
            "Expected Ok(()) for first packet"
        );
        assert!(
            stream.handle_rx(2, vec![0], true).await.is_ok(),
            "Expected Ok(()) for non-duplicate packet"
        );

        assert_eq!(
            stream.get_used_links(),
            vec!["link1".to_string()],
            "Used links should remain unchanged"
        );
    }
}

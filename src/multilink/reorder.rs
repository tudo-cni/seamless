use anyhow::{Context, Result};
use priority_queue::PriorityQueue;
use std::cmp::Reverse;
use tokio::sync::mpsc::UnboundedSender;
use tokio::task::JoinHandle;
use tokio::time::Duration;

#[allow(dead_code)]
pub enum ReorderResults {
    Initialized,
    InOrderPassthrough,
    OutofOrderPassthrough,
    Inserted,
    ArrivalDrain,
    TimeoutDrain,
    CapacityDrain,
}
pub struct ReorderBuffer {
    _buffer: PriorityQueue<Vec<u8>, Reverse<u32>>,
    _tun_writer: UnboundedSender<Vec<u8>>,
    _drain_timeout: Option<JoinHandle<()>>,
    _drain_timeout_delay: Option<Duration>,
    _max_size: usize,
    pub _sequence: u32,
}
impl ReorderBuffer {
    pub fn new(size: u32, _tun_writer: UnboundedSender<Vec<u8>>) -> ReorderBuffer {
        ReorderBuffer {
            _buffer: PriorityQueue::with_capacity(size as usize),
            _tun_writer,
            _drain_timeout: None,
            _drain_timeout_delay: None,
            _max_size: size as usize,
            _sequence: 0,
        }
    }

    pub fn buffer_is_empty(&self) -> bool {
        self._buffer.is_empty()
    }

    pub fn min_sequence(&self) -> Option<u32> {
        self._buffer.peek().map(|value| value.1 .0.to_owned())
    }

    pub fn handle_packet(
        &mut self,
        packet: Vec<u8>,
        packet_sequence: u32,
    ) -> Result<ReorderResults> {
        if packet_sequence == 0 {
            // First Packet, nothing to reorder
            self._sequence = 0;
            self._tun_writer.send(packet)?;
            Ok(ReorderResults::Initialized)
        } else if packet_sequence == self._sequence + 1 {
            // Packet in Order, just forwarding
            self._sequence = packet_sequence;
            self._tun_writer.send(packet)?;

            if self.min_sequence().unwrap_or(0) == packet_sequence + 1 {
                // Packet in "Order" and we can drain following

                if self.drain_next_continous_packet_chunk()? {
                    return Ok(ReorderResults::ArrivalDrain);
                }
            }
            return Ok(ReorderResults::InOrderPassthrough);
        } else if packet_sequence < self._sequence + 1 {
            self._tun_writer.send(packet)?;
            return Ok(ReorderResults::OutofOrderPassthrough);
        } else {
            match self.insert_packet(packet, packet_sequence) {
                Ok(_) => return Ok(ReorderResults::Inserted),
                Err(_) => return Ok(ReorderResults::CapacityDrain),
            };
        }
    }

    pub fn insert_packet(&mut self, packet: Vec<u8>, sequence: u32) -> Result<bool> {
        //println!("Cap: {:?}",self._buffer.capacity());
        if self._buffer.len() < self._max_size - 1 {
            self._buffer.push(packet, Reverse(sequence));
            Ok(true)
        } else {
            //Theoretically we can extend the buffer as much as we want but here we want to stay to a strict max size
            // Drain Queue
            self._buffer.push(packet, Reverse(sequence));
            let _ = self.drain_complete_buffer()?;
            Ok(false)
        }
    }

    pub fn drain_next_continous_packet_chunk(&mut self) -> Result<bool> {
        if self.buffer_is_empty() {
            return Ok(false);
        }
        let mut sequence = self
            ._buffer
            .peek()
            .context("Reorder Logic Error. Trying to read packet of empty buffer...")?
            .1
             .0;

        loop {
            self._tun_writer.send(
                self._buffer
                    .pop()
                    .context("Error in popping Packet from Reorder buffer...")?
                    .0,
            )?;

            match self._buffer.peek() {
                Some(value) => {
                    if value.1 .0 == sequence + 1 {
                        sequence = value.1 .0;
                    } else {
                        break;
                    }
                }
                _ => {
                    break;
                }
            }
        }

        self._sequence = sequence;
        Ok(true)
    }

    pub fn drain_complete_buffer(&mut self) -> Result<bool> {
        let mut sequence = 0;

        while let Some((packet, seq)) = self._buffer.pop() {
            self._tun_writer.send(packet)?;
            sequence = seq.0;
        }
        self._sequence = sequence;
        Ok(true)
    }
}

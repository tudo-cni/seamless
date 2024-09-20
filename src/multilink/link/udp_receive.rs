use std::io;
use std::sync::Arc;

use crate::multilink::types::MultiLinkTxQueue;
use tokio::net::UdpSocket;

pub struct UdpReceive {
    _name: String,
    _receive_channel: MultiLinkTxQueue,
    _socket: Arc<UdpSocket>,
}

impl UdpReceive {
    pub fn new(name: String, receive_channel: MultiLinkTxQueue, socket: Arc<UdpSocket>) -> Self {
        UdpReceive {
            _receive_channel: receive_channel,
            _socket: socket,
            _name: name,
        }
    }

    pub async fn handle_rx(&mut self) {
        let mut buf = [0; 2000];
        loop {
            self._socket
                .readable()
                .await
                .expect("UdpSocket not readable");
            match self._socket.try_recv_from(&mut buf) {
                Ok((len, _addr)) => {
                    self._receive_channel
                        .send((self._name.clone(), buf[..len].to_vec()))
                        .expect("Couldn't send");
                    continue;
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    continue;
                }
                Err(_) => {
                    break;
                }
            }
        }
    }
}

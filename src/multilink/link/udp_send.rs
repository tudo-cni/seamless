use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::mpsc::Receiver;
use tokio::sync::RwLock;

#[cfg(feature = "tracing")]
use tracing::error;

use crate::multilink::link;
use crate::multilink::packet;

pub struct UdpSend {
    _socket: Arc<UdpSocket>,
    _data_packet_channel: Receiver<(packet::MultiLinkHeader, Vec<u8>)>,
    _system_packet_channel: Receiver<(packet::MultiLinkHeader, Vec<u8>)>,
    _remote_addr: SocketAddr,
    _link_seq: u32,
    _link_arc: Arc<RwLock<link::Link>>,
}

impl UdpSend {
    pub fn new(
        data_packet_channel: Receiver<(packet::MultiLinkHeader, Vec<u8>)>,
        system_packet_channel: Receiver<(packet::MultiLinkHeader, Vec<u8>)>,
        socket: Arc<UdpSocket>,
        remote_addr: SocketAddr,
        link_arc: Arc<RwLock<link::Link>>,
    ) -> Self {
        UdpSend {
            _socket: socket,
            _data_packet_channel: data_packet_channel,
            _system_packet_channel: system_packet_channel,
            _remote_addr: remote_addr,
            _link_seq: 0,
            _link_arc: link_arc,
        }
    }
    fn _get_next_sequence(&mut self) -> u32 {
        self._link_seq = self._link_seq.wrapping_add(1);
        self._link_seq
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    pub async fn handle_tx(&mut self) {
        loop {
            let (mut header, bytes) = tokio::select! {
                biased;
                value = self._system_packet_channel.recv() => value.expect("Something bigger broke..."),
                value = self._data_packet_channel.recv() => value.expect("Something bigger broke..."),
            };
            header.set_link_seq(self._get_next_sequence());
            let mut encoded: Vec<u8> = header.into_bytes().to_vec();
            encoded.extend_from_slice(&bytes);
            let mut bytes_send = 0;

            while bytes_send < encoded.len() {
                match self
                    ._socket
                    .send_to(&encoded[bytes_send..encoded.len()], self._remote_addr)
                    .await
                {
                    Ok(value) => {
                        bytes_send += value;
                    }
                    Err(_e) => {
                        #[cfg(feature = "tracing")]
                        error!("{:?}", _e);

                        self._link_arc.write().await.notify_interface_down();
                        break;
                    }
                }
            }
        }
    }
}

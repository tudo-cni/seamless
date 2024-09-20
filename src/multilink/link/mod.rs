use serde_derive::Deserialize;
use std::fmt;
use std::net::IpAddr;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::UNIX_EPOCH;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use tokio::sync::RwLock;
use tokio::time::Duration;

#[cfg(feature = "tracing")]
use tracing::{info, trace};

use crate::multilink::packet;
use crate::multilink::technology::{self, GetValue, NormedQuality, SetValue, Technology};
use crate::multilink::types::{MultiLinkTxQueue, SharedLink, TxQueue};

mod udp_receive;
mod udp_send;

#[derive(Debug, Clone, PartialEq)]
pub enum LinkState {
    Down,
    Healthy,
    HighLatency,
    HighLoss,
    HighLatencyAndHighLoss,
}

impl fmt::Display for LinkState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[allow(dead_code)]
#[derive(Debug)]
pub enum RttComputationMethod {
    Plain,
    SmoothedTcpStyle,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "name")]
pub struct LinkConfig {
    #[serde(rename = "$key$")]
    pub name: String,
    pub local_ip: IpAddr,
    pub local_port: u16,
    pub remote_ip: IpAddr,
    pub remote_port: u16,
    #[serde(
        default,
        deserialize_with = "crate::multilink::deserialize_optional_duration"
    )]
    pub rtt_tolerance: Option<Duration>,
    #[serde(
        default,
        deserialize_with = "crate::multilink::deserialize_optional_duration"
    )]
    pub timeout_tolerance: Option<Duration>,
    #[serde(
        default,
        deserialize_with = "crate::multilink::deserialize_optional_duration"
    )]
    pub keepalive_interval: Option<Duration>,
    pub loss_tolerance: Option<f32>,
    #[serde(default)]
    pub technology: Technology,
    pub external_measurement: Option<String>,
}
#[allow(dead_code)]
#[derive(Debug)]
pub struct Link {
    pub name: String,
    _local_addr: SocketAddr,
    _remote_addr: SocketAddr,
    _rtt_tolerance: f32,
    _loss_tolerance: f32,
    _rtt_computation_method: RttComputationMethod,
    _rtt_variance: Option<f32>,
    _timeout_tolerance: u128,
    _keepalive_interval: u128,
    _rtt: Option<f32>,
    _saved_timestamp: Option<u16>,
    _saved_timestamp_received_at: Option<u16>,
    _state: LinkState,
    _link_seq_received: u32,
    _link_seq_received_last_down: u32,
    _link_seq_buffer: u128,
    _link_id: u8,
    _remote_link_id: Option<u8>,
    _next_keepalive: Option<u128>,
    _last_keepalive_sent: Option<u128>,
    _last_keepalive_received: Option<u128>,
    _bytes_sent: u128,
    _bytes_sent_total: u128,
    _bytes_received: u128,
    _bytes_received_total: u128,
    _last_datarate_calculation_timestamp: u128,
    _current_datarate_in: f64,
    _current_datarate_out: f64,
    _send_system_channel: TxQueue,
    _send_data_channel: TxQueue,
    pub _technology: technology::TechnologyData,
}

impl Link {
    pub async fn new(config: &LinkConfig, tx: MultiLinkTxQueue, link_id: u8) -> SharedLink {
        let local_addr = SocketAddr::new(config.local_ip, config.local_port);
        let remote_addr = SocketAddr::new(config.remote_ip, config.remote_port);

        let (system_packet_channel_tx, system_packet_channel_rx) =
            mpsc::channel::<(packet::MultiLinkHeader, Vec<u8>)>(1);
        let (data_packet_channel_tx, data_packet_channel_rx) =
            mpsc::channel::<(packet::MultiLinkHeader, Vec<u8>)>(1);
        let socket = Arc::new(
            UdpSocket::bind(&local_addr)
                .await
                .expect("Couldn't bind UDP Port"),
        );

        let rtt_tolerance = config
            .rtt_tolerance
            .unwrap_or(Duration::from_millis(500))
            .as_millis() as f32;
        let timeout_tolerance = config
            .timeout_tolerance
            .unwrap_or(Duration::from_secs(1))
            .as_millis();
        let keepalive_interval = config
            .keepalive_interval
            .unwrap_or(Duration::from_millis(100))
            .as_millis();
        let link = Link {
            name: config.name.clone(),
            _local_addr: local_addr,
            _remote_addr: remote_addr,
            _rtt: None,
            _rtt_tolerance: rtt_tolerance,
            _rtt_computation_method: RttComputationMethod::SmoothedTcpStyle,
            _rtt_variance: None,
            _loss_tolerance: config.loss_tolerance.unwrap_or(10.0),
            _timeout_tolerance: timeout_tolerance,
            _keepalive_interval: keepalive_interval,
            _saved_timestamp: None,
            _saved_timestamp_received_at: None,
            _state: LinkState::Healthy,
            _link_seq_received: 0,
            _link_seq_received_last_down: 0,
            _link_seq_buffer: 0,
            _link_id: link_id,
            _remote_link_id: None,
            _next_keepalive: None,
            _last_keepalive_received: Some(0),
            _last_keepalive_sent: Some(0),
            _bytes_received: 0,
            _bytes_received_total: 0,
            _bytes_sent: 0,
            _bytes_sent_total: 0,
            _last_datarate_calculation_timestamp: std::time::SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("Time went backwards")
                .as_millis(),
            _current_datarate_in: 0.0,  // Byte/s
            _current_datarate_out: 0.0, // Byte/s
            _send_data_channel: data_packet_channel_tx,
            _send_system_channel: system_packet_channel_tx,
            _technology: technology::TechnologyData::new(&config.technology),
        };

        let link_arc: SharedLink = Arc::new(RwLock::new(link));
        let mut udp_receiver =
            udp_receive::UdpReceive::new(config.name.clone(), tx, socket.clone());
        let mut udp_sender = udp_send::UdpSend::new(
            data_packet_channel_rx,
            system_packet_channel_rx,
            socket,
            remote_addr,
            link_arc.clone(),
        );
        tokio::spawn(async move { udp_sender.handle_tx().await });
        tokio::spawn(async move { udp_receiver.handle_rx().await });
        link_arc
    }

    #[cfg_attr(feature = "deep_tracing", tracing::instrument)]
    pub fn reset(&mut self) {
        #[cfg(feature = "tracing")]
        info!("resetting Link");

        self._rtt = None;
        self._rtt_variance = None;
        self._saved_timestamp = None;
        self._saved_timestamp_received_at = None;
        self._state = LinkState::Healthy;
        self._link_seq_received = 0;
        self._link_seq_received_last_down = 0;
        self._link_seq_buffer = 0;
        self._remote_link_id = None;
        self._next_keepalive = None;
        self._last_keepalive_received = Some(0);
        self._last_keepalive_sent = Some(0);
        self._bytes_received = 0;
        self._bytes_received_total = 0;
        self._bytes_sent = 0;
        self._bytes_sent_total = 0;
        self._last_datarate_calculation_timestamp = std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis();
        self._current_datarate_in = 0.0;
        self._current_datarate_out = 0.0;
    }

    pub fn calculate_datarates(&mut self) {
        let timestamp_now = std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis();

        let time_passed =
            (timestamp_now - self._last_datarate_calculation_timestamp) as f64 / 1000.0;

        self._current_datarate_out = self._bytes_sent as f64 / time_passed;
        self._current_datarate_in = self._bytes_received as f64 / time_passed;

        self._bytes_sent_total += self._bytes_sent;
        self._bytes_received_total += self._bytes_received;
        self._bytes_sent = 0;
        self._bytes_received = 0;

        self._last_datarate_calculation_timestamp = timestamp_now;
    }

    pub fn get_datarate_out(&self) -> f64 {
        self._current_datarate_out
    }

    #[cfg_attr(feature = "deep_tracing", tracing::instrument)]
    pub fn notify_interface_down(&mut self) {
        self._update_link_state(LinkState::Down);

        self._rtt = Some(f32::MAX);
        self._rtt_variance = Some(0.0);
    }

    pub fn get_datarate_in(&self) -> f64 {
        self._current_datarate_in
    }
    pub fn get_link_state(&self) -> LinkState {
        self._state.clone()
    }
    pub fn get_round_trip_time(&self) -> Option<f32> {
        self._rtt
    }

    pub fn update_loss(&mut self, seq: u32, moving_average_size: u32) {
        if let LinkState::Down = self._state {
            self._link_seq_received_last_down = seq;
        }

        if seq > self._link_seq_received {
            // We are either in order or out of order with an overflow
            if self._link_seq_received < moving_average_size && seq > u32::MAX - moving_average_size
            {
                // Out of Order Overflow
                let delta = self._link_seq_received.wrapping_sub(seq);
                if delta < moving_average_size {
                    self._link_seq_buffer |= 1 << delta;
                }
            } else {
                // Assuming In order
                // In Order and in our horizon. Shift bits accordingly
                let bit_to_shift = seq.wrapping_sub(self._link_seq_received);
                if bit_to_shift < 128 {
                    self._link_seq_buffer <<= seq.wrapping_sub(self._link_seq_received);
                } else {
                    self._link_seq_buffer = 0;
                }
                self._link_seq_buffer |= 1;
                self._link_seq_received = seq;
            }
        } else {
            // We are either out of order or in order with an overflow
            if seq < moving_average_size && self._link_seq_received > u32::MAX - moving_average_size
            {
                // In order Overflow
                let delta = seq.wrapping_sub(self._link_seq_received);
                self._link_seq_buffer <<= delta;
                self._link_seq_buffer |= 1;
                self._link_seq_received = seq;
            } else {
                // Out of Order
                let delta = self._link_seq_received - seq;
                if delta < moving_average_size {
                    // In Order and in our horizon. Shift bits accordingly
                    self._link_seq_buffer |= 1 << delta;
                }
            }
        }
    }
    pub fn get_seq_received(&self) -> u32 {
        self._link_seq_received - self._link_seq_received_last_down
    }

    #[cfg_attr(feature = "deep_tracing", tracing::instrument)]
    pub fn check_high_latency(&mut self) {
        if self._rtt.unwrap_or(0.0) <= self._rtt_tolerance {
            match self._state {
                LinkState::HighLatency => {
                    self._update_link_state(LinkState::Healthy);
                }
                LinkState::HighLatencyAndHighLoss => {
                    self._update_link_state(LinkState::HighLoss);
                }
                LinkState::Healthy | LinkState::HighLoss | LinkState::Down => {}
            }
        } else {
            match self._state {
                LinkState::Healthy => {
                    self._update_link_state(LinkState::HighLatency);
                }
                LinkState::HighLoss => {
                    self._update_link_state(LinkState::HighLatencyAndHighLoss);
                }
                _ => {}
            }
        }
    }

    #[cfg_attr(feature = "deep_tracing", tracing::instrument)]
    pub fn check_high_loss(&mut self) {
        if self.get_loss(128) <= self._loss_tolerance {
            match self._state {
                LinkState::HighLoss => {
                    self._update_link_state(LinkState::Healthy);
                }
                LinkState::HighLatencyAndHighLoss => {
                    self._update_link_state(LinkState::HighLatency);
                }
                _ => {}
            }
        } else {
            match self._state {
                LinkState::Healthy => {
                    self._update_link_state(LinkState::HighLoss);
                }
                LinkState::HighLatency => {
                    self._update_link_state(LinkState::HighLatencyAndHighLoss);
                }
                _ => {}
            }
        }
    }

    pub fn _update_link_state(&mut self, state: LinkState) {
        if state == self._state {
            return;
        }
        #[cfg(feature = "tracing")]
        info!(
            "LinkState of {} changed from {:?} to {:?}",
            self.name, self._state, state
        );
        self._state = state;
    }

    pub fn update_last_keepalive_received(&mut self, timestamp: u128) {
        self._last_keepalive_received = Some(timestamp);
    }

    pub fn update_last_keepalive_sent(&mut self, timestamp: u128) {
        self._last_keepalive_sent = Some(timestamp);
    }

    #[allow(dead_code)]
    pub fn get_last_keepalive_received(&self) -> u128 {
        self._last_keepalive_received
            .expect("Something bigger broke...")
    }

    #[allow(dead_code)]
    pub fn get_last_keepalive_sent(&self) -> u128 {
        self._last_keepalive_sent
            .expect("Something bigger broke...")
    }

    pub fn update_timestamps(&mut self, timestamp: u16, timestamp_received_at: u16) {
        self._saved_timestamp = Some(timestamp);
        self._saved_timestamp_received_at = Some(timestamp_received_at);
    }

    pub fn update_remote_link_id(&mut self, remote_link_id: u8) {
        self._remote_link_id = Some(remote_link_id);
    }

    #[cfg_attr(feature = "deep_tracing", tracing::instrument)]
    pub fn update_rtt(&mut self, timestamp_reply: u16) {
        let rtt = (self
            ._saved_timestamp_received_at
            .expect("Something bigger Broke..."))
        .wrapping_sub(timestamp_reply) as f32;
        match self._rtt_computation_method {
            RttComputationMethod::Plain => {
                self._rtt = Some(rtt);

                #[cfg(feature = "tracing")]
                trace!("Plain RTT of {} is {}", self.name, self._rtt.unwrap());
            }
            RttComputationMethod::SmoothedTcpStyle => {
                if self._rtt.is_none() || self._rtt == Some(f32::MAX) {
                    /* first measurement */
                    self._rtt = Some(rtt);
                    self._rtt_variance = Some(rtt / 2.0);

                    #[cfg(feature = "tracing")]
                    trace!(
                        "Smoothed RTT of {} is {} and variance is {}",
                        self.name,
                        self._rtt.unwrap(),
                        self._rtt_variance.unwrap()
                    );
                } else {
                    let alpha = 1.0 / 8.0;
                    let beta = 1.0 / 4.0;
                    self._rtt_variance = Some(
                        (1.0 - beta) * self._rtt_variance.expect("Something bigger Broke...")
                            + (beta * (self._rtt.expect("Something bigger Broke...") - rtt).abs()),
                    );
                    self._rtt = Some(
                        (1.0 - alpha) * self._rtt.expect("Something bigger Broke...")
                            + (alpha * rtt),
                    );

                    #[cfg(feature = "tracing")]
                    trace!(
                        "rtt is {} and variance is {}",
                        self._rtt.unwrap(),
                        self._rtt_variance.unwrap()
                    );
                }
            }
        }
    }

    #[cfg_attr(feature = "deep_tracing", tracing::instrument)]
    pub fn update_technology(&mut self, field: String, value: String) {
        #[cfg(feature = "tracing")]
        info!("updating technology");

        self._technology.set_value(field, value);
    }

    pub fn get_quality(&self) -> f32 {
        match &self._technology {
            technology::TechnologyData::Nr(tech) => tech.get_normed_quality(),
            technology::TechnologyData::Wifi(tech) => tech.get_normed_quality(),
            technology::TechnologyData::Generic | technology::TechnologyData::Lte(_) => 0.0,
        }
    }

    pub fn get_kpi(&self, kpi: String) -> Option<f32> {
        self._technology.get_value(kpi)
    }

    pub fn get_next_keepalive_due(&self, timestamp_now: u128) -> bool {
        timestamp_now
            > self._last_keepalive_sent.expect("Something bigger broke") + self._keepalive_interval
    }

    pub fn get_rtt_calculation_due(&mut self, timestamp_now: u128) -> Option<u16> {
        let mut response = None;
        if self._saved_timestamp_received_at.is_some() && self._saved_timestamp.is_some() {
            let time_since_received = (timestamp_now as u16).wrapping_sub(
                self._saved_timestamp_received_at
                    .expect("Something bigger broke..."),
            );
            //TODO "recent" timestamp depends somewhat on actually roundtrip time of the link. better use some kind of rtt instead of 1000
            if time_since_received < 1000 {
                response = Some(
                    self._saved_timestamp
                        .expect("Something bigger broke...")
                        .wrapping_add(time_since_received),
                );
            }
            self._saved_timestamp_received_at = None;
            self._saved_timestamp = None;
        }
        response
    }

    #[cfg_attr(feature = "deep_tracing", tracing::instrument)]
    pub fn get_link_timeout(&mut self, timestamp_now: u128) -> bool {
        if (self
            ._last_keepalive_received
            .expect("Something bigger broke...")
            != 0)
            && (self
                ._last_keepalive_received
                .expect("Something bigger broke...")
                + self._timeout_tolerance)
                < timestamp_now
        {
            self._update_link_state(LinkState::Down);

            return true;
        }

        if let LinkState::Down = self._state {
            // Set it to Healthy. Latency and Loss is checked in next calls on Higher level.
            self._update_link_state(LinkState::Healthy);
        }
        false
    }

    pub fn get_loss(&mut self, moving_average_size: u16) -> f32 {
        let mut lost: f32 = 0.0;

        for i in 0..moving_average_size {
            if (1 & (self._link_seq_buffer >> i)) == 0 {
                lost += 1.0;
            }
        }

        lost * 100.0 / (moving_average_size as f32)
    }

    pub fn increment_bytes_sent(&mut self, number_of_bytes: u32) {
        #[cfg(feature = "tracing")]
        trace!(name: "increment_bytes_sent", bytes_send = number_of_bytes);

        self._bytes_sent += number_of_bytes as u128;
    }
    pub fn increment_bytes_received(&mut self, number_of_bytes: u32) {
        #[cfg(feature = "tracing")]
        trace!(name: "increment_bytes_received", bytes_received = number_of_bytes);

        self._bytes_received += number_of_bytes as u128;
    }
    #[allow(dead_code)]
    pub fn get_bytes_sent(&self) -> u128 {
        self._bytes_sent
    }
    #[allow(dead_code)]
    pub fn get_bytes_received(&self) -> u128 {
        self._bytes_received
    }
    #[allow(dead_code)]
    pub fn get_bytes_sent_total(&self) -> u128 {
        self._bytes_sent_total
    }
    #[allow(dead_code)]
    pub fn get_bytes_received_total(&self) -> u128 {
        self._bytes_received_total
    }

    #[cfg_attr(feature = "deep_tracing", tracing::instrument)]
    pub async fn send_data_packet(&mut self, mut header: packet::MultiLinkHeader, packet: Vec<u8>) {
        self.increment_bytes_sent(packet::MULTILINK_HEADER_SIZE as u32 + packet.len() as u32);
        header.set_link_id(self._link_id);

        self._send_data_channel
            .send((header, packet))
            .await
            .expect("Channel closed");
    }

    #[cfg_attr(feature = "deep_tracing", tracing::instrument)]
    pub async fn send_system_packet(
        &mut self,
        mut header: packet::MultiLinkHeader,
        packet: Vec<u8>,
    ) {
        self.increment_bytes_sent(packet::MULTILINK_HEADER_SIZE as u32 + packet.len() as u32);
        header.set_link_id(self._link_id);
        self._send_system_channel
            .send((header, packet))
            .await
            .expect("Channel closed");
    }
}

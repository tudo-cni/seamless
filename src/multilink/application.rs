use serde_derive::Deserialize;

use crate::multilink::scheduler;
use crate::multilink::transport;
use std::time::UNIX_EPOCH;

#[derive(Debug, Deserialize)]
pub struct ApplicationConfig {
    #[serde(rename = "$key$")]
    pub name: String,
    #[serde(flatten)]
    pub transport_information: transport::TransportInformation,
    #[serde(default = "reorder_default")]
    pub reorder: bool,
    pub scheduler: scheduler::SchedulerConfig,
}

fn reorder_default() -> bool {
    true
}

/// The `Application` struct represents an abstract higher layer service registered for special handling in the config-file.
#[derive(Debug)]
pub struct Application {
    /// Unique Name of the Application. e.g. RTSP, RTMP, HTTP, MyOwnProtocol, etc.
    pub name: String,
    /// Containing unique transport information about higher layer protocol. Used to match packets received by a higher layer to the correct scheduling, etc.
    pub transport: transport::TransportInformation,
    /// `Scheduler` used for this exact application. Should diverge from global scheduler.
    pub scheduler: scheduler::Scheduler,
    pub reorder: bool,
    /// Statistical field of bytes send to the remote host for this application in this second. Used to calculate the current application datarate.
    _bytes_sent: u128,
    /// Statistical field of bytes received from the remote host for this application in this second. Used to calculate the current application datarate.
    _bytes_received: u128,
    /// Statistical field of the total number of bytes send to the remote host for this application.
    _bytes_sent_total: u128,
    /// Statistical field of the total number of bytes received from the remote host for this application.
    _bytes_received_total: u128,
    /// Timestamp of last time the datarates were calculated. Used to check when next calculation is due.
    _last_datarate_calculation_timestamp: u128,
    /// Statistical field of the current incoming datarate from remote host for this application.
    _current_datarate_in: f64,
    /// Statistical field of the current outgoing datarate to remote host for this application.
    _current_datarate_out: f64,
    /// A unique id for the remote to identify the packet for the correct reordering queue. This is also part of the multilink protocol header.
    _stream_id: u8,
    /// The current packet id used for reordering on the remote host. This is also part of the multilink protocol header.
    _sequence: u32,
}

impl Application {
    /// Calculates the current in- and outgoing datarate of the application.
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

    pub fn new(
        name: String,
        trans: transport::TransportInformation,
        sched: scheduler::Scheduler,
        reordering: bool,
        stream_id: u8,
    ) -> Application {
        Application {
            name,
            transport: trans,
            scheduler: sched,
            reorder: reordering,
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
            _stream_id: stream_id,
            _sequence: 0,
        }
    }

    /// Incrementing the private bytes sent field by given number of bytes
    pub fn increment_bytes_sent(&mut self, number_of_bytes: u32) {
        self._bytes_sent += number_of_bytes as u128;
    }
    /// Incrementing the private bytes received field by given number of bytes
    #[allow(dead_code)]
    pub fn increment_bytes_received(&mut self, number_of_bytes: u32) {
        self._bytes_received += number_of_bytes as u128;
    }
    #[allow(dead_code)]
    /// Getter method for bytes_send field. Return is of type u128 in Bytes
    pub fn get_bytes_sent(&self) -> u128 {
        self._bytes_sent
    }
    /// Getter method for bytes_received field. Return is of type u128 in Bytes
    #[allow(dead_code)]
    pub fn get_bytes_received(&self) -> u128 {
        self._bytes_received
    }

    /// Getter method for current outgoing datarate field. Return is of type f64 in Bytes/s   
    pub fn get_datarate_out(&self) -> f64 {
        self._current_datarate_out
    }

    /// Getter method for current incoming datarate field. Return is of type f64 in Bytes/s
    pub fn get_datarate_in(&self) -> f64 {
        self._current_datarate_in
    }
    /// Generating the next sequence number and returning it
    pub fn get_next_sequence(&mut self) -> u32 {
        self._sequence += 1;
        self._sequence
    }

    /// Getter method for application stream id
    pub fn get_stream_id(&self) -> u8 {
        self._stream_id
    }
}

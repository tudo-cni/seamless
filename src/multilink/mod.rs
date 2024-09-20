mod application;
mod external_measurement;
mod link;
mod metric;
pub mod network_interface;
mod packet;
mod reorder;
mod scheduler;
mod stream;
mod technology;
mod transport;
mod types;

use anyhow::{bail, Context, Result};
use duration_string::DurationString;
use lz4_flex::{compress_prepend_size, decompress_size_prepended};
use serde::{Deserialize, Deserializer};
use serde_with::{serde_as, KeyValueMap};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};
use std::{fs, vec};
use tokio::sync::mpsc;
use tokio::sync::mpsc::{Sender, UnboundedSender};
use tokio::sync::Mutex;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tokio::time;

#[cfg(feature = "tracing")]
use tracing::{info, trace};

use self::scheduler::Scheduler;
use self::{packet::build_initiate_multilink_header, scheduler::LinkScheduling};
#[derive(Deserialize)]
pub struct Config {
    pub multilink: MultilinkConfig,
}

impl Config {
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        #[cfg(feature = "tracing")]
        info!("parsing multilink config from {:?}", path.as_ref());

        let data = fs::read_to_string(path)?;
        let mut config: Config = serde_yaml::from_str(&data)?;

        if config.multilink.links.is_empty() {
            bail!("No links given");
        }

        if let Some(ref mut external_measurement) = config.multilink.external_measurement {
            config.multilink.links.iter().try_for_each(|link| {
                if let Some(link_measurement) = &link.external_measurement {
                    if external_measurement
                        .technologies
                        .get(link_measurement)
                        .context("technology not found")?
                        .technology
                        != link.technology
                    {
                        bail!("Mismatch in External Measurement and Link Technology")
                    } else {
                        external_measurement
                            .technologies
                            .get_mut(link_measurement)
                            .context("technology not found")?
                            .links
                            .push(link.name.clone());
                    }
                }
                Ok(())
            })?;
        }

        Ok(config)
    }
}

#[derive(Debug, Deserialize, Clone, Copy)]
pub enum UserGroup {
    Privileged,
    Regular { uid: u32, gid: u32 },
}

#[serde_as]
#[derive(Debug, Deserialize)]
pub struct MultilinkConfig {
    pub hostname: String,
    pub user: UserGroup,
    pub tunnel: network_interface::InterfaceConfig,
    pub external_measurement: Option<external_measurement::ExternalMeasurementConfig>,
    #[serde_as(as = "KeyValueMap<_>")]
    pub links: Vec<link::LinkConfig>,
    #[serde_as(as = "KeyValueMap<_>")]
    #[serde(default)]
    pub applications: Vec<application::ApplicationConfig>,
    #[serde_as(as = "KeyValueMap<_>")]
    #[serde(default)]
    pub metrics: Vec<metric::MetricConfig>,
    pub global_scheduler: scheduler::SchedulerConfig,
    #[serde(
        default,
        deserialize_with = "crate::multilink::deserialize_optional_duration"
    )]
    pub monitoring_interval: Option<Duration>,
    #[serde(
        default,
        deserialize_with = "crate::multilink::deserialize_optional_duration"
    )]
    pub initial_reorder_timeout: Option<Duration>,
    pub reorder_buffer_size: Option<u32>,
}

#[allow(dead_code)]
fn deserialize_duration<'de, D>(deserializer: D) -> Result<Duration, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(
        DurationString::from_string(String::deserialize(deserializer)?)
            .map_err(|_| serde::de::Error::custom("not a valid duration string"))?
            .into(),
    )
}

#[allow(dead_code)]
pub(crate) fn deserialize_optional_duration<'de, D>(
    deserializer: D,
) -> Result<Option<Duration>, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(Some(
        DurationString::from_string(String::deserialize(deserializer)?)
            .map_err(|_| serde::de::Error::custom("not a valid duration string"))?
            .into(),
    ))
}

#[derive(Debug)]
pub struct Multilink {
    _configuration: MultilinkConfig,
    _tasks: Vec<JoinHandle<()>>,
}

pub struct UnboundChannel(pub Sender<Vec<u8>>, pub mpsc::UnboundedReceiver<Vec<u8>>);

impl Multilink {
    pub fn new(parameter: MultilinkConfig) -> Multilink {
        // Define the Binary Encoding and Decoding Configuration, so that bit-fields are not missinterpreted (Shouldn't diff between both clients)
        Multilink {
            _configuration: parameter,
            _tasks: vec![],
        }
    }

    #[cfg_attr(feature = "deep_tracing", tracing::instrument)]
    pub async fn start(&mut self) -> Result<UnboundChannel> {
        let (channel_leaving_links_tx, channel_leaving_links_rx): (
            types::MultiLinkTxQueue,
            types::MultiLinkRxQueue,
        ) = mpsc::unbounded_channel::<(String, Vec<u8>)>();

        let (channel_entering_multilink_tx, channel_entering_multilink_rx) =
            mpsc::channel::<Vec<u8>>(1000);

        let (channel_leaving_multilink_tx, channel_leaving_multilink_rx): (
            mpsc::UnboundedSender<Vec<u8>>,
            mpsc::UnboundedReceiver<Vec<u8>>,
        ) = mpsc::unbounded_channel::<Vec<u8>>();

        let links: Arc<types::LinkHashMap> =
            Arc::new(self._generate_links(channel_leaving_links_tx).await);
        let applications = Arc::new(self._generate_applications(links.clone())?);

        let links_arc_receiving = Arc::clone(&links);
        let links_arc_check = Arc::clone(&links);
        let links_arc_statistics = Arc::clone(&links);
        let links_arc_zmq = Arc::clone(&links);
        let global_scheduler = Arc::new(RwLock::new(scheduler::Scheduler::new(
            &self._configuration.global_scheduler,
            links.clone(),
        )?));
        let global_scheduler_arc_statistics = Arc::clone(&global_scheduler);

        let applications_arc_statistics = applications.clone();

        for link_name in links.keys() {
            let mut link_lock = links
                .get(link_name)
                .context("link not found")?
                .write()
                .await;
            let header = build_initiate_multilink_header();
            link_lock.send_system_packet(header, vec![]).await;
            drop(link_lock);
        }

        #[cfg(feature = "tracing")]
        info!("starting multilink_sending_packet_task task");

        self._tasks.push(tokio::spawn(async move {
            multilink_sending_packet_task(
                channel_entering_multilink_rx,
                links,
                applications,
                global_scheduler,
            )
            .await
            .expect("multilink_sending_packet_task failed");
        }));
        let stream_reordering_timeout = self._configuration.initial_reorder_timeout;
        let stream_reorder_size = self._configuration.reorder_buffer_size;
        self._tasks.push(tokio::spawn(async move {
            multilink_receiving_packet_task(
                channel_leaving_multilink_tx,
                channel_leaving_links_rx,
                links_arc_receiving,
                stream_reorder_size,
                stream_reordering_timeout,
            )
            .await
            .expect("multilink_receiving_packet_task");
        }));

        let hostname = self._configuration.hostname.clone();
        if let Some(external_measurement) = &self._configuration.external_measurement {
            let zmq_conf = external_measurement.clone();
            #[cfg(feature = "tracing")]
            info!("starting zmq_sub task: {:?}", zmq_conf);
            self._tasks.push(tokio::spawn(async move {
                external_measurement::zmq_sub(zmq_conf, links_arc_zmq, hostname)
                    .await
                    .expect("zmq_sub failed");
            }));
        };

        if !self._configuration.metrics.is_empty() {
            let metric_arc = Arc::new(metric::Metric::new(&self._configuration.metrics));

            #[cfg(feature = "tracing")]
            info!(
                "starting multilink_statistics task: {:?}",
                self._configuration.metrics
            );

            self._tasks.push(tokio::spawn(async move {
                multilink_statistics(
                    metric_arc,
                    links_arc_statistics,
                    applications_arc_statistics,
                    global_scheduler_arc_statistics,
                )
                .await
                .expect("multilink_statistics failed");
            }));
        }

        let monitoring_interval = self
            ._configuration
            .monitoring_interval
            .unwrap_or(time::Duration::from_millis(200));

        #[cfg(feature = "tracing")]
        info!(
            "starting multilink_check_links_interval task with monitoring_interval={:?}",
            monitoring_interval
        );

        self._tasks.push(tokio::spawn(async move {
            multilink_check_links_interval(links_arc_check, monitoring_interval)
                .await
                .expect("multilink_check_links_interval failed");
        }));

        Ok(UnboundChannel(
            channel_entering_multilink_tx,
            channel_leaving_multilink_rx,
        ))
    }

    pub async fn run(self) {
        futures::future::join_all(self._tasks).await;
    }
    // Generating a list of applications based on a `Vec` of read in yaml application configs and a `LinkHashMap` containg all available links
    fn _generate_applications(
        &self,
        links: Arc<types::LinkHashMap>,
    ) -> Result<Vec<tokio::sync::Mutex<application::Application>>> {
        let mut apps: Vec<tokio::sync::Mutex<application::Application>> = vec![];
        for (i, app) in self._configuration.applications.iter().enumerate() {
            let sched = scheduler::Scheduler::new(&app.scheduler, links.clone())?;
            let _app = tokio::sync::Mutex::new(application::Application::new(
                app.name.clone(),
                app.transport_information,
                sched,
                app.reorder,
                (i as u8) + 1,
            ));
            apps.push(_app);
        }
        Ok(apps)
    }

    async fn _generate_links(&self, tx_multilink: types::MultiLinkTxQueue) -> types::LinkHashMap {
        let mut links: types::LinkHashMap = HashMap::new();
        for (i, link_config) in self._configuration.links.iter().enumerate() {
            let link = link::Link::new(link_config, tx_multilink.clone(), (i as u8) + 1).await;
            links.insert(link_config.name.clone(), link);
        }
        links
    }
}

#[cfg_attr(feature = "deep_tracing", tracing::instrument)]
async fn multilink_sending_packet_task(
    mut rx_tunnel: types::RxQueue,
    links: Arc<types::LinkHashMap>,
    applications: Arc<Vec<tokio::sync::Mutex<application::Application>>>,
    global_scheduler: Arc<RwLock<scheduler::Scheduler>>,
) -> Result<()> {
    let compress: bool = false;
    let mut default_sequence = 0;

    while let Some(bytes) = rx_tunnel.recv().await {
        let timestamp_now = std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .as_millis();
        //Before crafting the header, we need to know which link we will use. tl;dr: Call Scheduler.

        let packet_transport = transport::parse_transport_header(&bytes);
        let mut chosen_links = HashSet::new();
        let mut sequence = 0;
        let mut stream_id = 0;
        let mut matched_transport = false;
        let mut reorder = true;
        // Check for known transport
        for app in applications.iter() {
            let mut app_lock = app.lock().await;
            if app_lock.transport.compare(&packet_transport) {
                chosen_links = app_lock.scheduler.schedule().await;
                sequence = app_lock.get_next_sequence();
                stream_id = app_lock.get_stream_id();
                matched_transport = true;
                reorder = app_lock.reorder;
                app_lock.increment_bytes_sent(bytes.len() as u32)
            }
            drop(app_lock);
        }

        if !matched_transport {
            chosen_links = global_scheduler.write().await.schedule().await;
            default_sequence += 1;
            sequence = default_sequence;
        }
        for link in chosen_links.iter() {
            let packet = if compress {
                compress_prepend_size(&bytes)
            } else {
                bytes.clone()
            };

            let pkt_type = if chosen_links.len() > 1 {
                packet::PacketType::DataDup
            } else {
                packet::PacketType::Data
            };

            let link = links
                .get(link)
                .context("Couldn't get link. Something broke.")?;
            let mut timestamp_reply = 0;
            let mut calc_rtt = false;

            let mut link_lock = link.write().await;
            if let Some(reply_time) = link_lock.get_rtt_calculation_due(timestamp_now) {
                timestamp_reply = reply_time;
                calc_rtt = true;
            }

            let header = packet::build_multilink_header(
                pkt_type,
                stream_id,
                sequence,
                reorder,
                bytes.len() as u16,
                timestamp_now as u16,
                calc_rtt,
                timestamp_reply,
            );

            #[cfg(feature = "tracing")]
            trace!("built multilink header: {:?}", header);

            link_lock.send_data_packet(header, packet).await;
            drop(link_lock);
        }
    }
    Ok(())
}

async fn multilink_receiving_packet_task(
    tx_tunnel: UnboundedSender<Vec<u8>>,
    mut rx_multilink: types::MultiLinkRxQueue,
    links: Arc<types::LinkHashMap>,
    stream_reorder_size: Option<u32>,
    stream_timeout: Option<time::Duration>,
) -> Result<()> {
    let compress: bool = false;
    let mut streams: HashMap<u8, Mutex<stream::Stream>> = HashMap::new();

    while let Some((link, bytes)) = rx_multilink.recv().await {
        let timestamp_now = std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .as_millis();

        let decoded = packet::MultiLinkHeader::from_bytes(
            bytes[..packet::MULTILINK_HEADER_SIZE].try_into().unwrap(),
        );

        if let packet::PacketType::Initiate = decoded.pkt_type() {
            let mut received_link = links
                .get(&link)
                .context("Couldn't get receiving Link from Link Map")?
                .write()
                .await;
            received_link.reset();
            continue;
        }

        if decoded.reorder() {
            streams.entry(decoded.stream_id()).or_insert_with(|| {
                Mutex::new(stream::Stream::new(
                    tx_tunnel.clone(),
                    stream_reorder_size,
                    stream_timeout,
                ))
            });
        }

        let packet = if compress {
            decompress_size_prepended(&bytes[packet::MULTILINK_HEADER_SIZE..])?
        } else {
            bytes[packet::MULTILINK_HEADER_SIZE..].to_vec()
        };

        let mut received_link = links
            .get(&link)
            .context("Couldn't get receiving Link from Link Map")?
            .write()
            .await;

        received_link.update_timestamps(decoded.timestamp(), timestamp_now as u16);
        received_link.update_remote_link_id(decoded.link_id());

        if decoded.srrt_calculation() {
            received_link.update_rtt(decoded.timestamp_reply());
        }

        received_link.update_loss(decoded.link_seq(), 128);
        //println!("{:?}", received_link.get_loss(128));
        received_link.increment_bytes_received(packet.len() as u32);
        if let packet::PacketType::Keepalive = decoded.pkt_type() {
            received_link.update_last_keepalive_received(timestamp_now);
            if received_link.get_next_keepalive_due(timestamp_now) {
                let mut timestamp_reply = 0;
                let mut calc_rtt = false;
                if let Some(reply_time) = received_link.get_rtt_calculation_due(timestamp_now) {
                    timestamp_reply = reply_time;
                    calc_rtt = true;
                }
                let header = packet::build_keepalive_multilink_header(
                    timestamp_now as u16,
                    calc_rtt,
                    timestamp_reply,
                    0,
                );
                //println!("{:?}", encoded);
                received_link.send_system_packet(header, vec![]).await;
                received_link.update_last_keepalive_sent(timestamp_now);
            }
        }
        drop(received_link);
        if let packet::PacketType::Data | packet::PacketType::DataDup = decoded.pkt_type() {
            let used_links: Vec<String>;
            {
                let mut stream = streams
                    .get(&decoded.stream_id())
                    .context("Couldn't get correct stream")?
                    .lock()
                    .await;
                stream.insert_used_link(link);

                used_links = stream.get_used_links();
                drop(stream);
            }
            let mut highest_rtt: f32 = 0.0;
            for used_link_name in used_links {
                let rtt = links
                    .get(&used_link_name)
                    .context("link not found")?
                    .read()
                    .await
                    .get_round_trip_time()
                    .unwrap_or(0.0);
                if rtt > highest_rtt && rtt != f32::MAX {
                    highest_rtt = rtt;
                }
            }

            let mut stream = streams
                .get(&decoded.stream_id())
                .context("Couldn't get correct stream")?
                .lock()
                .await;
            stream.set_reorder_timeout(highest_rtt as u64);
            if let packet::PacketType::Data = decoded.pkt_type() {
                stream
                    .handle_rx(decoded.stream_seq(), packet, false)
                    .await?;
            } else {
                stream.handle_rx(decoded.stream_seq(), packet, true).await?;
            }
        }
    }
    Ok(())
}

async fn multilink_check_links_interval(
    links: Arc<types::LinkHashMap>,
    interval: time::Duration,
) -> Result<()> {
    let mut interval = time::interval(interval);
    interval.tick().await;
    loop {
        interval.tick().await;
        let timestamp_now = std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .as_millis();
        for key in links.keys() {
            let mut link_to_check = links
                .get(key)
                .context("Couldn't get receiving Link from Link Map")?
                .write()
                .await;

            if link_to_check.get_next_keepalive_due(timestamp_now) {
                let mut timestamp_reply = 0;
                let mut calc_rtt = false;
                if let Some(reply_time) = link_to_check.get_rtt_calculation_due(timestamp_now) {
                    timestamp_reply = reply_time;
                    calc_rtt = true;
                }
                let header = packet::build_keepalive_multilink_header(
                    timestamp_now as u16,
                    calc_rtt,
                    timestamp_reply,
                    0,
                );
                link_to_check.send_system_packet(header, vec![]).await;
                link_to_check.update_last_keepalive_sent(timestamp_now);
            }
            link_to_check.get_link_timeout(timestamp_now);
            link_to_check.check_high_latency();
            link_to_check.check_high_loss();
        }
    }
}

async fn multilink_statistics(
    metric: Arc<metric::Metric>,
    links: Arc<types::LinkHashMap>,
    applications: Arc<Vec<tokio::sync::Mutex<application::Application>>>,
    global_scheduler: Arc<RwLock<Scheduler>>,
) -> Result<()> {
    let mut interval = time::interval(metric.reporting_interval);
    interval.tick().await;
    loop {
        interval.tick().await;

        let mut report_datarate = false;
        let mut report_round_trip_time = false;
        let mut report_link_state = false;
        let mut report_link_loss = false;
        let mut report_megabytes_sent_total = false;
        let mut report_megabytes_received_total = false;
        let mut report_chosen_links = false;

        if let Some(metric_reports) = &metric.metrics_to_report {
            for metric_to_report in metric_reports {
                match metric_to_report {
                    metric::MetricsToReportConfig::Datarate => report_datarate = true,
                    metric::MetricsToReportConfig::RoundTripTime => report_round_trip_time = true,
                    metric::MetricsToReportConfig::LinkState => report_link_state = true,
                    metric::MetricsToReportConfig::LinkLoss => report_link_loss = true,
                    metric::MetricsToReportConfig::MegabytesSentTotal => {
                        report_megabytes_sent_total = true
                    }
                    metric::MetricsToReportConfig::MegabytesReceivedTotal => {
                        report_megabytes_received_total = true
                    }
                    metric::MetricsToReportConfig::ChosenLinks => report_chosen_links = true,
                }
            }
        }

        for key in links.keys() {
            let link_datarate_out;
            let link_datarate_in;
            let link_round_trip_time;
            let link_state;
            let link_loss;
            let link_bytes_sent_total;
            let link_bytes_received_total;
            {
                let mut link_to_check = links
                    .get(key)
                    .context("Couldn't get receiving Link from Link Map")?
                    .write()
                    .await;
                link_to_check.calculate_datarates();
                link_datarate_out = link_to_check.get_datarate_out();
                link_datarate_in = link_to_check.get_datarate_in();
                link_bytes_sent_total = link_to_check.get_bytes_sent_total();
                link_bytes_received_total = link_to_check.get_bytes_received_total();
                link_round_trip_time = link_to_check.get_round_trip_time().unwrap_or(0.0);
                link_state = link_to_check.get_link_state();
                link_loss = link_to_check.get_loss(128);
                drop(link_to_check);
            }
            if report_datarate {
                metric
                    .report_datarate(
                        metric::DatarateType::Link(key.clone()),
                        link_datarate_out,
                        metric::DataDirection::Out,
                    )
                    .await?;
                metric
                    .report_datarate(
                        metric::DatarateType::Link(key.clone()),
                        link_datarate_in,
                        metric::DataDirection::In,
                    )
                    .await?;
            }
            if report_round_trip_time {
                metric
                    .report_round_trip_time(key.clone(), link_round_trip_time.into())
                    .await?;
            }
            if report_link_state {
                metric
                    .report_link_state(key.clone(), link_state.clone())
                    .await?;
            }
            if report_link_loss {
                metric.report_link_loss(key.clone(), link_loss).await?;
            }
            if report_megabytes_sent_total {
                metric
                    .report_megabytes_sent_total(key.clone(), link_bytes_sent_total)
                    .await?;
            }
            if report_megabytes_received_total {
                metric
                    .report_megabytes_received_total(key.clone(), link_bytes_received_total)
                    .await?;
            }
        }

        let _scheduler = global_scheduler.read().await;
        if report_chosen_links {
            metric
                .report_link_decision(
                    String::from("global"),
                    _scheduler.to_string(),
                    _scheduler.get_last_chosen_links(),
                )
                .await?;
        }
        drop(_scheduler);

        for application in applications.iter() {
            let app_name;
            let app_datarate_out;
            let app_datarate_in;
            let app_scheduler: String;
            let app_links: HashSet<String>;
            {
                let mut app = application.lock().await;
                app.calculate_datarates();
                app_name = app.name.clone();
                app_datarate_out = app.get_datarate_out();
                app_datarate_in = app.get_datarate_in();
                app_scheduler = app.scheduler.to_string();
                app_links = app.scheduler.get_last_chosen_links();
                drop(app);
            }
            if report_datarate {
                metric
                    .report_datarate(
                        metric::DatarateType::Application(app_name.clone()),
                        app_datarate_out,
                        metric::DataDirection::Out,
                    )
                    .await?;
                metric
                    .report_datarate(
                        metric::DatarateType::Application(app_name.clone()),
                        app_datarate_in,
                        metric::DataDirection::In,
                    )
                    .await?;
            }
            if report_chosen_links {
                metric
                    .report_link_decision(
                        app_name.clone(),
                        app_scheduler.clone(),
                        app_links.clone(),
                    )
                    .await?;
            }
        }
    }
}

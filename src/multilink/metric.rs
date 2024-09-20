use crate::multilink::link::LinkState;
use anyhow::Result;
use chrono::Utc;
use futures::prelude::stream;
use influxdb::{self, InfluxDbWriteable};
use itertools::Itertools;
use serde_derive::Deserialize;
use std::collections::HashSet;
use tokio::time::Duration;

#[derive(Debug, Clone, Deserialize)]
pub struct Influxdb1Config {
    pub host: String,
    pub port: u16,
    pub db: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Influxdb2Config {
    pub host: String,
    pub port: u16,
    pub org: String,
    pub token: String,
    pub bucket: String,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "backend")]
pub enum ReportingConfig {
    Influxdb1(Influxdb1Config),
    Influxdb2(Influxdb2Config),
}

#[derive(Debug, Clone, Deserialize)]
pub enum MetricsToReportConfig {
    Datarate,
    RoundTripTime,
    LinkState,
    LinkLoss,
    MegabytesSentTotal,
    MegabytesReceivedTotal,
    ChosenLinks,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct MetricConfig {
    #[serde(rename = "$key$")]
    pub name: String,
    pub report_to: ReportingConfig,
    pub metrics_to_report: Option<Vec<MetricsToReportConfig>>,
    #[serde(
        default,
        deserialize_with = "crate::multilink::deserialize_optional_duration"
    )]
    pub reporting_interval: Option<Duration>,
}

#[allow(dead_code)]
pub enum DatarateType {
    Link(String),
    Multilink(String),
    Application(String),
}

#[allow(dead_code)]
pub struct Metric {
    influxdb2_parameter: Option<Influxdb2Config>,
    influxdb2_client: Option<influxdb2::Client>,
    influxdb1_parameter: Option<Influxdb1Config>,
    influxdb1_client: Option<influxdb::Client>,
    pub reporting_interval: Duration,
    pub metrics_to_report: Option<Vec<MetricsToReportConfig>>,
}

pub enum DataDirection {
    In,
    Out,
}

impl Metric {
    pub fn new(conf_metrics: &[MetricConfig]) -> Metric {
        // Has to be done nicer...
        let mut influxdb2_parameter = None;
        let mut influxdb2_client = None;
        let mut influxdb1_parameter = None;
        let mut influxdb1_client = None;
        let mut metrics_to_report = None;

        for conf_metric in conf_metrics {
            match &conf_metric.report_to {
                ReportingConfig::Influxdb2(para) => {
                    influxdb2_client = Some(influxdb2::Client::new(
                        format!("http://{}:{}", para.host, para.port),
                        para.org.clone(),
                        para.token.clone(),
                    ));
                    influxdb2_parameter = Some(para.clone());
                }
                ReportingConfig::Influxdb1(para) => {
                    influxdb1_client = Some(
                        influxdb::Client::new(
                            format!("http://{}:{}", para.host, para.port),
                            para.db.clone(),
                        )
                        .with_auth("root", "root"),
                    );
                    influxdb1_parameter = Some(para.clone());
                }
            }
            match &conf_metric.metrics_to_report {
                Some(_) => {
                    metrics_to_report.clone_from(&conf_metric.metrics_to_report);
                }
                None => {}
            }
        }

        Metric {
            influxdb2_parameter,
            influxdb2_client,
            influxdb1_parameter,
            influxdb1_client,
            reporting_interval: conf_metrics[0]
                .reporting_interval
                .unwrap_or(Duration::from_millis(1000)),
            metrics_to_report,
        }
    }

    pub async fn report_datarate(
        &self,
        datarate_type: DatarateType,
        datarate: f64,
        direction: DataDirection,
    ) -> Result<()> {
        let str_direction = match direction {
            DataDirection::In => "datarate_in",
            DataDirection::Out => "datarate_out",
        };
        match &self.influxdb2_parameter {
            Some(_) => {
                let point = match &datarate_type {
                    DatarateType::Link(name) => influxdb2::models::DataPoint::builder("link")
                        .tag("name", name.clone())
                        .field(str_direction, datarate)
                        .build()?,
                    DatarateType::Multilink(name) => {
                        influxdb2::models::DataPoint::builder("multilink")
                            .tag("name", name.clone())
                            .field(str_direction, datarate)
                            .build()?
                    }
                    DatarateType::Application(name) => {
                        influxdb2::models::DataPoint::builder("application")
                            .tag("name", name.clone())
                            .field(str_direction, datarate)
                            .build()?
                    }
                };
                self.write_to_influxdb2(vec![point]).await?;
            }
            None => {}
        }
        match &self.influxdb1_parameter {
            Some(_) => {
                let nanoseconds = Utc::now()
                    .timestamp_nanos_opt()
                    .map_or_else(|| 0, |n| n as u128);
                let point = match datarate_type {
                    DatarateType::Link(name) => influxdb::Timestamp::Nanoseconds(nanoseconds)
                        .into_query("multilink")
                        .add_tag("type", "link")
                        .add_tag("name", name)
                        .add_field(str_direction, datarate),
                    DatarateType::Multilink(name) => influxdb::Timestamp::Nanoseconds(nanoseconds)
                        .into_query("multilink")
                        .add_tag("type", "multilink")
                        .add_tag("name", name)
                        .add_field(str_direction, datarate),
                    DatarateType::Application(name) => {
                        influxdb::Timestamp::Nanoseconds(nanoseconds)
                            .into_query("multilink")
                            .add_tag("type", "application")
                            .add_tag("name", name)
                            .add_field(str_direction, datarate)
                    }
                };
                self.write_to_influxdb1(vec![point]).await;
            }
            None => {}
        }
        Ok(())
    }

    pub async fn report_round_trip_time(
        &self,
        link_name: String,
        round_trip_time: f64,
    ) -> Result<()> {
        let nanoseconds = Utc::now()
            .timestamp_nanos_opt()
            .map_or_else(|| 0, |n| n as u128);
        match &self.influxdb2_parameter {
            Some(_) => {
                let point = influxdb2::models::DataPoint::builder("link")
                    .tag("name", link_name.clone())
                    .field("round_trip_time", round_trip_time)
                    .build()?;
                self.write_to_influxdb2(vec![point]).await?;
            }
            None => {}
        }
        match &self.influxdb1_parameter {
            Some(_) => {
                let point = influxdb::Timestamp::Nanoseconds(nanoseconds)
                    .into_query("multilink")
                    .add_tag("type", "link")
                    .add_tag("name", link_name)
                    .add_field("round_trip_time", round_trip_time);
                self.write_to_influxdb1(vec![point]).await;
            }
            None => {}
        }

        Ok(())
    }

    pub async fn report_link_state(&self, link_name: String, link_state: LinkState) -> Result<()> {
        let nanoseconds = Utc::now()
            .timestamp_nanos_opt()
            .map_or_else(|| 0, |n| n as u128);
        match &self.influxdb2_parameter {
            Some(_) => {
                let point = influxdb2::models::DataPoint::builder("link")
                    .tag("name", link_name.clone())
                    .field("link_state", link_state.to_string())
                    .build()?;
                self.write_to_influxdb2(vec![point]).await?;
            }
            None => {}
        }
        match &self.influxdb1_parameter {
            Some(_) => {
                let point = influxdb::Timestamp::Nanoseconds(nanoseconds)
                    .into_query("multilink")
                    .add_tag("type", "link")
                    .add_tag("name", link_name)
                    .add_field("link_state", link_state.to_string());
                self.write_to_influxdb1(vec![point]).await;
            }
            None => {}
        }

        Ok(())
    }

    pub async fn report_link_decision(
        &self,
        application_name: String,
        scheduler: String,
        links: HashSet<String>,
    ) -> Result<()> {
        let nanoseconds = Utc::now()
            .timestamp_nanos_opt()
            .map_or_else(|| 0, |n| n as u128);
        match &self.influxdb2_parameter {
            Some(_) => {
                let point = influxdb2::models::DataPoint::builder("application")
                    .tag("name", application_name.clone())
                    .field("scheduler", scheduler.to_string())
                    .field("links", links.iter().join(", "))
                    .build()?;
                self.write_to_influxdb2(vec![point]).await?;
            }
            None => {}
        }
        match &self.influxdb1_parameter {
            Some(_) => {
                let point = influxdb::Timestamp::Nanoseconds(nanoseconds)
                    .into_query("multilink")
                    .add_tag("type", "application")
                    .add_tag("name", application_name)
                    .add_field("scheduler", scheduler.to_string())
                    .add_field("links", links.iter().join(", "));
                self.write_to_influxdb1(vec![point]).await;
            }
            None => {}
        }

        Ok(())
    }

    pub async fn report_link_loss(&self, link_name: String, link_loss: f32) -> Result<()> {
        let link_loss_f64: f64 = link_loss as f64;
        let nanoseconds = Utc::now()
            .timestamp_nanos_opt()
            .map_or_else(|| 0, |n| n as u128);
        match &self.influxdb2_parameter {
            Some(_) => {
                let point = influxdb2::models::DataPoint::builder("link")
                    .tag("name", link_name.clone())
                    .field("link_loss", link_loss_f64)
                    .build()?;
                self.write_to_influxdb2(vec![point]).await?;
            }
            None => {}
        }
        match &self.influxdb1_parameter {
            Some(_) => {
                let point = influxdb::Timestamp::Nanoseconds(nanoseconds)
                    .into_query("multilink")
                    .add_tag("type", "link")
                    .add_tag("name", link_name)
                    .add_field("link_loss", link_loss_f64);
                self.write_to_influxdb1(vec![point]).await;
            }
            None => {}
        }

        Ok(())
    }

    pub async fn report_megabytes_sent_total(
        &self,
        link_name: String,
        link_bytes_sent_total: u128,
    ) -> Result<()> {
        let megabytes_sent_total: f64 = link_bytes_sent_total as f64 / 1000000.0;
        let nanoseconds = Utc::now()
            .timestamp_nanos_opt()
            .map_or_else(|| 0, |n| n as u128);
        match &self.influxdb2_parameter {
            Some(_) => {
                let point = influxdb2::models::DataPoint::builder("link")
                    .tag("name", link_name.clone())
                    .field("megabytes_sent_total", megabytes_sent_total)
                    .build()?;
                self.write_to_influxdb2(vec![point]).await?;
            }
            None => {}
        }
        match &self.influxdb1_parameter {
            Some(_) => {
                let point = influxdb::Timestamp::Nanoseconds(nanoseconds)
                    .into_query("multilink")
                    .add_tag("type", "link")
                    .add_tag("name", link_name)
                    .add_field("megabytes_sent_total", megabytes_sent_total);
                self.write_to_influxdb1(vec![point]).await;
            }
            None => {}
        }

        Ok(())
    }

    pub async fn report_megabytes_received_total(
        &self,
        link_name: String,
        link_bytes_received_total: u128,
    ) -> Result<()> {
        let megabytes_received_total: f64 = link_bytes_received_total as f64 / 1000000.0;
        let nanoseconds = Utc::now()
            .timestamp_nanos_opt()
            .map_or_else(|| 0, |n| n as u128);
        match &self.influxdb2_parameter {
            Some(_) => {
                let point = influxdb2::models::DataPoint::builder("link")
                    .tag("name", link_name.clone())
                    .field("megabytes_received_total", megabytes_received_total)
                    .build()?;
                self.write_to_influxdb2(vec![point]).await?;
            }
            None => {}
        }
        match &self.influxdb1_parameter {
            Some(_) => {
                let point = influxdb::Timestamp::Nanoseconds(nanoseconds)
                    .into_query("multilink")
                    .add_tag("type", "link")
                    .add_tag("name", link_name)
                    .add_field("megabytes_received_total", megabytes_received_total);
                self.write_to_influxdb1(vec![point]).await;
            }
            None => {}
        }

        Ok(())
    }

    pub async fn write_to_influxdb1(&self, points: Vec<influxdb::WriteQuery>) {
        match &self.influxdb1_client {
            Some(client) => {
                client
                    .query(points)
                    .await
                    .expect("Couldn't report to influxdb1");
            }
            None => {}
        }
    }

    pub async fn write_to_influxdb2(
        &self,
        points: Vec<influxdb2::models::DataPoint>,
    ) -> Result<()> {
        if let Some(client) = &self.influxdb2_client {
            if let Some(parameter) = &self.influxdb2_parameter {
                client
                    .write(&parameter.bucket, stream::iter(points))
                    .await?;
            }
        }

        Ok(())
    }
}

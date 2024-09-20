use crate::multilink::types::LinkHashMap;
use std::collections::HashSet;
use std::fmt;
use std::sync::Arc;

pub mod scheduler_bestquality;
pub mod scheduler_lowestrtt;
pub mod scheduler_priority_quality;
pub mod scheduler_redundant;
pub mod scheduler_roundrobin;
pub mod scheduler_singlelink;
mod smooth_transition;
mod time_to_trigger;
use anyhow::{bail, Result};
use async_trait::async_trait;
use serde_derive::Deserialize;

#[async_trait]
pub trait LinkScheduling {
    async fn schedule(&mut self) -> HashSet<String>;
}

#[derive(Debug)]
pub enum Scheduler {
    RoundRobin(scheduler_roundrobin::RoundRobinScheduler),
    SingleLink(scheduler_singlelink::SingleLinkScheduler),
    LowestRtt(scheduler_lowestrtt::LowestRttScheduler),
    Redundant(scheduler_redundant::RedundantScheduler),
    BestQuality(scheduler_bestquality::BestQualityScheduler),
    PriorityQuality(scheduler_priority_quality::PriorityQualityScheduler),
}

impl fmt::Display for Scheduler {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Scheduler::RoundRobin(_) => write!(f, "RoundRobin"),
            Scheduler::SingleLink(_) => write!(f, "SingleLink"),
            Scheduler::LowestRtt(_) => write!(f, "LowestRtt"),
            Scheduler::Redundant(_) => write!(f, "Redundant"),
            Scheduler::BestQuality(_) => write!(f, "BestQuality"),
            Scheduler::PriorityQuality(_) => write!(f, "PriorityQuality"),
        }
    }
}

impl Scheduler {
    pub fn new(scheduler: &SchedulerConfig, links: Arc<LinkHashMap>) -> Result<Self> {
        // Check link existence for LowestRtt, BestQuality, and Redundant
        let check_links_existence = |links_to_use: &Vec<String>| {
            for link_name in links_to_use.iter() {
                if !links.contains_key(link_name) {
                    bail!("{:?} doesn't exist", link_name);
                }
            }
            Ok(())
        };
        match scheduler {
            SchedulerConfig::RoundRobin | SchedulerConfig::WeightedRoundRobin(_) => Ok(
                Scheduler::RoundRobin(scheduler_roundrobin::RoundRobinScheduler::new(links)),
            ),
            SchedulerConfig::SingleLink(config) => {
                // Check link existence for SingleLinkScheduler
                if !links.contains_key(&config.link) {
                    bail!("{:?} doesn't exist", config.link);
                }

                Ok(Scheduler::SingleLink(
                    scheduler_singlelink::SingleLinkScheduler::new(config.link.clone(), links),
                ))
            }
            SchedulerConfig::LowestRtt(config) => {
                check_links_existence(&config.links)?;
                Ok(Scheduler::LowestRtt(
                    scheduler_lowestrtt::LowestRttScheduler::new(
                        config.links.clone(),
                        links,
                        config.time_to_trigger,
                        config.smooth_transition_time,
                    ),
                ))
            }
            SchedulerConfig::BestQuality(config) => {
                check_links_existence(&config.links)?;
                Ok(Scheduler::BestQuality(
                    scheduler_bestquality::BestQualityScheduler::new(
                        config.links.clone(),
                        links,
                        config.time_to_trigger,
                    ),
                ))
            }
            SchedulerConfig::Redundant(config) => {
                check_links_existence(&config.links)?;
                Ok(Scheduler::Redundant(
                    scheduler_redundant::RedundantScheduler::new(config.links.clone(), links),
                ))
            }
            SchedulerConfig::PriorityQuality(config) => {
                // Check link existence for PriorityQualityScheduler
                for link_config in config.links.iter() {
                    if !links.contains_key(&link_config.link_name) {
                        bail!("{:?} doesn't exist", link_config.link_name);
                    }
                }

                Ok(Scheduler::PriorityQuality(
                    scheduler_priority_quality::PriorityQualityScheduler::new(
                        config.links.clone(),
                        links,
                        config.time_to_trigger,
                    ),
                ))
            }
        }
    }
    pub fn get_last_chosen_links(&self) -> HashSet<String> {
        match self {
            Scheduler::RoundRobin(scheduler) => scheduler.get_last_chosen_links(),
            Scheduler::SingleLink(scheduler) => scheduler.get_last_chosen_links(),
            Scheduler::LowestRtt(scheduler) => scheduler.get_last_chosen_links(),
            Scheduler::Redundant(scheduler) => scheduler.get_last_chosen_links(),
            Scheduler::BestQuality(scheduler) => scheduler.get_last_chosen_links(),
            Scheduler::PriorityQuality(scheduler) => scheduler.get_last_chosen_links(),
        }
    }
}

#[async_trait]
impl LinkScheduling for Scheduler {
    async fn schedule(&mut self) -> HashSet<String> {
        match self {
            Scheduler::RoundRobin(scheduler) => scheduler.schedule().await,
            Scheduler::SingleLink(scheduler) => scheduler.schedule().await,
            Scheduler::LowestRtt(scheduler) => scheduler.schedule().await,
            Scheduler::Redundant(scheduler) => scheduler.schedule().await,
            Scheduler::BestQuality(scheduler) => scheduler.schedule().await,
            Scheduler::PriorityQuality(scheduler) => scheduler.schedule().await,
        }
    }
}
#[derive(Debug, Deserialize)]
pub struct WeightedRoundRobinConfig {}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum SchedulerConfig {
    RoundRobin,
    WeightedRoundRobin(WeightedRoundRobinConfig),
    SingleLink(scheduler_singlelink::SingleLinkSchedulerConfig),
    LowestRtt(scheduler_lowestrtt::LowestRttSchedulerConfig),
    Redundant(scheduler_redundant::RedundantSchedulerConfig),
    BestQuality(scheduler_bestquality::BestQualitySchedulerConfig),
    PriorityQuality(scheduler_priority_quality::PriorityQualitySchedulerConfig),
}

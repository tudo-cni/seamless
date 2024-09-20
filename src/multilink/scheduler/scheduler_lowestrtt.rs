use crate::multilink::{link::LinkState, types::LinkHashMap};
use serde_derive::Deserialize;
use std::{collections::HashMap, collections::HashSet, sync::Arc, time::Duration};
use tokio::time;

#[cfg(feature = "tracing")]
use tracing::trace;

use crate::multilink::scheduler::smooth_transition::SmoothTransition;
use crate::multilink::scheduler::time_to_trigger::SchedulerTimeToTrigger;
use crate::multilink::scheduler::LinkScheduling;

use async_trait::async_trait;

#[derive(Debug, Deserialize)]
pub struct LowestRttSchedulerConfig {
    pub links: Vec<String>,
    #[serde(
        default,
        deserialize_with = "crate::multilink::deserialize_optional_duration"
    )]
    pub time_to_trigger: Option<Duration>,
    #[serde(
        default,
        deserialize_with = "crate::multilink::deserialize_optional_duration"
    )]
    pub smooth_transition_time: Option<Duration>,
}
#[derive(Debug)]
pub struct LowestRttScheduler {
    _links: Arc<LinkHashMap>,
    _links_to_use: Vec<String>,
    _time_to_trigger: SchedulerTimeToTrigger,
    _smooth_transistion: SmoothTransition,
    _last_chosen_links: HashSet<String>,
}

impl LowestRttScheduler {
    pub fn new(
        links_to_use: Vec<String>,
        links: Arc<LinkHashMap>,
        time_to_trigger: Option<time::Duration>,
        smooth_transition_time: Option<time::Duration>,
    ) -> LowestRttScheduler {
        LowestRttScheduler {
            _links_to_use: links_to_use.clone(),
            _links: links,
            _time_to_trigger: SchedulerTimeToTrigger::new(links_to_use, time_to_trigger),
            _smooth_transistion: SmoothTransition::new(smooth_transition_time),
            _last_chosen_links: HashSet::new(),
        }
    }
    pub fn get_last_chosen_links(&self) -> HashSet<String> {
        self._last_chosen_links.clone()
    }
}

#[async_trait]
impl LinkScheduling for LowestRttScheduler {
    #[cfg_attr(feature = "deep_tracing", tracing::instrument)]
    async fn schedule(&mut self) -> HashSet<String> {
        let mut link_to_latency_map: HashMap<String, f32> = HashMap::new();
        for link_name in self._links_to_use.iter() {
            let link = self
                ._links
                .get(link_name)
                .expect("Link not in LinkMap")
                .read()
                .await;
            match link.get_link_state() {
                LinkState::Healthy => {
                    let value = match link.get_round_trip_time() {
                        // Negative rtt, because time_to_trigger considers higher == better, but we want lower == better
                        Some(rtt) => -rtt,
                        None => f32::MIN,
                    };
                    link_to_latency_map.insert(link_name.clone(), value);
                }
                LinkState::HighLoss => {
                    if link.get_seq_received() < 128 {
                        // TODO: Bad Practice
                        let value = match link.get_round_trip_time() {
                            // Negative rtt, because time_to_trigger considers higher == better, but we want lower == better
                            Some(rtt) => -rtt,
                            None => f32::MIN,
                        };
                        link_to_latency_map.insert(link_name.clone(), value);
                    } else {
                        link_to_latency_map.insert(link_name.clone(), f32::MIN);
                    }
                }
                _ => {
                    link_to_latency_map.insert(link_name.clone(), f32::MIN);
                }
            }
        }

        #[cfg(feature = "tracing")]
        trace!("link_to_latency_map: {:?}", link_to_latency_map);

        if !link_to_latency_map.is_empty() {
            self._time_to_trigger.update(link_to_latency_map);
        }
        let mut links = HashSet::new();
        links.insert(self._time_to_trigger.get_serving_link());
        links = self._smooth_transistion.smooth_out(links);
        self._last_chosen_links.clone_from(&links);
        links
    }
}

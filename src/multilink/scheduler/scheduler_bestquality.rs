use crate::multilink::scheduler::time_to_trigger::SchedulerTimeToTrigger;
use crate::multilink::{link::LinkState, types::LinkHashMap};
use serde_derive::Deserialize;
use std::{collections::HashMap, collections::HashSet, sync::Arc, time::Duration};

#[cfg(feature = "tracing")]
use tracing::trace;

use crate::multilink::scheduler::LinkScheduling;

use async_trait::async_trait;
#[derive(Debug, Deserialize)]
pub struct BestQualitySchedulerConfig {
    pub links: Vec<String>,
    #[serde(
        default,
        deserialize_with = "crate::multilink::deserialize_optional_duration"
    )]
    pub time_to_trigger: Option<Duration>,
}

#[derive(Debug)]
pub struct BestQualityScheduler {
    _links: Arc<LinkHashMap>,
    _links_to_use: Vec<String>,
    _time_to_trigger: SchedulerTimeToTrigger,
    _last_chosen_links: HashSet<String>,
}

impl BestQualityScheduler {
    pub fn new(
        links_to_use: Vec<String>,
        links: Arc<LinkHashMap>,
        time_to_trigger: Option<Duration>,
    ) -> BestQualityScheduler {
        BestQualityScheduler {
            _links_to_use: links_to_use.clone(),
            _links: links,
            _time_to_trigger: SchedulerTimeToTrigger::new(links_to_use, time_to_trigger),
            _last_chosen_links: HashSet::new(),
        }
    }
    pub fn get_last_chosen_links(&self) -> HashSet<String> {
        self._last_chosen_links.clone()
    }
}

#[async_trait]
impl LinkScheduling for BestQualityScheduler {
    #[cfg_attr(feature = "deep_tracing", tracing::instrument)]
    async fn schedule(&mut self) -> HashSet<String> {
        let mut link_to_quality_map: HashMap<String, f32> = HashMap::new();

        for link_name in self._links_to_use.iter() {
            let link = self
                ._links
                .get(link_name)
                .expect("Link not in LinkMap")
                .read()
                .await;
            match link.get_link_state() {
                LinkState::Healthy => {
                    let value = link.get_quality();
                    link_to_quality_map.insert(link_name.clone(), value);
                }
                LinkState::HighLoss => {
                    if link.get_seq_received() < 128 {
                        // TODO: Bad Practice
                        let value = link.get_quality();
                        link_to_quality_map.insert(link_name.clone(), value);
                    } else {
                        link_to_quality_map.insert(link_name.clone(), f32::MIN);
                    }
                }
                _ => {
                    link_to_quality_map.insert(link_name.clone(), f32::MIN);
                }
            }
        }

        #[cfg(feature = "tracing")]
        trace!("link_to_quality_map: {:?}", link_to_quality_map);

        if !link_to_quality_map.is_empty() {
            self._time_to_trigger.update(link_to_quality_map);
        }
        let mut links = HashSet::new();
        links.insert(self._time_to_trigger.get_serving_link());
        self._last_chosen_links.clone_from(&links);
        links
    }
}

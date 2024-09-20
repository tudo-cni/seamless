use crate::multilink::{
    link::LinkState, scheduler::time_to_trigger::SchedulerTimeToTrigger, types::LinkHashMap,
};
use serde_derive::Deserialize;
use std::{collections::HashMap, collections::HashSet, sync::Arc, time::Duration};
use tokio::time;

#[cfg(feature = "tracing")]
use tracing::trace;

use crate::multilink::scheduler::LinkScheduling;

use async_trait::async_trait;
#[derive(Debug, Deserialize)]
pub struct PriorityQualitySchedulerConfig {
    pub links: Vec<PriorityQualitySchedulerLinkConfig>,
    #[serde(
        default,
        deserialize_with = "crate::multilink::deserialize_optional_duration"
    )]
    pub time_to_trigger: Option<Duration>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PriorityQualitySchedulerLinkConfig {
    pub link_name: String,
    pub kpi: String,
    pub threshold: f32,
}

#[derive(Debug)]
pub struct PriorityQualityScheduler {
    _links: Arc<LinkHashMap>,
    _link_information: Vec<PriorityQualitySchedulerLinkConfig>,
    _time_to_trigger: SchedulerTimeToTrigger,
    _last_chosen_links: HashSet<String>,
}

impl PriorityQualityScheduler {
    pub fn new(
        links_to_use: Vec<PriorityQualitySchedulerLinkConfig>,
        links: Arc<LinkHashMap>,
        time_to_trigger: Option<time::Duration>,
    ) -> PriorityQualityScheduler {
        PriorityQualityScheduler {
            _link_information: links_to_use.clone(),
            _links: links,
            _time_to_trigger: SchedulerTimeToTrigger::new(
                links_to_use
                    .iter()
                    .map(|_link| _link.link_name.clone())
                    .collect::<Vec<String>>(),
                time_to_trigger,
            ),
            _last_chosen_links: HashSet::new(),
        }
    }
    pub fn get_last_chosen_links(&self) -> HashSet<String> {
        self._last_chosen_links.clone()
    }
}

#[async_trait]
impl LinkScheduling for PriorityQualityScheduler {
    #[cfg_attr(feature = "deep_tracing", tracing::instrument)]
    async fn schedule(&mut self) -> HashSet<String> {
        let mut link_to_quality_map: HashMap<String, f32> = HashMap::new();

        for (i, link_info) in self._link_information.iter().enumerate() {
            let link = self
                ._links
                .get(&link_info.link_name)
                .expect("Link not in LinkMap")
                .read()
                .await;

            match link.get_link_state() {
                LinkState::Healthy => match link.get_kpi(link_info.kpi.clone()) {
                    Some(kpi) => {
                        if kpi > link_info.threshold {
                            link_to_quality_map.insert(
                                link_info.link_name.clone(),
                                100.0
                                    - ((i as f32) * (100.0 / self._link_information.len() as f32)),
                            );
                        } else {
                            link_to_quality_map.insert(link_info.link_name.clone(), f32::MIN);
                        }
                    }
                    None => {
                        link_to_quality_map.insert(link_info.link_name.clone(), f32::MIN);
                    }
                },
                LinkState::HighLoss => {
                    if link.get_seq_received() < 128 {
                        match link.get_kpi(link_info.kpi.clone()) {
                            Some(kpi) => {
                                if kpi > link_info.threshold {
                                    link_to_quality_map.insert(
                                        link_info.link_name.clone(),
                                        100.0
                                            - ((i as f32)
                                                * (100.0 / self._link_information.len() as f32)),
                                    );
                                } else {
                                    link_to_quality_map
                                        .insert(link_info.link_name.clone(), f32::MIN);
                                }
                            }
                            None => {
                                link_to_quality_map.insert(link_info.link_name.clone(), f32::MIN);
                            }
                        }
                    } else {
                        link_to_quality_map.insert(link_info.link_name.clone(), f32::MIN);
                    }
                }
                _ => {
                    link_to_quality_map.insert(link_info.link_name.clone(), f32::MIN);
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

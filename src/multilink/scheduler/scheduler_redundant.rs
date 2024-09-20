use crate::multilink::{link::LinkState, scheduler::LinkScheduling, types::LinkHashMap};
use async_trait::async_trait;
use serde_derive::Deserialize;
use std::{collections::HashSet, sync::Arc};

#[cfg(feature = "tracing")]
use tracing::trace;

#[derive(Debug, Deserialize)]
pub struct RedundantSchedulerConfig {
    pub links: Vec<String>,
}

#[derive(Debug)]
pub struct RedundantScheduler {
    _links: Arc<LinkHashMap>,
    _links_to_use: Vec<String>,
    _last_chosen_links: HashSet<String>,
}

impl RedundantScheduler {
    pub fn new(links_to_use: Vec<String>, links: Arc<LinkHashMap>) -> RedundantScheduler {
        let links_arc = links;
        RedundantScheduler {
            _links_to_use: links_to_use,
            _links: links_arc,
            _last_chosen_links: HashSet::new(),
        }
    }
    pub fn get_last_chosen_links(&self) -> HashSet<String> {
        self._last_chosen_links.clone()
    }
}

#[async_trait]
impl LinkScheduling for RedundantScheduler {
    #[cfg_attr(feature = "deep_tracing", tracing::instrument)]
    async fn schedule(&mut self) -> HashSet<String> {
        let mut links = HashSet::new();
        for link_name in self._links_to_use.iter() {
            let link = self
                ._links
                .get(link_name)
                .expect("Link not in Links Map")
                .read()
                .await;
            match link.get_link_state() {
                LinkState::Healthy => {
                    links.insert(link_name.to_string());
                }
                LinkState::HighLoss => {
                    if link.get_seq_received() < 128 {
                        links.insert(link_name.to_string());
                    }
                }
                _ => {}
            }
        }

        #[cfg(feature = "tracing")]
        trace!("links: {:?}", links);

        self._last_chosen_links.clone_from(&links);
        links
    }
}

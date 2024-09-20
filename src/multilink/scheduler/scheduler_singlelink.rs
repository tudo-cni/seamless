use crate::multilink::{link::LinkState, scheduler::LinkScheduling, types::LinkHashMap};
use async_trait::async_trait;
use serde_derive::Deserialize;
use std::{collections::HashSet, sync::Arc};

#[derive(Debug, Deserialize)]
pub struct SingleLinkSchedulerConfig {
    pub link: String,
}
#[derive(Debug)]
pub struct SingleLinkScheduler {
    _link: String,
    _links: Arc<LinkHashMap>,
    _last_chosen_links: HashSet<String>,
}

impl SingleLinkScheduler {
    pub fn new(link: String, links: Arc<LinkHashMap>) -> SingleLinkScheduler {
        SingleLinkScheduler {
            _link: link,
            _links: links,
            _last_chosen_links: HashSet::new(),
        }
    }
    pub fn get_last_chosen_links(&self) -> HashSet<String> {
        self._last_chosen_links.clone()
    }
}

#[async_trait]
impl LinkScheduling for SingleLinkScheduler {
    #[cfg_attr(feature = "deep_tracing", tracing::instrument)]
    async fn schedule(&mut self) -> HashSet<String> {
        let mut links = HashSet::new();
        let link = self
            ._links
            .get(&self._link)
            .expect("Link not in Links Map")
            .read()
            .await;
        match link.get_link_state() {
            LinkState::Healthy => {
                links.insert(self._link.clone());
            }
            LinkState::HighLoss => {
                if link.get_seq_received() < 128 {
                    links.insert(self._link.clone());
                }
            }
            _ => {}
        }
        self._last_chosen_links.clone_from(&links);
        links
    }
}

use crate::multilink::{link::LinkState, scheduler::LinkScheduling, types::LinkHashMap};
use async_trait::async_trait;
use std::{collections::HashSet, sync::Arc, vec};

#[cfg(feature = "tracing")]
use tracing::trace;

#[derive(Debug)]
pub struct RoundRobinScheduler {
    _link_names: Vec<String>,
    _last_link: usize,
    _links: Arc<LinkHashMap>,
    _last_chosen_links: HashSet<String>,
}

impl RoundRobinScheduler {
    pub fn new(links: Arc<LinkHashMap>) -> RoundRobinScheduler {
        let mut scheduler = RoundRobinScheduler {
            _link_names: vec![],
            _last_link: 0,
            _links: links,
            _last_chosen_links: HashSet::new(),
        };
        for key in scheduler._links.keys() {
            scheduler._link_names.push(key.clone());
        }
        scheduler
    }
    pub fn get_last_chosen_links(&self) -> HashSet<String> {
        self._last_chosen_links.clone()
    }
}

#[async_trait]
impl LinkScheduling for RoundRobinScheduler {
    #[cfg_attr(feature = "deep_tracing", tracing::instrument)]
    async fn schedule(&mut self) -> HashSet<String> {
        let mut link_name;
        loop {
            self._last_link = (self._last_link + 1) % self._links.len();
            link_name = self._link_names[self._last_link].clone();
            let link = self
                ._links
                .get(&link_name)
                .expect("Link not in Links Map")
                .read()
                .await;
            match link.get_link_state() {
                LinkState::Healthy => {
                    break;
                }
                LinkState::HighLoss => {
                    if link.get_seq_received() < 128 {
                        break;
                    }
                }
                _ => {}
            }
            drop(link);
        }

        #[cfg(feature = "tracing")]
        trace!("current link: \"{}\"", link_name);
        let mut links = HashSet::new();
        links.insert(link_name);
        self._last_chosen_links.clone_from(&links);
        links
    }
}

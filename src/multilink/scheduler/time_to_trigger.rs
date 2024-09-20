use std::collections::HashMap;
use tokio::time;

#[cfg(feature = "tracing")]
use tracing::trace;

#[derive(Debug)]
pub struct SchedulerTimeToTrigger {
    _time_to_trigger: time::Duration,
    _time_to_trigger_tracker: HashMap<String, Option<time::Instant>>,
    _serving_link: String,
}

impl SchedulerTimeToTrigger {
    pub fn new(links: Vec<String>, time_to_trigger: Option<time::Duration>) -> Self {
        let mut links_ttt: HashMap<String, Option<time::Instant>> = HashMap::new();
        for link in links.iter() {
            links_ttt.insert(link.clone(), None);
        }

        SchedulerTimeToTrigger {
            _time_to_trigger_tracker: links_ttt,
            _serving_link: links[0].clone(),
            _time_to_trigger: time_to_trigger.unwrap_or(time::Duration::from_millis(200)),
        }
    }

    #[cfg_attr(feature = "deep_tracing", tracing::instrument)]
    pub fn update<T: PartialOrd + Clone + Copy + std::fmt::Debug>(
        &mut self,
        link_values: HashMap<String, T>,
    ) {
        if link_values.contains_key(&self._serving_link) {
            for (link, value) in link_values.iter() {
                if *link == self._serving_link {
                    continue;
                }
                if value > &link_values[&self._serving_link] {
                    match self._time_to_trigger_tracker[link] {
                        None => {
                            self._time_to_trigger_tracker
                                .insert(link.to_owned(), Some(time::Instant::now()));
                        }
                        Some(since) => {
                            if since.elapsed() > self._time_to_trigger {
                                link.clone_into(&mut self._serving_link); // Could be extended to not have a race condition regarding first upcoming better link

                                #[cfg(feature = "tracing")]
                                trace!("switched serving link to \"{}\"", self._serving_link);

                                self._clear_tracker();
                                break;
                            }
                        }
                    }
                } else {
                    *self
                        ._time_to_trigger_tracker
                        .entry(link.clone())
                        .or_insert(None) = None;
                }
            }
        } else {
            let mut last_best: Option<T> = None;
            for (link, value) in link_values.iter() {
                match last_best {
                    None => {
                        last_best = Some(*value);
                        link.clone_into(&mut self._serving_link);
                    }
                    Some(best) => {
                        if value > &best {
                            last_best = Some(*value);
                            link.clone_into(&mut self._serving_link);
                        }
                    }
                }
            }

            #[cfg(feature = "tracing")]
            trace!("switched serving link to \"{}\"", self._serving_link);
        }
    }
    pub fn _clear_tracker(&mut self) {
        for (_, val) in self._time_to_trigger_tracker.iter_mut() {
            *val = None;
        }
    }
    pub fn get_serving_link(&self) -> String {
        self._serving_link.to_owned()
    }
}

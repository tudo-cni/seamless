use std::collections::HashSet;
use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct SmoothTransition {
    _handover_duration: Option<Duration>,
    _handover_instant: Option<Instant>,
    _last_links: HashSet<String>,
    _current_links: HashSet<String>,
}

impl SmoothTransition {
    pub fn new(handover_duration: Option<Duration>) -> Self {
        SmoothTransition {
            _handover_duration: handover_duration,
            _handover_instant: None,
            _last_links: HashSet::new(),
            _current_links: HashSet::new(),
        }
    }

    pub fn smooth_out(&mut self, links: HashSet<String>) -> HashSet<String> {
        if let Some(handover_duration) = self._handover_duration {
            // First Pakets, initialize Smoothener
            let mut smoothed_links = links.clone();
            if self._last_links.is_empty() && self._current_links.is_empty() {
                self._last_links.clone_from(&links);
                self._current_links.clone_from(&links);
                return smoothed_links;
            }
            if links != self._last_links {
                if links != self._current_links {
                    self._current_links.clone_from(&links);
                    self._handover_instant = Some(Instant::now());
                }
                if let Some(since) = self._handover_instant {
                    if since.elapsed() < handover_duration {
                        smoothed_links.extend(self._last_links.clone());
                    } else {
                        self._last_links.clone_from(&self._current_links);
                        self._current_links = HashSet::new();
                        self._handover_instant = None;
                    }
                }
            }
            return smoothed_links;
        }
        links
    }
}

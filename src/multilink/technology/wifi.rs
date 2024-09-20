use std::collections::HashMap;

use crate::multilink::technology::GetValue;
use crate::multilink::technology::NormedQuality;
use crate::multilink::technology::SetValue;

#[derive(Debug)]
pub struct WiFi {
    _kpi: HashMap<String, f32>,
}
impl WiFi {
    pub fn new() -> WiFi {
        WiFi {
            _kpi: HashMap::new(),
        }
    }
}
impl SetValue for WiFi {
    fn set_value(&mut self, key: String, value: String) {
        if let Ok(parsed_value) = value.parse::<f32>() {
            self._kpi.insert(key, parsed_value);
        }
    }
}
impl GetValue for WiFi {
    fn get_value(&self, key: String) -> Option<f32> {
        self._kpi.get(&key).copied()
    }
}

impl NormedQuality for WiFi {
    fn get_normed_quality(&self) -> f32 {
        match self._kpi.get("quality") {
            Some(quality) => (quality / 70.0) * 100.0,
            None => 100.0,
        }
    }
}

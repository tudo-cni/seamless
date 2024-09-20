use std::collections::HashMap;

use crate::multilink::technology::{GetValue, NormedQuality, SetValue};

#[derive(Debug)]
pub struct NewRadio {
    _kpi: HashMap<String, f32>,
}

impl NewRadio {
    pub fn new() -> NewRadio {
        NewRadio {
            _kpi: HashMap::new(),
        }
    }
}

impl SetValue for NewRadio {
    fn set_value(&mut self, key: String, value: String) {
        if let Ok(parsed_value) = value.parse::<f32>() {
            self._kpi.insert(key, parsed_value);
        }
    }
}
impl GetValue for NewRadio {
    fn get_value(&self, key: String) -> Option<f32> {
        self._kpi.get(&key).copied()
    }
}

impl NormedQuality for NewRadio {
    fn get_normed_quality(&self) -> f32 {
        match self._kpi.get("ss_sinr") {
            Some(sinr) => {
                let sinr_i = *sinr as i32;
                if sinr_i > 40 {
                    100.0
                } else if (-23..=40).contains(&sinr_i) {
                    let step = 0.63; // Max SINR >40 min -23 => 63
                    let value = sinr_i + 23;
                    return (value as f32 / step) * 100.0;
                } else {
                    return 0.0;
                }
            }
            None => 100.0,
        }
    }
}

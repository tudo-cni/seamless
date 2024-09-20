use std::collections::HashMap;

use crate::multilink::technology::GetValue;
use crate::multilink::technology::SetValue;

#[derive(Debug)]
pub struct Lte {
    _kpi: HashMap<String, f32>,
}
impl Lte {
    pub fn new() -> Lte {
        Lte {
            _kpi: HashMap::new(),
        }
    }
}

impl SetValue for Lte {
    fn set_value(&mut self, key: String, value: String) {
        if let Ok(parsed_value) = value.parse::<f32>() {
            self._kpi
                .insert(key, parsed_value)
                .expect("Couldn't sort into KPI map");
        }
    }
}

impl GetValue for Lte {
    fn get_value(&self, key: String) -> Option<f32> {
        self._kpi.get(&key).copied()
    }
}

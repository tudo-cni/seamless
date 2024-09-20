use serde::Deserialize;

mod lte;
mod newradio;
mod wifi;

pub trait SetValue {
    fn set_value(&mut self, key: String, value: String);
}
pub trait NormedQuality {
    fn get_normed_quality(&self) -> f32;
}

pub trait GetValue {
    fn get_value(&self, name: String) -> Option<f32>;
}

#[derive(Debug)]
pub enum TechnologyData {
    Nr(newradio::NewRadio),
    Lte(lte::Lte),
    Wifi(wifi::WiFi),
    Generic,
}

#[derive(Debug, PartialEq, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Technology {
    Nr,
    Lte,
    Wifi,
    Generic,
}

impl Default for Technology {
    fn default() -> Self {
        Self::Generic
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct TechnologyConfig {
    pub technology: Technology,
    #[serde(default)]
    pub links: Vec<String>,
    pub topic: String,
    pub sub_topic: Option<String>,
    pub fields: Vec<String>,
}

impl TechnologyData {
    pub fn new(technology: &Technology) -> TechnologyData {
        match technology {
            Technology::Wifi => TechnologyData::Wifi(wifi::WiFi::new()),
            Technology::Nr => TechnologyData::Nr(newradio::NewRadio::new()),
            Technology::Lte => TechnologyData::Lte(lte::Lte::new()),
            Technology::Generic => TechnologyData::Generic,
        }
    }
}

impl SetValue for TechnologyData {
    fn set_value(&mut self, key: String, value: String) {
        match self {
            TechnologyData::Nr(_nr) => _nr.set_value(key, value),
            TechnologyData::Wifi(_wifi) => _wifi.set_value(key, value),
            TechnologyData::Lte(_lte) => _lte.set_value(key, value),
            TechnologyData::Generic => {}
        }
    }
}

impl GetValue for TechnologyData {
    fn get_value(&self, key: String) -> Option<f32> {
        match self {
            TechnologyData::Nr(_nr) => _nr.get_value(key),
            TechnologyData::Wifi(_wifi) => _wifi.get_value(key),
            TechnologyData::Lte(_lte) => _lte.get_value(key),
            TechnologyData::Generic => None,
        }
    }
}

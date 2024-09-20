use crate::multilink::technology;
use crate::multilink::types::LinkHashMap;
use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use std::{collections::HashMap, sync::Arc};
use zeromq::{Socket, SocketRecv};

#[derive(Debug, Clone, Deserialize)]
pub struct ExternalMeasurementConfig {
    #[serde(default = "zmq_ip_default")]
    pub zmq_ip: String,
    pub zmq_port: u16,
    #[serde(rename = "measurements")]
    pub technologies: HashMap<String, technology::TechnologyConfig>,
}

fn zmq_ip_default() -> String {
    "localhost".to_string()
}

impl ExternalMeasurementConfig {
    pub fn address(&self) -> String {
        format!("tcp://{}:{}", self.zmq_ip, self.zmq_port)
    }
}

pub async fn zmq_sub(
    conf: ExternalMeasurementConfig,
    links: Arc<LinkHashMap>,
    hostname: String,
) -> Result<()> {
    let mut socket = zeromq::SubSocket::new();

    socket.connect(&conf.address()).await?;
    let mut subscriber_topics: HashMap<String, String> = HashMap::new();
    for key in conf.technologies.keys() {
        for kpi in &conf.technologies[key].fields {
            let mut topic = hostname.clone();
            topic.push(' ');
            topic.push_str(&conf.technologies[key].topic);
            match &conf.technologies[key].sub_topic {
                Some(sub_topic) => {
                    topic.push(' ');
                    topic.push_str(sub_topic);
                }
                None => {}
            }
            topic.push(' ');
            topic.push_str(kpi);

            subscriber_topics.insert(topic.clone(), key.clone());

            socket.subscribe(&topic).await?;
        }
    }

    while let Ok(msg) = socket.recv().await {
        let msg_string: String = msg.try_into().map_err(|e| anyhow!("{e:?}"))?;
        let mut splits: Vec<&str> = msg_string.split(char::is_whitespace).collect();
        let value = splits.pop().unwrap_or("");
        let topic = splits.join(" ");
        let field = splits.pop().unwrap_or("");
        if subscriber_topics.contains_key(&topic) {
            let measurement = subscriber_topics[&topic].clone();
            let measurement_links = conf.technologies[&measurement].links.clone();
            for link in measurement_links {
                links
                    .get(&link)
                    .context("Link not in links")?
                    .write()
                    .await
                    .update_technology(field.to_string(), value.to_string());
            }
        }
        //let fields: HashMap<String,String> = serde_json::from_str(&fields).unwrap();
    }
    Ok(())
}

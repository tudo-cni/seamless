use anyhow::Result;
use serde_derive::Deserialize;
use std::net::Ipv4Addr;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc::Sender;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio_tun::TunBuilder;

#[cfg(feature = "tracing")]
use tracing::{info, trace, Level};

use super::UserGroup;

#[derive(Debug, Clone, Deserialize)]
pub struct InterfaceConfig {
    pub name: String,
    pub local_ip: Ipv4Addr,
    pub remote_ip: Ipv4Addr,
    pub mtu: u32,
    pub netmask: Ipv4Addr,
}
/// The `MultiLinkInterface` represents the tunnel interface exposed to the host system.
pub struct NetworkInterface {
    /// The local IP address exposed to the hostname.
    _local_addr: Ipv4Addr,
    /// The remote IP address for which a route via the tunnel device is exposed
    _remote_addr: Ipv4Addr,
}

impl NetworkInterface {
    /// Generate a new instance of the `MultiLinkInterface` already starting all required read and write async tasks connected to the tunnel interface.
    #[cfg_attr(feature = "deep_tracing", tracing::instrument)]
    pub fn new(
        config: InterfaceConfig,
        user: UserGroup,
        tx: Sender<Vec<u8>>,
        mut rx: UnboundedReceiver<Vec<u8>>,
    ) -> Result<NetworkInterface> {
        #[cfg(feature = "tracing")]
        info!("building tun0 device: {:?}", &config);

        let mut tun0_builder = TunBuilder::new()
            .name(&config.name)
            .tap(false)
            .packet_info(false)
            .mtu(config.mtu.try_into()?)
            .up()
            .address(config.local_ip)
            .destination(config.remote_ip)
            .broadcast(Ipv4Addr::BROADCAST)
            .netmask(config.netmask);

        if let UserGroup::Regular { uid, gid } = user {
            tun0_builder = tun0_builder.owner(uid.try_into().unwrap());
            tun0_builder = tun0_builder.group(gid.try_into().unwrap());
        }

        let tun0 = tun0_builder.try_build()?;

        let (mut reader_tun0, mut writer_tun0) = tokio::io::split(tun0);

        #[cfg(feature = "tracing")]
        let span_tun_write = tracing::span!(Level::TRACE, "tun_write");
        tokio::spawn(async move {
            #[cfg(feature = "tracing")]
            let _enter = span_tun_write.enter();

            while let Some(bytes) = rx.recv().await {
                #[cfg(feature = "tracing")]
                trace!("writing {} bytes: {:?}", bytes.len(), &bytes);

                let mut bytes_send = 0;
                while bytes_send < bytes.len() {
                    bytes_send += writer_tun0
                        .write(&bytes[bytes_send..bytes.len()])
                        .await
                        .expect("Couldn't write to Tun Device");
                }
            }
        });

        #[cfg(feature = "tracing")]
        let span_tun_read = tracing::span!(Level::TRACE, "tun_read");
        tokio::spawn(async move {
            #[cfg(feature = "tracing")]
            let _enter = span_tun_read.enter();

            let mut buf = [0u8; 2000];
            loop {
                let len = reader_tun0
                    .read(&mut buf)
                    .await
                    .expect("Couldn't read from Tun Device");

                #[cfg(feature = "tracing")]
                trace!("reading {} bytes: {:?}", len, buf);

                let _ = tx.send(buf[..len].to_vec()).await;
            }
        });

        Ok(NetworkInterface {
            _local_addr: config.local_ip,
            _remote_addr: config.remote_ip,
        })
    }
}

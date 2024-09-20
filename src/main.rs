use clap::Parser;

use multilink::UserGroup;
#[cfg(feature = "tracing")]
use tracing::info;
#[cfg(feature = "tracing")]
use tracing_subscriber::prelude::*;

use anyhow::Result;

mod argparser;
mod multilink;

//#[tokio::main(flavor = "multi_thread", worker_threads = 10)]
#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    #[cfg(feature = "tracing")]
    init_tracing();

    #[cfg(feature = "tracing")]
    info!("SEAMLESS PID: {}", std::process::id());

    // Uncomment the following line to enable tokio-console
    // console_subscriber::init();

    let args = argparser::Args::parse();
    let conf = multilink::Config::from_file(&args.config)?;
    let tunnel_conf = conf.multilink.tunnel.clone();
    let user = conf.multilink.user;
    let mut _multilink = multilink::Multilink::new(conf.multilink);
    let multilink::UnboundChannel(sender, receiver) = _multilink.start().await?;

    let _tun =
        multilink::network_interface::NetworkInterface::new(tunnel_conf, user, sender, receiver)?;

    // drop privileges
    if let UserGroup::Regular { uid, gid } = user {
        nix::unistd::setresgid(gid.into(), gid.into(), gid.into())?;
        nix::unistd::setresuid(uid.into(), uid.into(), uid.into())?;
    }

    // run with defined privileges
    _multilink.run().await;
    Ok(())
}

#[cfg(feature = "tracing")]
fn init_tracing() {
    let subscriber =
        tracing_subscriber::registry().with(tracing_subscriber::fmt::layer().compact());
    subscriber.init();
}

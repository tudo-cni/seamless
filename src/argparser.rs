use clap::Parser;

/// YAM(L)A Yet Another Multilink Approach
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)] // Read from `Cargo.toml`
pub struct Args {
    /// Path to YAM(L)A config file
    #[arg(short, long, value_name = "CONFIG_FILE")]
    pub config: String,
}

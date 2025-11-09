use clap::Parser;

pub fn default_keypair_path() -> String {
    std::env::var("HOME")
        .map(|h| format!("{h}/.config/solana/id.json"))
        .unwrap_or_else(|_| "./id.json".to_string())
}

pub fn expand_home(p: &str) -> String {
    if let Ok(home) = std::env::var("HOME") {
        if p.starts_with("~/") {
            return format!("{home}/{}", &p[2..]);
        }
    }
    p.to_string()
}

#[derive(Parser, Debug, Clone)]
#[command(name = "chomp-strat-bot", author, version, about = "Baseline Chomp/Glass strat bot for Solana")]
pub struct Cli {
    #[arg(long = "rpc", default_value = "https://api.mainnet-beta.solana.com")]
    pub rpc_url: String,

    #[arg(long = "keypair", default_value_t = default_keypair_path())]
    pub keypair_path: String,

    #[arg(long = "program", env = "PROGRAM_ID", default_value = "ChompZg47TcVy5fk2LxPEpW6SytFYBES5SHoqgrm8A4D")]
    pub program_id: String,

    #[arg(long = "collector", env = "FEE_COLLECTOR", default_value = "EGJnqcxVbhJFJ6Xnchtaw8jmPSvoLXfN2gWsY9Etz5SZ")]
    pub fee_collector: String,

    #[arg(long = "autoplay", default_value_t = false)]
    pub autoplay: bool,

    #[arg(long = "interval_ms", default_value_t = 1500u64)]
    pub interval_ms: u64,

    #[arg(long = "max_moves", default_value_t = 200u32)]
    pub max_moves: u32,

    #[arg(long = "last_move_wins", default_value_t = false)]
    pub last_move_wins: bool,

    #[arg(long = "reset", default_value_t = false)]
    pub reset: bool,

    #[arg(long = "init_if_missing", default_value_t = true)]
    pub init_if_missing: bool,

    #[arg(long = "r")]
    pub row: Option<u8>,

    #[arg(long = "c")]
    pub col: Option<u8>,

    #[arg(long = "cash_out", default_value_t = false)]
    pub cash_out: bool,
}

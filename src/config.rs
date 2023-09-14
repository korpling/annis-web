use clap::{Parser, ValueEnum};
use std::path::PathBuf;

#[derive(ValueEnum, Clone)]
enum JWTVerificationType {
    HS256,
    RS256,
}

#[derive(Parser, Default)]
#[command(author, version, about, long_about = None)]
pub struct CliConfig {
    /// Port to listen to
    #[arg(long, short, default_value_t = 3000)]
    pub port: u16,
    /// If set, the SQLite database file to store sessions in
    #[arg(long)]
    pub session_file: Option<PathBuf>,
}

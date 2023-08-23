use crate::{state::JwtType, Result};
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
    /// Verification algorithm for the JWT token used by the graphANNIS backend.
    #[arg(long)]
    jwt_type: Option<JWTVerificationType>,
    /// File containg the public (RS256) or private key (HS256) used for
    /// verifing the JWT token used by the graphANNIS backend.
    #[arg(long)]
    jwt_key_file: Option<PathBuf>,
}

impl CliConfig {
    pub fn jwt_type(&self) -> Result<JwtType> {
        if let (Some(jwt_type), Some(jwt_key_file)) = (&self.jwt_type, &self.jwt_key_file) {
            let key_file_content = std::fs::read_to_string(jwt_key_file)?;
            match jwt_type {
                JWTVerificationType::HS256 => {
                    let key = jsonwebtoken::DecodingKey::from_secret(key_file_content.as_bytes());
                    Ok(JwtType::HS256(key))
                }
                JWTVerificationType::RS256 => {
                    let key = jsonwebtoken::DecodingKey::from_rsa_pem(key_file_content.as_bytes())?;
                    Ok(JwtType::RS256(key))
                }
            }
        } else {
            Ok(JwtType::None)
        }
    }
}

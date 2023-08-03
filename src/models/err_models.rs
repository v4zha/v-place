use std::error::Error;
use std::fmt::Display;

use redis::RedisError;

#[derive(Debug)]
pub enum VpError {
    RedisErr(RedisError),
    // ScyllaErr(&'a str),
}
impl Error for VpError {}

impl From<RedisError> for VpError {
    fn from(value: RedisError) -> Self {
        Self::RedisErr(value)
    }
}

impl Display for VpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use VpError::*;
        match self {
            RedisErr(e) => write!(f, "[Redis Error]: {}", e),
            // ScyllaErr(e) => write!(f, "[Scylla Error]: {}", e),
        }
    }
}
impl actix_web::ResponseError for VpError {}

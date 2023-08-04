use std::error::Error;
use std::fmt::Display;
use std::num::TryFromIntError;

use redis::RedisError;
use scylla::transport::errors::QueryError;
use scylla::transport::query_result::FirstRowTypedError;

#[derive(Debug)]
pub enum VpError {
    InitCanvasErr,
    RedisErr(RedisError),
    ColorSizeMismatch,
    CanvasSizeMismatch,
    InvalidUser,
    ScyllaQueryErr(QueryError),
    ScyllaTypeErr(FirstRowTypedError),
    ParseIntErr(TryFromIntError),
}
impl Error for VpError {}

impl From<RedisError> for VpError {
    fn from(err: RedisError) -> Self {
        Self::RedisErr(err)
    }
}
impl From<QueryError> for VpError {
    fn from(err: QueryError) -> Self {
        Self::ScyllaQueryErr(err)
    }
}
impl From<FirstRowTypedError> for VpError {
    fn from(err: FirstRowTypedError) -> Self {
        Self::ScyllaTypeErr(err)
    }
}

impl From<TryFromIntError> for VpError {
    fn from(err: TryFromIntError) -> Self {
        Self::ParseIntErr(err)
    }
}

impl Display for VpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use VpError::*;
        match self {
            InitCanvasErr => write!(f, "Unable to initialize canvas"),
            RedisErr(e) => write!(f, "[Redis Error]: {}", e),
            ColorSizeMismatch => write!(
                f,
                "[Color Size Mismatch]: color size > 15. accepted range [0,15]"
            ),
            InvalidUser => write!(f, "[Invalid User]: Invalid User Id"),
            ScyllaQueryErr(e) => write!(f, "[Scylla Query Error]: {}", e),
            ScyllaTypeErr(e) => write!(f, "[Scylla Row Type Error]: {}", e),
            ParseIntErr(e) => write!(f, "[Error parsing Int]: {}", e),
            CanvasSizeMismatch => {
                write!(f, "[Canvas Size Mismatch]: Enter (x,y) < Canvas Dimension")
            }
        }
    }
}
impl actix_web::ResponseError for VpError {}

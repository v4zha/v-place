use chrono::Utc;
use redis::Client;
use uuid::Uuid;

use crate::models::err_models::VpError;
use crate::models::p_models::AppState;
use crate::models::scylla_models::ScyllaManager;

pub async fn init_place(app_state: &AppState<'_>, redis: &Client) -> Result<(), VpError> {
    let mut conn = redis.get_tokio_connection_manager().await?;
    let dim: usize = app_state
        .canvas_dim
        .try_into()
        .map_err(|_| VpError::InitCanvasErr)?;
    let canvas_size: usize = (dim * dim + 1) / 2;
    log::debug!("Canvas Bitfield size {}", canvas_size);
    redis::Cmd::set(app_state.canvas_id.as_bytes(), vec![0u8; canvas_size])
        .query_async(&mut conn)
        .await?;
    Ok(())
}
pub async fn diff_last_placed(
    uid: &Uuid,
    cooldown: usize,
    scylla: &ScyllaManager,
) -> Result<i64, VpError> {
    let res = scylla.get_user(uid).await;
    match res {
        Ok(user) => Ok(Utc::now().timestamp() - user.last_placed),
        Err(VpError::InvalidUser) => Ok(i64::try_from(cooldown)?),
        Err(e) => Err(e),
    }
}

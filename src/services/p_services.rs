use redis::Client;

use crate::models::err_models::VpError;
use crate::models::p_models::AppState;

pub async fn init_canvas(app_state: &AppState<'_>, redis: &Client) -> Result<(), VpError> {
    let mut conn = redis.get_tokio_connection_manager().await?;
    let dim: usize = app_state
        .canvas_dim
        .try_into()
        .map_err(|_| VpError::InitCanvasErr)?;
    let canvas_size = dim * dim/2;
    log::debug!("Canvas Bitfield size {}", canvas_size);
    redis::Cmd::set(app_state.canvas_id.as_bytes(), vec![0u8; canvas_size])
        .query_async(&mut conn)
        .await?;
    Ok(())
}

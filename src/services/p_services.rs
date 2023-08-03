use redis::Client;

use crate::models::err_models::VpError;
use crate::models::p_models::AppState;

pub async fn init_canvas(app_state: &AppState<'_>, redis: &Client) -> Result<(), VpError> {
    let mut conn = redis.get_tokio_connection_manager().await?;
    redis::Cmd::set(
        app_state.canvas_id.as_bytes(),
        vec![0u8; app_state.canvas_dim * app_state.canvas_dim],
    )
    .query_async::<_, String>(&mut conn)
    .await?;
    Ok(())
}

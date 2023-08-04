use chrono::Utc;
use redis::Client;
use scylla::transport::query_result::FirstRowTypedError;
use scylla::Session;
use uuid::Uuid;

use crate::models::err_models::VpError;
use crate::models::p_models::{AppState, UserDetails};

pub async fn init_place(
    app_state: &AppState<'_>,
    redis: &Client,
    scylla: &Session,
) -> Result<(), VpError> {
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
    scylla.query("CREATE KEYSPACE IF NOT EXISTS vplace WITH REPLICATION = {'class' : 'NetworkTopologyStrategy', 'replication_factor' : 1}", &[]).await?;
    scylla
        .query("CREATE TABLE IF NOT EXISTS vplace.player (id uuid,uname text,x int,y int,color int,last_placed timestamp,primary key (id))", &[])
        .await?;
    Ok(())
}
pub async fn diff_last_placed(
    uid: &Uuid,
    cooldown: usize,
    scylla: &Session,
) -> Result<i64, VpError> {
    let query = "SELECT id, uname, x, y, color, last_placed FROM vplace.player WHERE id = ?";
    let rows = scylla.query(query, (uid,)).await?;
    let res = rows.first_row_typed::<UserDetails>();
    match res {
        Ok(user) => Ok(Utc::now().timestamp() - user.last_placed),
        Err(FirstRowTypedError::RowsEmpty) => Ok(i64::try_from(cooldown)?),
        Err(e) => Err(VpError::ScyllaTypeErr(e)),
    }
}

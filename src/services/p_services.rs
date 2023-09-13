use actix::Addr;
use chrono::Utc;
use futures::TryFutureExt;
use redis::Client;
use uuid::Uuid;

use crate::models::err_models::VpError;
use crate::models::p_models::{AppState, PlaceUpdate, UpdatePixel, VpSrv};
use crate::models::scylla_models::ScyllaManager;

pub async fn init_place(app_state: &AppState<'_>, redis: &Client) -> Result<(), VpError> {
    let mut conn = redis.get_tokio_connection_manager().await?;
    if redis::Cmd::exists(app_state.canvas_id.as_bytes())
        .query_async::<_, u8>(&mut conn)
        .await?
        .eq(&0)
    {
        let dim: usize = app_state
            .canvas_dim
            .try_into()
            .map_err(|_| VpError::InitCanvasErr)?;
        let canvas_size: usize = (dim * dim + 1) / 2;
        log::debug!("Canvas Bitfield size {}", canvas_size);
        redis::Cmd::set(app_state.canvas_id.as_bytes(), vec![0u8; canvas_size])
            .query_async(&mut conn)
            .await?;
    }
    Ok(())
}

pub async fn reset_place(
    app_state: &AppState<'_>,
    redis: &Client,
    scylla: &ScyllaManager,
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
    log::debug!("[Redis] : Canvas Reset {}", app_state.canvas_id);
    scylla.reset_db().await?;
    log::debug!("[SycallaDb] : vplace.player & vplace.pixel_data Reset");
    Ok(())
}

pub async fn update_place(
    u_req: &UpdatePixel,
    app_data: &AppState<'_>,
    redis: &Client,
    scylla: &ScyllaManager,

    pu_srv: &Addr<VpSrv<'_>>,
) -> Result<(), VpError> {
    // color size-> 16 colors [0,15], max val -> 15
    if u_req.color <= 15 {
        if u_req.loc.0 < app_data.canvas_dim && u_req.loc.1 < app_data.canvas_dim {
            let offset: u32 = u_req.loc.0 * app_data.canvas_dim + u_req.loc.1;
            // set redis bitmap
            let mut conn = redis
                .get_tokio_connection_manager()
                .await
                .map_err(VpError::RedisErr)?;
            let redis_fut = async {
                redis::cmd("bitfield")
                    .arg(app_data.canvas_id.as_bytes())
                    .arg("SET")
                    .arg("u4")
                    .arg(format!("#{}", offset))
                    .arg(u_req.color)
                    .query_async::<_, ()>(&mut conn)
                    .await
            }
            .map_err(VpError::RedisErr);
            // update user timestamp in scylladb
            //also update pixeldata : )
            let scylla_fut = scylla.update_db(u_req);

            //execute both  database fut : )
            tokio::try_join!(redis_fut, scylla_fut)?;
            // uid and uname not send to client : )
            // pixel based query will be added as different endpoint : )
            log::debug!(
                "updated color : {} for location ({},{}) ",
                u_req.color,
                u_req.loc.0,
                u_req.loc.1
            );
            pu_srv.do_send(PlaceUpdate {
                loc: u_req.loc,
                color: u_req.color,
            });
            Ok(())
        } else {
            Err(VpError::CanvasSizeMismatch)?
        }
    } else {
        Err(VpError::ColorSizeMismatch)?
    }
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

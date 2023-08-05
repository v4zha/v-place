use std::borrow::Cow;

use actix::{Addr, Handler, StreamHandler};
use actix_web::{get, post, web, Either, HttpRequest, HttpResponse, Responder};
use actix_web_actors::ws;
use base64::engine::general_purpose;
use base64::Engine;
use chrono::Utc;
use redis::Client;
use scylla::Session;

use crate::models::err_models::VpError;
use crate::models::p_models::{
    AppState, CanvasResponse, PuConnect, PuDisconnect, PuListener, PuRes, PuSrv, UpdatePixel,
    WaitTime,
};
use crate::services::p_services::diff_last_placed;

#[get("/canvas")]
async fn get_canvas(
    app_data: web::Data<AppState<'_>>,
    redis: web::Data<Client>,
) -> actix_web::Result<impl Responder> {
    let mut conn = redis
        .get_tokio_connection_manager()
        .await
        .map_err(VpError::RedisErr)?;
    let res = redis::Cmd::get(app_data.canvas_id.as_bytes())
        .query_async::<_, Vec<u8>>(&mut conn)
        .await
        .map_err(VpError::RedisErr)?;
    //base64 encode the bytearray
    let resb64 = general_purpose::STANDARD_NO_PAD.encode(res);
    Ok(HttpResponse::Ok().json(CanvasResponse {
        id: app_data.canvas_id.as_ref(),
        dim: app_data.canvas_dim,
        canvas: &resb64,
    }))
}

#[get("/vplace")]
pub async fn vplace(
    req: HttpRequest,
    srv_addr: web::Data<Addr<PuSrv<'_>>>,
    stream: web::Payload,
) -> impl Responder {
    ws::start(PuListener::new(srv_addr.clone()), &req, stream)
}

#[post("/pixel/update")]
async fn update_pixel(
    update_req: web::Json<UpdatePixel>,
    app_data: web::Data<AppState<'_>>,
    redis: web::Data<Client>,
    scylla: web::Data<Session>,
    pu_srv: web::Data<Addr<PuSrv<'_>>>,
) -> actix_web::Result<impl Responder> {
    let req = update_req.into_inner();
    let mut conn = redis
        .get_tokio_connection_manager()
        .await
        .map_err(VpError::RedisErr)?;
    // color size-> 16 colors [0,15], max val -> 15
    if req.color <= 15 {
        if req.loc.0 < app_data.canvas_dim && req.loc.1 < app_data.canvas_dim {
            if req.uid.is_some() && req.uname.is_some() {
                let time_diff: i64 =
                    diff_last_placed(&req.uid.unwrap(), app_data.cooldown, &scylla).await?;
                let cd = i64::try_from(app_data.cooldown).map_err(VpError::ParseIntErr)?;
                if time_diff.ge(&cd) {
                    let offset: u32 = req.loc.1 * app_data.canvas_dim + req.loc.0;
                    // set redis bitmap
                    redis::cmd("bitfield")
                        .arg(app_data.canvas_id.as_bytes())
                        .arg("SET")
                        .arg("u4")
                        .arg(format!("#{}", offset))
                        .arg(req.color)
                        .query_async(&mut conn)
                        .await
                        .map_err(VpError::RedisErr)?;
                    // update user timestamp in scylladb
                    let ix = i32::try_from(req.loc.0).map_err(VpError::ParseIntErr)?;
                    let iy = i32::try_from(req.loc.1).map_err(VpError::ParseIntErr)?;
                    let ic = i32::try_from(req.color)?;
                    scylla .query("INSERT INTO vplace.player (id, uname, x, y, color, last_placed) VALUES (?, ?, ?, ?, ?, ?)",
            (req.uid, req.uname,ix,iy,ic, Utc::now().timestamp()),
        ).await.map_err(VpError::ScyllaQueryErr)?;
                    // uid and uname not send to client : )
                    // pixel based query will be added as different endpoint : )
                    pu_srv.do_send(UpdatePixel {
                        uid: None,
                        uname: None,
                        loc: req.loc,
                        color: req.color,
                    });
                    Ok(Either::Left(HttpResponse::Ok()))
                } else {
                    Ok(Either::Right(HttpResponse::Forbidden().json(WaitTime {
                        rem_wait: cd - time_diff,
                    })))
                }
            } else {
                Err(VpError::InvalidUser)?
            }
        } else {
            Err(VpError::CanvasSizeMismatch)?
        }
    } else {
        Err(VpError::ColorSizeMismatch)?
    }
}

// websocket handlers
impl<'a> StreamHandler<Result<ws::Message, ws::ProtocolError>> for PuListener<'a> {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        use ws::Message::*;
        if let Ok(Ping(msg)) = msg {
            ctx.pong(&msg)
        }
    }
}
impl Handler<PuConnect<'_>> for PuSrv<'_> {
    type Result = ();

    fn handle(&mut self, msg: PuConnect, _ctx: &mut Self::Context) -> Self::Result {
        self.listeners.insert(msg.0.clone());
        log::debug!(
            "New client connection.Total connection count : {}",
            self.listeners.len()
        );
    }
}
impl Handler<PuDisconnect<'_>> for PuSrv<'_> {
    type Result = ();

    fn handle(&mut self, msg: PuDisconnect, _ctx: &mut Self::Context) -> Self::Result {
        self.listeners.remove(&msg.0);
        log::debug!(
            "Client Disconnected.Total connection count : {}",
            self.listeners.len()
        );
    }
}

impl Handler<UpdatePixel> for PuSrv<'_> {
    type Result = ();

    fn handle(&mut self, msg: UpdatePixel, _ctx: &mut Self::Context) -> Self::Result {
        if let Ok(res) = serde_json::to_string(&msg) {
            let msg = Cow::from(res);
            self.listeners
                .iter()
                .for_each(|addr| addr.do_send(PuRes(msg.clone())));
        }
    }
}

impl Handler<PuRes<'_>> for PuListener<'_> {
    type Result = ();

    fn handle(&mut self, msg: PuRes, ctx: &mut Self::Context) -> Self::Result {
        ctx.text(msg.0.as_ref());
    }
}

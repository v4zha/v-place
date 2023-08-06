use std::borrow::Cow;

use actix::{Addr, Handler, StreamHandler};
use actix_web::{get, post, web, Either, HttpRequest, HttpResponse, Responder};
use actix_web_actors::ws;
use base64::engine::general_purpose;
use base64::Engine;
use futures::TryFutureExt;
use redis::Client;

use crate::models::err_models::VpError;
use crate::models::p_models::{
    AppState, CanvasResponse, UpdatePixel, VpConnect, VpDisconnect, VpListener, VpRes, VpSrv,
    WaitTime,
};
use crate::models::scylla_models::ScyllaManager;
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
    srv_addr: web::Data<Addr<VpSrv<'_>>>,
    stream: web::Payload,
) -> impl Responder {
    ws::start(VpListener::new(srv_addr.clone()), &req, stream)
}

#[get("/pixel/{x}/{y}")]
pub async fn pixel_info(
    path: web::Path<(u32, u32)>,
    app_data: web::Data<AppState<'_>>,
    scylla: web::Data<ScyllaManager>,
) -> actix_web::Result<impl Responder> {
    let (x, y) = path.into_inner();
    if x < app_data.canvas_dim && y < app_data.canvas_dim {
        let res = scylla.get_pixel(x, y).await;
        match res {
            Ok(pixel) => Ok(HttpResponse::Ok().json(pixel)),
            Err(VpError::NoPixelData) => Ok(HttpResponse::NotFound().body("no Pixel Data Found")),
            Err(e) => Err(e)?,
        }
    } else {
        Err(VpError::CanvasSizeMismatch)?
    }
}

#[post("/pixel/update")]
async fn update_pixel(
    update_req: web::Json<UpdatePixel>,
    app_data: web::Data<AppState<'_>>,
    redis: web::Data<Client>,
    scylla: web::Data<ScyllaManager>,
    pu_srv: web::Data<Addr<VpSrv<'_>>>,
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
                    let redis_fut = async {
                        redis::cmd("bitfield")
                            .arg(app_data.canvas_id.as_bytes())
                            .arg("SET")
                            .arg("u4")
                            .arg(format!("#{}", offset))
                            .arg(req.color)
                            .query_async::<_, ()>(&mut conn)
                            .await
                    }
                    .map_err(VpError::RedisErr);
                    // update user timestamp in scylladb
                    //also update pixeldata : )
                    let scylla_fut = scylla.update_db(&req);

                    //execute both  database fut : )
                    tokio::try_join!(redis_fut, scylla_fut)?;
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
impl<'a> StreamHandler<Result<ws::Message, ws::ProtocolError>> for VpListener<'a> {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        use ws::Message::*;
        if let Ok(Ping(msg)) = msg {
            ctx.pong(&msg)
        }
    }
}
impl Handler<VpConnect<'_>> for VpSrv<'_> {
    type Result = ();

    fn handle(&mut self, msg: VpConnect, _ctx: &mut Self::Context) -> Self::Result {
        self.listeners.insert(msg.0.clone());
        log::debug!(
            "New client connection.Total connection count : {}",
            self.listeners.len()
        );
    }
}
impl Handler<VpDisconnect<'_>> for VpSrv<'_> {
    type Result = ();

    fn handle(&mut self, msg: VpDisconnect, _ctx: &mut Self::Context) -> Self::Result {
        self.listeners.remove(&msg.0);
        log::debug!(
            "Client Disconnected.Total connection count : {}",
            self.listeners.len()
        );
    }
}

impl Handler<UpdatePixel> for VpSrv<'_> {
    type Result = ();

    fn handle(&mut self, msg: UpdatePixel, _ctx: &mut Self::Context) -> Self::Result {
        if let Ok(res) = serde_json::to_string(&msg) {
            let msg = Cow::from(res);
            self.listeners
                .iter()
                .for_each(|addr| addr.do_send(VpRes(msg.clone())));
        }
    }
}

impl Handler<VpRes<'_>> for VpListener<'_> {
    type Result = ();

    fn handle(&mut self, msg: VpRes, ctx: &mut Self::Context) -> Self::Result {
        ctx.text(msg.0.as_ref());
    }
}

use actix::{Addr, Handler, StreamHandler};
use actix_web::{get, web, HttpRequest, HttpResponse, Responder};
use actix_web_actors::ws;
use redis::Client;

use crate::models::err_models::VpError;
use crate::models::p_models::{
    AppState, CanvasResponse, PuConnect, PuDisconnect, PuListener, PuSrv,
};

#[get("/canvas")]
async fn get_canvas(
    app_data: web::Data<AppState<'_>>,
    redis: web::Data<Client>,
) -> actix_web::Result<impl Responder> {
    let mut conn = redis
        .get_tokio_connection_manager()
        .await
        .map_err(|e| VpError::RedisErr(e))?;
    let res = redis::Cmd::get(app_data.canvas_id.as_bytes())
        .query_async::<_, String>(&mut conn)
        .await
        .map_err(|e| VpError::RedisErr(e))?;
    Ok(HttpResponse::Ok().json(CanvasResponse {
        id: app_data.canvas_id.as_ref(),
        canvas: &res,
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

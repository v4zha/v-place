use std::borrow::Cow;
use std::collections::HashSet;

use actix::{Actor, Addr, AsyncContext, Message};
use actix_web::web;
use actix_web_actors::ws;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Deserialize)]
pub struct UpdatePixel {
    pub uid: Uuid,
    pub uname: String,
    // coordinates : (x,y)
    pub loc: (u32, u32),
    pub color: u8,
}

#[derive(Message, Serialize)]
#[rtype(result = "()")]
pub struct PlaceUpdate {
    // coordinates : (x,y)
    pub loc: (u32, u32),
    pub color: u8,
}

#[derive(Serialize)]
pub struct CanvasResponse<'a> {
    pub id: &'a str,
    pub dim: u32,
    pub canvas: &'a str,
    pub cooldown: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WaitTime {
    pub rem_wait: i64,
}

//AppState
pub struct AppState<'a> {
    pub canvas_id: Cow<'a, str>,
    // dim*dim is the the real dimension of canvas
    //admin Id
    pub admin_token: Cow<'a, str>,
    pub canvas_dim: u32,
    pub cooldown: usize,
}
impl<'a> AppState<'a> {
    pub fn new(
        admin_token: Cow<'a, str>,
        canvas_id: Cow<'a, str>,
        canvas_dim: u32,
        cooldown: usize,
    ) -> Self {
        Self {
            admin_token,
            canvas_id,
            canvas_dim,
            cooldown,
        }
    }
}

// Pixel Update Server Actor
pub struct VpSrv<'a: 'static> {
    pub listeners: HashSet<Addr<VpListener<'a>>>,
}
impl<'a> VpSrv<'a> {
    pub fn new() -> Self {
        VpSrv {
            listeners: HashSet::new(),
        }
    }
}
// Pixel Update Listener Actor
pub struct VpListener<'a: 'static> {
    srv_addr: web::Data<Addr<VpSrv<'a>>>,
    addr: Option<Addr<Self>>,
}
impl<'a> VpListener<'a> {
    pub fn new(srv_addr: web::Data<Addr<VpSrv<'a>>>) -> Self {
        Self {
            srv_addr,
            addr: None,
        }
    }
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct VpConnect<'a: 'static>(pub Addr<VpListener<'a>>);

#[derive(Message)]
#[rtype(result = "()")]
pub struct VpDisconnect<'a: 'static>(pub Addr<VpListener<'a>>);

#[derive(Message)]
#[rtype(result = "()")]
pub struct VpRes<'a>(pub Cow<'a, str>);

impl<'a> Actor for VpListener<'a> {
    type Context = ws::WebsocketContext<Self>;
    fn started(&mut self, ctx: &mut Self::Context) {
        let addr = ctx.address();
        self.addr = Some(addr.clone());
        self.srv_addr.do_send(VpConnect(addr));
    }
    fn stopped(&mut self, _ctx: &mut Self::Context) {
        if let Some(addr) = &self.addr {
            self.srv_addr.do_send(VpDisconnect(addr.clone()))
        }
    }
}
impl<'a> Actor for VpSrv<'a> {
    type Context = actix::Context<Self>;
}

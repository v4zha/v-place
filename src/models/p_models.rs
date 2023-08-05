use std::borrow::Cow;
use std::collections::HashSet;

use actix::{Actor, Addr, AsyncContext, Message};
use actix_web::web;
use actix_web_actors::ws;
use scylla::FromRow;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Message)]
#[rtype(result = "()")]
#[derive(Serialize, Deserialize)]
pub struct UpdatePixel {
    pub uid: Option<Uuid>,
    pub uname: Option<String>,
    // coordinates : (x,y)
    pub loc: (u32, u32),
    pub color: u8,
}

#[derive(Serialize)]
pub struct CanvasResponse<'a> {
    pub id: &'a str,
    pub dim: u32,
    pub canvas: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WaitTime {
    pub rem_wait: i64,
}

//ScyllaDb RowData
#[derive(FromRow)]
pub struct UserDetails {
    pub id: Uuid,
    pub name: String,
    pub x: i32,     //u32 aan sherikkum , but CQL derive does'nt support : )
    pub y: i32,     // same as above : )
    pub color: i32, // sherikkum u8
    pub last_placed: i64,
}

//AppState
pub struct AppState<'a> {
    pub canvas_id: Cow<'a, str>,
    // dim*dim is the the real dimension of canvas
    pub canvas_dim: u32,
    pub cooldown: usize,
}
impl<'a> AppState<'a> {
    pub fn new(canvas_id: Cow<'a, str>, canvas_dim: u32, cooldown: usize) -> Self {
        Self {
            canvas_id,
            canvas_dim,
            cooldown,
        }
    }
}

// Pixel Update Server Actor
pub struct PuSrv<'a: 'static> {
    pub listeners: HashSet<Addr<PuListener<'a>>>,
}
impl<'a> PuSrv<'a> {
    pub fn new() -> Self {
        PuSrv {
            listeners: HashSet::new(),
        }
    }
}
// Pixel Update Listener Actor
pub struct PuListener<'a: 'static> {
    srv_addr: web::Data<Addr<PuSrv<'a>>>,
    addr: Option<Addr<Self>>,
}
impl<'a> PuListener<'a> {
    pub fn new(srv_addr: web::Data<Addr<PuSrv<'a>>>) -> Self {
        Self {
            srv_addr,
            addr: None,
        }
    }
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct PuConnect<'a: 'static>(pub Addr<PuListener<'a>>);

#[derive(Message)]
#[rtype(result = "()")]
pub struct PuDisconnect<'a: 'static>(pub Addr<PuListener<'a>>);

#[derive(Message)]
#[rtype(result = "()")]
pub struct PuRes<'a>(pub Cow<'a, str>);

impl<'a> Actor for PuListener<'a> {
    type Context = ws::WebsocketContext<Self>;
    fn started(&mut self, ctx: &mut Self::Context) {
        let addr = ctx.address();
        self.addr = Some(addr.clone());
        self.srv_addr.do_send(PuConnect(addr));
    }
    fn stopped(&mut self, _ctx: &mut Self::Context) {
        if let Some(addr) = &self.addr {
            self.srv_addr.do_send(PuDisconnect(addr.clone()))
        }
    }
}
impl<'a> Actor for PuSrv<'a> {
    type Context = actix::Context<Self>;
}

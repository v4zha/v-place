use std::borrow::Cow;
use std::collections::HashSet;

use actix::{Actor, Addr};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Coordinate {
    x: u32,
    y: u32,
}
#[derive(Serialize, Deserialize)]
pub struct UpdatePixel<'a> {
    uname: Cow<'a, str>,
    loc: Coordinate,
    color: u8,
}

// Pixel Update Server Actor
pub struct PuSrv {
    listeners: HashSet<Addr<PuListener>>,
}

// Pixel Update Listener Actor
pub struct PuListener {}

impl Actor for PuListener {
    type Context = actix::Context<Self>;
}
impl Actor for PuSrv {
    type Context = actix::Context<Self>;
}

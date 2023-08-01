extern crate redis;
mod handlers;
mod models;

use std::env;
use std::sync::{Arc, OnceLock};

use actix_cors::Cors;
use actix_web::middleware::Logger;
use actix_web::{web, App, HttpServer};
use dotenvy::dotenv;
use handlers::p_handlers::get_canvas;
use scylla::SessionBuilder;

static CANVAS_DIM: OnceLock<u32> = OnceLock::new();

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv().ok();
    env::set_var("RUST_LOG", "actix_web=debug");
    env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .init();
    let host = env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port = env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let redis_url = env::var("REDIS_URL").unwrap_or_else(|_| "redis://0.0.0.0:6379".to_string());
    let scylla_url = env::var("SCYLLA_URL").unwrap_or_else(|_| "0.0.0.0:9042".to_string());
    let canvas_dim =
        env::var("CANVAS_DIM").map_or(500, |count| count.parse::<u32>().unwrap_or(500));
    CANVAS_DIM
        .set(canvas_dim)
        .expect("Unable to set CANVAS_DIM");
    let host_port = format!("{}:{}", host, port);
    let redis = redis::Client::open(redis_url).expect("Error connecting to RedisDB");
    let scylla_session = SessionBuilder::new()
        .known_node(scylla_url)
        .build()
        .await
        .expect("Error connecting to ScyllaDB");
    let scylla = Arc::new(scylla_session);
    log::info!("v-place server listening on : {}", host_port);
    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(redis.clone()))
            .app_data(web::Data::new(scylla.clone()))
            .wrap(Logger::default())
            .wrap(Cors::permissive())
            .service(get_canvas)
    })
    .bind(host_port)?
    .run()
    .await
}

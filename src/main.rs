mod handlers;
mod models;
mod services;
use std::env;

use actix::Actor;
use actix_cors::Cors;
use actix_web::middleware::Logger;
use actix_web::{http, web, App, HttpServer};
use dotenvy::dotenv;
use handlers::p_handlers::get_canvas;
use mimalloc::MiMalloc;

use crate::handlers::p_handlers::{pixel_info, update_pixel, vplace};
use crate::models::p_models::{AppState, VpSrv};
use crate::models::scylla_models::ScyllaBuilder;
use crate::services::p_services::init_place;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;
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
    let canvas_id = env::var("CANVAS_ID").unwrap_or_else(|_| "vplace_1".to_string());
    let cooldown = env::var("COOLDOWN").map_or(60, |c| c.parse::<usize>().unwrap_or(60));
    let host_port = format!("{}:{}", host, port);
    let redis_client = redis::Client::open(redis_url).expect("Error connecting to RedisDB");
    let redis = web::Data::new(redis_client);
    let scylla_man = ScyllaBuilder::try_init(&scylla_url, canvas_dim)
        .await
        .expect("Error initiating ScyllaBuilder")
        .try_build()
        .await
        .expect("Unable to Build ScyllaManger");
    let scylla = web::Data::new(scylla_man);
    let app_state = web::Data::new(AppState::new(canvas_id.into(), canvas_dim, cooldown));
    let vp_srv = VpSrv::new().start();
    init_place(&app_state, &redis)
        .await
        .expect("Error Initialising Canvas");
    log::debug!("Canvas {} Initialised.", app_state.canvas_id);
    log::debug!(
        "Canvas Dimension : {}x{}",
        app_state.canvas_dim,
        app_state.canvas_dim
    );
    log::info!("v-place server listening on : {}", host_port);
    let cpus = num_cpus::get();
    HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            // use only in testing : )
            .wrap(
                Cors::default()
                    .allowed_origin("*")
                    .allowed_methods(vec!["GET", "POST"])
                    .allowed_headers(vec![http::header::AUTHORIZATION, http::header::ACCEPT])
                    .allowed_header(http::header::CONTENT_TYPE)
                    .max_age(3600),
            )
            .app_data(app_state.clone())
            .app_data(web::Data::new(vp_srv.clone()))
            .app_data(redis.clone())
            .app_data(scylla.clone())
            .service(vplace)
            .service(get_canvas)
            .service(update_pixel)
            .service(pixel_info)
    })
    .bind(host_port)?
    .workers(cpus * 2)
    .run()
    .await
}

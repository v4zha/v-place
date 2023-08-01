use actix_web::{get, HttpResponse, Responder};

#[get("/canvas")]
async fn get_canvas() -> impl Responder {
    HttpResponse::Ok()
}

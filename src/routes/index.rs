use rocket::{form::Form, serde::json::Json, State};
use rocket_db_pools::Connection;
use todel::{
    http::ClientIP,
    ids::IDGenerator,
    models::{FetchResponse, File, FileData, FileUpload},
    Conf,
};
use tokio::sync::Mutex;

use crate::{
    ratelimit::{RatelimitedRouteResponse, Ratelimiter},
    Cache, DB,
};

#[post("/", data = "<upload>")]
pub async fn upload<'a>(
    upload: Form<FileUpload<'a>>,
    ip: ClientIP,
    mut cache: Connection<Cache>,
    mut db: Connection<DB>,
    conf: &State<Conf>,
    gen: &State<Mutex<IDGenerator>>,
) -> RatelimitedRouteResponse<Json<FileData>> {
    let mut ratelimiter = Ratelimiter::new("attachments", "attachments", ip, conf.inner());
    ratelimiter
        .process_ratelimit(upload.file.len() as u128, &mut cache)
        .await?;
    let upload = upload.into_inner();
    let file = File::create(
        upload.file,
        "attachments".to_string(),
        gen.inner(),
        &mut db,
        upload.spoiler,
    )
    .await
    .map_err(|e| ratelimiter.wrap_response::<_, ()>(e).unwrap())?;
    ratelimiter.wrap_response(Json(file))
}

#[get("/<id>")]
pub async fn fetch<'a>(
    id: u128,
    ip: ClientIP,
    mut cache: Connection<Cache>,
    mut db: Connection<DB>,
    conf: &State<Conf>,
) -> RatelimitedRouteResponse<FetchResponse<'a>> {
    let mut ratelimiter = Ratelimiter::new("fetch_file", "attachments", ip, conf.inner());
    ratelimiter.process_ratelimit(0, &mut cache).await?;
    let file = File::fetch_file(id, "attachments", &mut db)
        .await
        .map_err(|e| ratelimiter.wrap_response::<_, ()>(e).unwrap())?;
    ratelimiter.wrap_response(file)
}

#[get("/<id>/download", rank = 2)]
pub async fn fetch_download<'a>(
    id: u128,
    ip: ClientIP,
    mut cache: Connection<Cache>,
    mut db: Connection<DB>,
    conf: &State<Conf>,
) -> RatelimitedRouteResponse<FetchResponse<'a>> {
    let mut ratelimiter = Ratelimiter::new("fetch_file", "attachments", ip, conf.inner());
    ratelimiter.process_ratelimit(0, &mut cache).await?;
    let file = File::fetch_file_download(id, "attachments", &mut db)
        .await
        .map_err(|e| ratelimiter.wrap_response::<_, ()>(e).unwrap())?;
    ratelimiter.wrap_response(file)
}

#[get("/<id>/data", rank = 2)]
pub async fn fetch_data<'a>(
    id: u128,
    ip: ClientIP,
    mut cache: Connection<Cache>,
    mut db: Connection<DB>,
    conf: &State<Conf>,
) -> RatelimitedRouteResponse<Json<FileData>> {
    let mut ratelimiter = Ratelimiter::new("fetch_file", "attachments", ip, conf.inner());
    ratelimiter.process_ratelimit(0, &mut cache).await?;
    let file = File::fetch_file_data(id, "attachments", &mut db)
        .await
        .map_err(|e| ratelimiter.wrap_response::<_, ()>(e).unwrap())?;
    ratelimiter.wrap_response(Json(file))
}

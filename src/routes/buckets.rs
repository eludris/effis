use rocket::{form::Form, serde::json::Json, State};
use rocket_db_pools::Connection;
use todel::{
    http::ClientIP,
    ids::IDGenerator,
    models::{ErrorResponseData, FileData, ValidationError},
    Conf,
};
use tokio::sync::Mutex;

use crate::{
    models::{FetchResponse, File, FileUpload},
    ratelimit::{RatelimitedRouteResponse, Ratelimiter},
    Cache, BUCKETS, DB,
};

#[post("/<bucket>", data = "<upload>", rank = 2)]
pub async fn upload<'a>(
    bucket: &'a str,
    upload: Form<FileUpload<'a>>,
    ip: ClientIP,
    mut cache: Connection<Cache>,
    mut db: Connection<DB>,
    conf: &State<Conf>,
    gen: &State<Mutex<IDGenerator>>,
) -> RatelimitedRouteResponse<Json<FileData>> {
    let mut ratelimiter = Ratelimiter::new("attachments", bucket, ip, conf.inner());
    ratelimiter
        .process_ratelimit(upload.file.len() as u128, &mut cache)
        .await?;
    if !BUCKETS.contains(&bucket) {
        return Err(ratelimiter
            .wrap_response::<_, ()>(
                ValidationError {
                    field_name: "board".to_string(),
                    error: "Unknown bucket".to_string(),
                }
                .to_error_response(),
            )
            .unwrap());
    }
    let upload = upload.into_inner();
    let file = File::create(
        upload.file,
        bucket.to_string(),
        gen.inner(),
        &mut db,
        upload.spoiler,
    )
    .await
    .map_err(|e| ratelimiter.wrap_response::<_, ()>(e).unwrap())?;
    ratelimiter.wrap_response(Json(file))
}

#[get("/<bucket>/<id>", rank = 3)]
pub async fn fetch<'a>(
    bucket: &'a str,
    id: u128,
    ip: ClientIP,
    mut cache: Connection<Cache>,
    mut db: Connection<DB>,
    conf: &State<Conf>,
) -> RatelimitedRouteResponse<FetchResponse<'a>> {
    let mut ratelimiter = Ratelimiter::new("fetch_file", bucket, ip, conf.inner());
    ratelimiter.process_ratelimit(0, &mut cache).await?;
    if !BUCKETS.contains(&bucket) {
        return Err(ratelimiter
            .wrap_response::<_, ()>(
                ValidationError {
                    field_name: "board".to_string(),
                    error: "Unknown bucket".to_string(),
                }
                .to_error_response(),
            )
            .unwrap());
    }
    let file = File::fetch_file(id, bucket, &mut db)
        .await
        .map_err(|e| ratelimiter.wrap_response::<_, ()>(e).unwrap())?;
    ratelimiter.wrap_response(file)
}

#[get("/<bucket>/<id>/download", rank = 3)]
pub async fn fetch_download<'a>(
    bucket: &'a str,
    id: u128,
    ip: ClientIP,
    mut cache: Connection<Cache>,
    mut db: Connection<DB>,
    conf: &State<Conf>,
) -> RatelimitedRouteResponse<FetchResponse<'a>> {
    let mut ratelimiter = Ratelimiter::new("fetch_file", bucket, ip, conf.inner());
    ratelimiter.process_ratelimit(0, &mut cache).await?;
    if !BUCKETS.contains(&bucket) {
        return Err(ratelimiter
            .wrap_response::<_, ()>(
                ValidationError {
                    field_name: "board".to_string(),
                    error: "Unknown bucket".to_string(),
                }
                .to_error_response(),
            )
            .unwrap());
    }
    let file = File::fetch_file_download(id, bucket, &mut db)
        .await
        .map_err(|e| ratelimiter.wrap_response::<_, ()>(e).unwrap())?;
    ratelimiter.wrap_response(file)
}

#[get("/<bucket>/<id>/data", rank = 3)]
pub async fn fetch_data<'a>(
    bucket: &'a str,
    id: u128,
    ip: ClientIP,
    mut cache: Connection<Cache>,
    mut db: Connection<DB>,
    conf: &State<Conf>,
) -> RatelimitedRouteResponse<Json<FileData>> {
    let mut ratelimiter = Ratelimiter::new("fetch_file", bucket, ip, conf.inner());
    ratelimiter.process_ratelimit(0, &mut cache).await?;
    if !BUCKETS.contains(&bucket) {
        return Err(ratelimiter
            .wrap_response::<_, ()>(
                ValidationError {
                    field_name: "board".to_string(),
                    error: "Unknown bucket".to_string(),
                }
                .to_error_response(),
            )
            .unwrap());
    }
    let file = File::fetch_file_data(id, bucket, &mut db)
        .await
        .map_err(|e| ratelimiter.wrap_response::<_, ()>(e).unwrap())?;
    ratelimiter.wrap_response(Json(file))
}

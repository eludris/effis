use rocket::{form::Form, serde::json::Json, State};
use rocket_db_pools::Connection;
use todel::{http::ClientIP, ids::IDGenerator, models::FileData, Conf};
use tokio::sync::Mutex;

use crate::{
    models::{File, FileUpload},
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

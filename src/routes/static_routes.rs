use std::{io::ErrorKind, path::Path};

use crate::{
    ratelimit::{RatelimitedRouteResponse, Ratelimiter},
    Cache,
};
use rocket::{
    http::{ContentType, Header},
    State,
};
use rocket_db_pools::Connection;
use todel::{
    http::ClientIP,
    models::{
        ErrorResponse, ErrorResponseData, FetchResponse, NotFoundError, ServerError,
        ValidationError,
    },
    Conf,
};
use tokio::fs::File;

#[get("/static/<name>")]
pub async fn fetch_static_file<'a>(
    name: &'a str,
    ip: ClientIP,
    mut cache: Connection<Cache>,
    conf: &State<Conf>,
) -> RatelimitedRouteResponse<FetchResponse<'a>> {
    let mut ratelimiter = Ratelimiter::new("fetch_file", "static", ip, conf.inner());
    ratelimiter.process_ratelimit(0, &mut cache).await?;
    let path = Path::new(name).file_name().map(Path::new).ok_or_else(|| {
        ratelimiter
            .wrap_response::<_, ()>(
                ValidationError {
                    field_name: "name".to_string(),
                    error: "Could not find a valid file name".to_string(),
                }
                .to_error_response(),
            )
            .unwrap()
    })?;
    let extension = path.extension();
    let content_type = match extension {
        Some(extension) => ContentType::from_extension(extension.to_str().ok_or_else(|| {
            ratelimiter
                .wrap_response::<_, ()>(
                    ValidationError {
                        field_name: "name".to_string(),
                        error: "Invalid file extension".to_string(),
                    }
                    .to_error_response(),
                )
                .unwrap()
        })?),
        None => None,
    };
    let file = File::open(Path::new("./files/static").join(path))
        .await
        .map_err(|e| {
            if e.kind() == ErrorKind::NotFound {
                ratelimiter
                    .wrap_response::<_, ()>(NotFoundError.to_error_response())
                    .unwrap()
            } else {
                ratelimiter
                    .wrap_response::<_, ()>(
                        ServerError {
                            error: "Failed to upload file".to_string(),
                        }
                        .to_error_response(),
                    )
                    .unwrap()
            }
        })?;
    log::info!("Fetched static file {}", name);
    ratelimiter.wrap_response(FetchResponse {
        file,
        disposition: Header::new(
            "Content-Disposition",
            format!(
                "inline; filename=\"{}\"",
                path.file_name().unwrap().to_str().unwrap()
            ),
        ),
        content_type: content_type.unwrap_or(ContentType::Any),
    })
}

#[get("/static/<name>/download")]
pub async fn download_static_file<'a>(
    name: &'a str,
    ip: ClientIP,
    mut cache: Connection<Cache>,
    conf: &State<Conf>,
) -> RatelimitedRouteResponse<Result<FetchResponse<'a>, ErrorResponse>> {
    let mut ratelimiter = Ratelimiter::new("fetch_file", "static", ip, conf.inner());
    ratelimiter.process_ratelimit(0, &mut cache).await?;
    let path = Path::new(name).file_name().map(Path::new).ok_or_else(|| {
        ratelimiter
            .wrap_response::<_, ()>(
                ValidationError {
                    field_name: "name".to_string(),
                    error: "Could not find a valid file name".to_string(),
                }
                .to_error_response(),
            )
            .unwrap()
    })?;
    let extension = path.extension();
    let content_type = match extension {
        Some(extension) => ContentType::from_extension(extension.to_str().ok_or_else(|| {
            ratelimiter
                .wrap_response::<_, ()>(
                    ValidationError {
                        field_name: "name".to_string(),
                        error: "Invalid file extension".to_string(),
                    }
                    .to_error_response(),
                )
                .unwrap()
        })?),
        None => None,
    };
    let file = File::open(Path::new("./files/static").join(path))
        .await
        .map_err(|e| {
            if e.kind() == ErrorKind::NotFound {
                ratelimiter
                    .wrap_response::<_, ()>(NotFoundError.to_error_response())
                    .unwrap()
            } else {
                ratelimiter
                    .wrap_response::<_, ()>(
                        ServerError {
                            error: "Failed to upload file".to_string(),
                        }
                        .to_error_response(),
                    )
                    .unwrap()
            }
        })?;
    log::info!("Fetched static file {}", name);
    ratelimiter.wrap_response(Ok(FetchResponse {
        file,
        disposition: Header::new(
            "Content-Disposition",
            format!(
                "attachment; filename=\"{}\"",
                path.file_name().unwrap().to_str().unwrap()
            ),
        ),
        content_type: content_type.unwrap_or(ContentType::Any),
    }))
}

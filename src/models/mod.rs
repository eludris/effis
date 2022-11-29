#![allow(dead_code)] // TODO: remove this
use std::path::PathBuf;

use image::io::Reader as ImageReader;
use rocket::{
    fs::TempFile,
    http::{ContentType, Header},
};
use sqlx::{pool::PoolConnection, MySql};
use todel::models::{ErrorResponseData, FileData, FileMetadata, NotFoundError};
use todel::{ids::IDGenerator, models::ErrorResponse};
use tokio::sync::Mutex;

pub struct File {
    pub id: u128,
    pub file_id: u128,
    pub name: String,
    pub content_type: String,
    pub hash: String,
    pub bucket: String,
    pub spoiler: bool,
    pub width: Option<usize>,
    pub height: Option<usize>,
}

#[derive(Debug, Responder)]
pub struct FetchResponse<'a> {
    pub file: tokio::fs::File,
    pub disposition: Header<'a>,
    pub content_type: ContentType,
}

impl File {
    pub async fn create<'a>(
        mut file: TempFile<'a>,
        bucket: String,
        gen: &Mutex<IDGenerator>,
        db: &mut PoolConnection<MySql>,
        spoiler: bool,
    ) -> Result<Self, ErrorResponse> {
        let id = gen.lock().await.generate_id();
        let path = PathBuf::from(format!("./data/{}", id));
        let name = file.name().unwrap().to_string();
        file.persist_to(&path).await.unwrap();
        let data = tokio::fs::read(&path).await.unwrap();

        let hash = sha256::digest(&data[..]);
        let file = if let Ok((file_id, content_type, width, height)) = sqlx::query!(
            "
SELECT file_id, content_type, width, height
FROM files
WHERE hash = ?
            ",
            hash,
        )
        .fetch_one(&mut *db)
        .await
        .map(|f| (f.file_id, f.content_type, f.width, f.height))
        {
            tokio::fs::remove_file(path).await.unwrap();
            sqlx::query!(
                "
INSERT INTO files(id, file_id, name, content_type, hash, bucket, spoiler, width, height)
VALUES(?, ?, ?, ?, ?, ?, ?, ?, ?)
                ",
                id.to_string(),
                file_id,
                name,
                content_type,
                hash,
                bucket,
                spoiler,
                width,
                height
            )
            .execute(&mut *db)
            .await
            .unwrap();

            Self {
                id,
                file_id: file_id.parse::<u128>().unwrap(),
                name,
                content_type,
                hash,
                bucket,
                spoiler,
                width: width.map(|s| s as usize),
                height: height.map(|s| s as usize),
            }
        } else {
            let file = tokio::task::spawn_blocking(move || {
                let mime = tree_magic::from_u8(&data);
                let (width, height) = match mime.as_ref() {
                    "image/gif" | "image/jpeg" | "image/png" | "image/webp" => {
                        if mime == "image/jepg" {
                            // if something fails here we *want* the server to 500
                            ImageReader::open(&path)
                                .unwrap()
                                .decode()
                                .unwrap()
                                .save(&path)
                                .unwrap();
                        }
                        imagesize::blob_size(&data)
                            .map(|d| (Some(d.width), Some(d.height)))
                            .unwrap_or((None, None))
                    }
                    "video/mp4" | "video/webm" | "video/quicktime" => {
                        let mut dimensions = (None, None);
                        for stream in ffprobe::ffprobe(&path).unwrap().streams.iter() {
                            if let (Some(width), Some(height)) = (stream.width, stream.height) {
                                dimensions = (Some(width as usize), Some(height as usize));
                            }
                        }
                        dimensions
                    }
                    _ => (None, None),
                };
                Self {
                    id,
                    file_id: id,
                    name,
                    content_type: mime,
                    hash,
                    bucket,
                    spoiler,
                    width,
                    height,
                }
            })
            .await
            .unwrap();
            sqlx::query!(
                "
INSERT INTO files(id, file_id, name, content_type, hash, bucket, spoiler, width, height)
VALUES(?, ?, ?, ?, ?, ?, ?, ?, ?)
                ",
                file.id.to_string(),
                file.id.to_string(),
                file.name,
                file.content_type,
                file.hash,
                file.bucket,
                file.spoiler,
                file.width.map(|s| s as u32),
                file.height.map(|s| s as u32),
            )
            .execute(&mut *db)
            .await
            .unwrap();

            file
        };

        Ok(file)
    }

    pub async fn get(id: u128, db: &mut PoolConnection<MySql>) -> Option<Self> {
        sqlx::query!(
            "
SELECT *
FROM files
WHERE id = ?
            ",
            id.to_string(),
        )
        .fetch_one(&mut *db)
        .await
        .map(|r| Self {
            id: r.id.parse().unwrap(),
            file_id: r.id.parse().unwrap(),
            name: r.name,
            content_type: r.content_type,
            hash: r.hash,
            bucket: r.bucket,
            spoiler: r.spoiler == 1,
            width: r.width.map(|s| s as usize),
            height: r.height.map(|s| s as usize),
        })
        .ok()
    }

    pub async fn fetch_file<'a>(
        id: u128,
        db: &mut PoolConnection<MySql>,
    ) -> Result<FetchResponse<'a>, ErrorResponse> {
        let file_data = Self::get(id, db)
            .await
            .ok_or_else(|| NotFoundError.to_error_response())?;
        let file = tokio::fs::File::open(format!("data/{}", file_data.id))
            .await
            .unwrap();
        Ok(FetchResponse {
            file,
            disposition: Header::new(
                "Content-Disposition",
                format!("inline; filename=\"{}\"", file_data.name),
            ),
            content_type: ContentType::parse_flexible(&file_data.content_type).unwrap(),
        })
    }

    pub async fn fetch_file_download<'a>(
        id: u128,
        db: &mut PoolConnection<MySql>,
    ) -> Result<FetchResponse<'a>, ErrorResponse> {
        let file_data = Self::get(id, db)
            .await
            .ok_or_else(|| NotFoundError.to_error_response())?;
        let file = tokio::fs::File::open(format!("data/{}", file_data.id))
            .await
            .unwrap();
        Ok(FetchResponse {
            file,
            disposition: Header::new(
                "Content-Disposition",
                format!("attachment; filename=\"{}\"", file_data.name),
            ),
            content_type: ContentType::parse_flexible(&file_data.content_type).unwrap(),
        })
    }

    pub async fn fetch_file_data(
        id: u128,
        db: &mut PoolConnection<MySql>,
    ) -> Result<FileData, ErrorResponse> {
        let file = Self::get(id, db)
            .await
            .ok_or_else(|| NotFoundError.to_error_response())?;
        let metadata = if file.width.is_some() && file.height.is_some() {
            match file.content_type.as_ref() {
                "image/gif" | "image/jpeg" | "image/png" | "image/webp" => FileMetadata::Image {
                    width: file.width,
                    height: file.height,
                },
                // TODO: get video width and height
                "video/mp4" | "video/webm" | "video/quicktime" => FileMetadata::Image {
                    width: file.width,
                    height: file.height,
                },
                _ if file.content_type.starts_with("text") => FileMetadata::Text,
                _ => FileMetadata::Other,
            }
        } else {
            FileMetadata::Other
        };

        Ok(FileData {
            id: file.id,
            name: file.name,
            bucket: file.bucket,
            metadata,
            spoiler: file.spoiler,
        })
    }
}

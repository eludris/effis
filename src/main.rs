#[macro_use]
extern crate rocket;

use rocket::{
    form::Form,
    fs::TempFile,
    http::{ContentType, Header},
    serde::{json::Json, Deserialize, Serialize},
    tokio::{
        fs::{create_dir, read_dir, File},
        sync::Mutex,
    },
    Build, Rocket, State,
};
use std::{env, path::Path};
use todel::ids::{generate_instance_id, IDGenerator};

#[derive(Debug, FromForm)]
struct FileData<'a> {
    name: String,
    file: TempFile<'a>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
enum UploadResponse {
    FileTooBig { max_size: u32 },
    ErrorUploadingFile,
    Sucess { id: u64 },
}

#[post("/upload", data = "<data>")]
async fn upload(
    data: Form<FileData<'_>>,
    generator: &State<Mutex<IDGenerator>>,
) -> Json<UploadResponse> {
    let mut data = data.into_inner();
    // 100MB
    Json(if data.file.len() < 100_000_000 {
        let id = generator.lock().await.generate_id();
        match create_dir(format!("files/{}", id)).await {
            Ok(_) => {
                match data
                    .file
                    .move_copy_to(format!("files/{}/{}", id, data.name))
                    .await
                {
                    Ok(_) => {
                        log::info!("New Upload with id {}", id);
                        UploadResponse::Sucess { id }
                    }
                    Err(err) => {
                        log::warn!("Couldn't upload file: {:?}", err);
                        UploadResponse::ErrorUploadingFile
                    }
                }
            }
            Err(err) => {
                log::warn!("Couldn't upload file: {:?}", err);
                UploadResponse::ErrorUploadingFile
            }
        }
    } else {
        log::warn!("Got an upload that exeeced the size limit");
        UploadResponse::FileTooBig {
            max_size: 10_000_000,
        }
    })
}

#[derive(Debug, Responder)]
struct FetchResponse<'a> {
    file: File,
    disposition: Header<'a>,
    content_type: ContentType,
}

#[get("/<id>")]
async fn fetch<'a>(id: u64) -> Result<FetchResponse<'a>, String> {
    let files = Path::new("files").join(id.to_string());
    let filepath = read_dir(files)
        .await
        .map_err(|_| "Server failed to retrieve file")?
        .next_entry()
        .await
        .map_err(|_| "Server failed to retrieve file")?
        .ok_or("File not found")?
        .path();
    let filename = filepath
        .file_name()
        .ok_or("Server failed to retrieve file")?
        .to_str()
        .ok_or("Server failed to retrieve file")?;
    log::info!("Fetched file with id {}", id);
    let file = File::open(&filepath).await.map_err(|_| "File not found")?;
    Ok(FetchResponse {
        file,
        disposition: Header::new(
            "Content-Disposition",
            format!("inline; filename=\"{}\"", filename),
        ),
        content_type: ContentType::from_extension(
            filename
                .split('.')
                .last()
                .ok_or("Server failed to retrieve file")?,
        )
        .ok_or("Server failed to retrieve file")?,
    })
}

#[launch]
fn rocket() -> Rocket<Build> {
    dotenv::dotenv().ok();
    env_logger::init();

    let instance_name =
        env::var("INSTANCE_NAME").expect("Couldn't find the \"INSTANCE_NAME\" environment varable");
    let instance_id = generate_instance_id(&instance_name);
    let generator = IDGenerator::new(instance_id);
    let generator = Mutex::new(generator);

    rocket::build()
        .mount("/", routes![upload, fetch])
        .manage(generator)
}

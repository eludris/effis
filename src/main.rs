#[macro_use]
extern crate rocket;

use rocket::{
    form::Form,
    fs::TempFile,
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
                    .persist_to(format!("files/{}/{}", id, data.name))
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
            max_size: 100_000_000,
        }
    })
}

#[get("/<id>")]
async fn fetch(id: u64) -> Option<File> {
    let files = Path::new("files").join(id.to_string());
    let filename = read_dir(files)
        .await
        .unwrap()
        .next_entry()
        .await
        .unwrap()
        .unwrap()
        .path();
    log::info!("Fetched file with id {}", id);
    File::open(&filename).await.ok()
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

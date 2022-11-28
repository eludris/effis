#[macro_use]
extern crate rocket;

mod cors;
mod models;
mod ratelimit;
mod routes;

use std::env;

use anyhow::{anyhow, Context};

use rocket::{
    data::{ByteUnit, Limits, ToByteUnit},
    tokio::sync::Mutex,
    Build, Config, Rocket,
};
use rocket_db_pools::{deadpool_redis::Pool, sqlx::MySqlPool, Database};
use todel::{
    ids::{generate_instance_id, IDGenerator},
    Conf,
};

#[derive(Database)]
#[database("db")]
pub struct DB(MySqlPool);

#[derive(Database)]
#[database("cache")]
pub struct Cache(Pool);

fn rocket() -> Result<Rocket<Build>, anyhow::Error> {
    #[cfg(test)]
    {
        env::set_var("ELUDRIS_CONF", "tests/Eludris.toml");
        dotenvy::dotenv().ok();
        env_logger::init().ok();
    }

    let conf = Conf::new_from_env()?;

    conf.effis
        .file_size
        .parse::<ByteUnit>()
        .map_err(|err| anyhow!("{}", err))
        .with_context(|| format!("Invalid file size limit {}", conf.effis.file_size))?;
    conf.effis
        .ratelimit
        .file_size_limit
        .parse::<ByteUnit>()
        .map_err(|err| anyhow!("{}", err))
        .with_context(|| format!("Invalid ratelimit file size limit {}", conf.effis.file_size))?;

    let config = Config::figment()
        .merge((
            "limits",
            Limits::default()
                .limit("data-form", 20.mebibytes())
                .limit("file", 20.mebibytes()),
        ))
        .merge(("temp_dir", "./data"))
        .merge((
            "databases.db",
            rocket_db_pools::Config {
                url: env::var("DATABASE_URL")
                    .unwrap_or_else(|_| "mysql://root:root@localhost:3306/eludris".to_string()),
                min_connections: None,
                max_connections: 1024,
                connect_timeout: 3,
                idle_timeout: None,
            },
        ))
        .merge((
            "databases.cache",
            rocket_db_pools::Config {
                url: env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string()),
                min_connections: None,
                max_connections: 1024,
                connect_timeout: 3,
                idle_timeout: None,
            },
        ));

    Ok(rocket::custom(config)
        .manage(Mutex::new(IDGenerator::new(generate_instance_id())))
        .manage(conf)
        .attach(DB::init())
        .attach(Cache::init())
        .attach(cors::Cors)
        .mount("/", routes::routes()))
}

#[rocket::main]
async fn main() -> Result<(), anyhow::Error> {
    dotenvy::dotenv().ok();
    env_logger::init();

    let db_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "mysql://root:root@localhost:3306/eludris".to_string());

    let pool = MySqlPool::connect(&db_url)
        .await
        .with_context(|| format!("Failed to connect to database on {}", db_url))?;
    sqlx::migrate!()
        .run(&pool)
        .await
        .context("Failed to run migrations")?;

    let _ = rocket()?
        .launch()
        .await
        .context("Encountered an error while running Rest API")?;

    Ok(())
}

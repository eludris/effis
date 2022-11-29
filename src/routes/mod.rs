mod static_routes;

use rocket::Route;

pub fn routes() -> Vec<Route> {
    routes![
        static_routes::fetch_static_file,
        static_routes::download_static_file
    ]
}

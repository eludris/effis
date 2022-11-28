use rocket::Route;

#[get("/")]
pub async fn index() -> &'static str {
    "h"
}

pub fn routes() -> Vec<Route> {
    routes![index]
}

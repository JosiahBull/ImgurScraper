//Global Config
const SERVER_IP: &str = "mongodb://localhost:27017";

//Imports
mod mongo_db_interface;
mod imgur_interface;
mod filter;

use warp::{http, Filter, http::Response};
use crate::mongo_db_interface::{Database, Post};
use crate::imgur_interface::Downloader;

///An api endpoint. Takes a post from the api, and then returns the data in that post. Will OCR scan, and apply filtering if required.
async fn process_posts_to_queue(new_post: Post, db: Database) -> Result<impl warp::Reply, warp::Rejection> {
    println!("Request received: {}", new_post.post_url);
    let document: Post = match db.get_post(&new_post.id).await {
        Ok(data) => {
            //The post already exists in the database, so return the information we already need.
            data
        },
        Err(e) if e.to_string() == "Failed to find document" => {
            //The post does not exist in the database, or something went wrong.
            let downloader = Downloader::new(&new_post.id, db.clone());
            let post = match downloader.get_post().await {
                Ok(data) => data,
                Err(e) => {
                    //Unknown error occured.
                    println!("A serious error has occured in the database: {:?}", e);
                    let response = Response::builder()
                        .status(http::StatusCode::from_u16(500).unwrap())
                        .body("Database Error(2)".to_owned());
                    return Ok(response);
                }
            };

            let download = downloader.download_post_images(post).await;
            if download.is_err() {
                println!("A serious error has occured in the database: {}", e);
                let response = Response::builder()
                    .status(http::StatusCode::from_u16(500).unwrap())
                    .body("Database Error(3)".to_owned());
                return Ok(response);
            }
            download.unwrap()
        },
        Err(e) => {
            //Unknown error occured.
            println!("A serious error has occured in the database: {}", e);
            let response = Response::builder()
                .status(http::StatusCode::from_u16(500).unwrap())
                .body("Database Error(4)".to_owned());
            return Ok(response);
        }
    };

    let response = Response::builder()
        .status(http::StatusCode::from_u16(200).unwrap())
        .body(serde_json::to_string(&document).unwrap());

    Ok(response)
}

//Json Parsers

///Parses the input json to a struct that the internal program can use. If it fails returns 403 bad request along with info to the user.
fn authenticate_post() -> impl Filter<Extract = (Post,), Error = warp::Rejection> + Clone {
    warp::body::content_length_limit(1024 * 16).and(warp::body::json())
}

//Main
#[tokio::main]
async fn main() -> () {
    let db = Database::new(SERVER_IP).await.expect("Failed to init database.");


    let cors = warp::cors()
        .allow_any_origin()
        .allow_headers(vec!["User-Agent", "Sec-Fetch-Mode", "Referer", "Origin", "Access-Control-Request-Method", "Access-Control-Request-Headers", "Content-Type"])
        .allow_methods(vec!["POST", "GET", "OPTIONS"]);

    let check_post = warp::post()
        .and(warp::path("check_post_priority"))
        .and(warp::path::end())
        .and(authenticate_post())
        .and(warp::any().map(move || db.clone()))
        .and_then(|info, db| {
            process_posts_to_queue(info, db)
        });

    let routes = check_post.with(cors);

    warp::serve(routes)
        .tls()
        .cert_path("/home/ubuntu/PersonalProjects/0015_ImgurScraper/extension_contact_server/src/certs/cert.pem")
        .key_path("/home/ubuntu/PersonalProjects/0015_ImgurScraper/extension_contact_server/src/certs/key1.rsa")
        .run(([0, 0, 0, 0], 3030))
        .await;
}
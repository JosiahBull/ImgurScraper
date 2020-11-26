//Global Config
const SERVER_IP: &str = "mongodb://localhost:27017";
const TEMP_IMAGE_STORAGE: &str = "example/";

//Imports
use serde::Deserialize;
use serde::Serialize;
use reqwest::header::USER_AGENT;
use tokio;
use std::fs::File;
use std::io::prelude::*;
use regex::Regex;
use std::time::{SystemTime, UNIX_EPOCH};
use img_hash::{HasherConfig, HashAlg::Gradient};
use image as image_reader;
use mongodb::{Client, options::ClientOptions, bson::{doc, Bson}, bson};
use std::error;
use std::{thread, time};
use std::fs;
use std::error::Error;
use std::io::{Error as CustomError, ErrorKind};
use leptess;

//Struct Declarations
#[derive(Deserialize, Debug, Clone)]
struct Image {
    id: String,
    title: Option<String>,
    description: Option<String>,
    width: u32,
    height: u32,
    views: u64,
    size: u64,
    bandwidth: u64,
    link: String
}

#[derive(Deserialize, Debug, Clone)]
struct Tag {
    name: String,
    display_name: String,
    followers: u64,
    total_items: u64,
    following: bool,
    description: Option<String>
}

#[derive(Deserialize, Debug, Clone)]
struct Post {
    id: String,
    title: Option<String>,
    description: Option<String>,
    datetime: u64,
    account_url: String,
    views: u32,
    link: String,
    ups: u32,
    downs: u32,
    is_album: bool,
    vote: Option<bool>,
    favourite: Option<bool>,
    nsfw: Option<bool>,
    comment_count: u32,
    favourite_count: Option<u32>,
    images_count: Option<u32>,
    is_ad: bool,
    images: Option<Vec<Image>>,
    tags: Vec<Tag>
}

#[derive(Deserialize)]
struct Response {
    data: Vec<Post>,
}

#[derive(Clone)]
struct ImageDownloadCard {
    url: String,
    id: String,
    parent_id: String,
    date_queued: u128,
    date_downloaded: u128,
    downloaded: bool,
    errored: bool,
    error: String,
    path: String,
    hash: String,
    extension: String,
    text: String,
}

impl ImageDownloadCard {
    fn new(id: &String, url: String, parent_id: &String) -> ImageDownloadCard {
        let mut output = ImageDownloadCard {
            id: (&id).to_string(),
            parent_id: (&parent_id).to_string(),
            url: url,
            date_queued: get_time(),
            date_downloaded: 0,
            downloaded: false,
            error: "".to_owned(),
            errored: true,
            path: "".to_owned(),
            hash: "".to_owned(),
            extension: "".to_owned(),
            text: "".to_owned()
        };
        output.extension = output.get_extension().unwrap_or("").to_owned();
        return output;
    }
    fn get_extension(&self) -> Result<&str, Box<dyn Error>> {

        let regex = match Regex::new(r".[0-9a-z]+$")?.captures(&self.url) {
            Some(res) => res,
            None => return Err(Box::new(CustomError::new(ErrorKind::NotFound, "No regex capture found. (1)"))),
        };
        let extension = match regex.get(0) {
            Some(res) => res,
            None => return Err(Box::new(CustomError::new(ErrorKind::NotFound, "No regex capture found. (2)"))),
        }.as_str();

        Ok(extension)
    }

}

#[derive(Serialize, Deserialize, Debug)]
struct Status {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    id: Option<bson::oid::ObjectId>,
    viewed_posts: i64,
    remaining_posts: i64,
    to_store: i64,
}

//Functions
fn get_time() -> u128 {
    SystemTime::now().duration_since(UNIX_EPOCH).expect("Time went backwards").as_millis()
}

async fn request_posts(client: reqwest::Client, page: u16) -> Result<Vec<Post>, Box<dyn Error>> {
    let url = format!("https://api.imgur.com/3/gallery/hot/viral/day/{}?showViral=true&mature=true&album_previews=true", page);
    let response = client
        .get(&url)
        .header(USER_AGENT, "PostmanRuntime/7.26.5")
        .header("Authorization", "Client-ID <>")
        .header("Accept", "*/*")
        .header("Connection", "keep-alive")
        .send()
        .await?;

    let result = response.text().await?;
    let v: Response = serde_json::from_str(&*result).unwrap(); //Todo: Implement error catching here.
    
    Ok(v.data)
}

fn process_posts(posts: Vec<Post>) -> Vec<ImageDownloadCard> {
    //Take a vector of posts, and process them into a vector of images to be downloaded.
    let mut result: Vec<ImageDownloadCard> = vec![];

    for post in posts {
        if post.is_album && post.images.is_some() {
            for image in post.images.unwrap() {
                result.push(ImageDownloadCard::new(&image.id, image.link, &post.id));
            }
        } else {
            result.push(ImageDownloadCard::new(&post.id, post.link, &post.id));
        }
    }

    return result;
}

async fn download_image(mut image: ImageDownloadCard, client: reqwest::Client, path: String) -> Result<ImageDownloadCard, ImageDownloadCard> {
    //Take an image link, and where to store the file. Download the image to that path. Should return a result with the filepath if succesful, or an error if not.
    //By default assume an error is going to occur, then we can correct it at the end.
    image.date_downloaded = get_time();
    // println!("Downloading image.");
    let response = match client
        .get(&image.url)
        .send()
        .await {
            Ok(res) => res,
            Err(e) => {
                image.error = format!("{:?}", e);
                return Err(image);
            },
    };

    let filename = format!("{}{}{}", path, image.id, image.extension);

    let mut file_out = match File::create(filename.clone()) {
        Ok(res) => res,
        Err(e) => {
            image.error = format!("{:?}", e);
            return Err(image);
        },
    };
    // println!("Saving image.");
    let output_file = match response.bytes().await {
        Ok(res) => res,
        Err(e) => {
            image.error = format!("{:?}", e);
            return Err(image);
        },
    };

    match file_out.write_all(&output_file) {
        Ok(res) => res,
        Err(e) => {
            image.error = format!("{:?}", e);
            return Err(image);
        },
    };

    image.errored = false;
    image.downloaded = true;
    image.path = filename;
    return Ok(image);
}

fn hash_image(mut image: ImageDownloadCard) -> Result<ImageDownloadCard, ImageDownloadCard> {
    //TODO: Add functionality to handle hashing of mp4 images, for now just skip them

    //Generate a hash based on the image, using the graident algorithm. This will be used for comparisons later.
    println!("Hashing image.");
    let hasher = HasherConfig::new().hash_alg(Gradient).to_hasher();
    
    let loaded_image = match image_reader::open(image.path.clone()) {
        Ok(res) => res,
        Err(e) => {
            image.errored = true;
            image.error = format!("{:?}", e);
            return Err(image);
        },
    };

    image.hash = hasher.hash_image(&loaded_image).to_base64();

    return Ok(image);
}

fn scan_image(image: &ImageDownloadCard, mut scanner: leptess::LepTess) -> String {
    if image.extension == ".mp4" {
        return "This is an Mp4, cannot process.".to_owned();
    }
    scanner.set_image(&image.path).unwrap();
    return scanner.get_utf8_text().unwrap_or("".to_owned());
}


async fn delete_image(image: &ImageDownloadCard) {
    let result = fs::remove_file(&image.path);

    if result.is_err() {
        println!("Error deleting file!\n{:?}", result);
    }
}

async fn upload_images(images_database: mongodb::Collection, images: Vec<ImageDownloadCard>) {
    //Takes a mongodb object.
    //Takes all previous information about the image, and uploads it to the mongodb server ready for servicing. 
    //Should also update any relevant information in the database regarding number of available posts, etc.
    let mut results: Vec<mongodb::bson::Document> = vec![];
    for image in images {
        let new_doc = doc! {
            "url": image.url,
            "id": image.id,
            "dateQueued": image.date_queued.to_string(),
            "dateDownloaded": image.date_downloaded.to_string(),
            "downloaded": image.downloaded,
            "errored": image.errored,
            "error": image.error,
            "path": image.path,
            "hash": image.hash,
            "viewed": false,
            "valid": true,
            "checked": false
        };
        results.push(new_doc);
    }
    images_database.insert_many(results, None).await.unwrap();
}

async fn upload_posts(post_database: mongodb::Collection, posts: Vec<Post>) {
    let mut results: Vec<mongodb::bson::Document> = vec![];
    for post in posts {
        let mut images: Vec<mongodb::bson::Document> = vec![];
        let mut tags: Vec<mongodb::bson::Document> = vec![];
        if post.is_album && post.images.is_some() {
            for image in post.images.unwrap() {
                let new_image_doc = doc! {
                    "id": image.id,
                    "title": image.title.unwrap_or("".to_owned()),
                    "description": image.description.unwrap_or("".to_owned()),
                    "width": image.width,
                    "height": image.height,
                    "views": image.views,
                    "size": image.size,
                    "bandwidth": image.bandwidth,
                    "link": image.link
                };
                images.push(new_image_doc);
            }
        }
        if post.tags.len() > 0 {
            for tag in post.tags {
                let new_tag_doc = doc! {
                    "name": tag.name,
                    "display_name": tag.display_name,
                    "followers": tag.followers,
                    "total_items": tag.total_items,
                    "following": tag.following,
                    "description": tag.description.unwrap_or("".to_owned())
                };
                tags.push(new_tag_doc);
            }
        }
        let new_doc = doc! {
            "id": post.id,
            "title": post.title.unwrap_or("".to_owned()),
            "description": post.description.unwrap_or("".to_owned()),
            "datetime": post.datetime,
            "account_url": post.account_url,
            "views": post.views,
            "link": post.link,
            "ups": post.ups,
            "downs": post.downs,
            "is_album": post.is_album,
            "vote": post.vote.unwrap_or(false),
            "favourite": post.favourite.unwrap_or(false),
            "nsfw": post.nsfw.unwrap_or(false),
            "comment_count": post.comment_count,
            "favourite_count": post.favourite_count.unwrap_or(0),
            "images_count": post.images_count.unwrap_or(0),
            "id_ad": post.is_ad,
            "images": images,
            "tags": tags
        };
        results.push(new_doc);
    }
    post_database.insert_many(results, None).await.unwrap();
}

async fn update_database_info(num_posts: u32, status_database: mongodb::Collection) -> bool {
    let filter = doc! {};
    let update = doc! {"$inc": {"remaining_posts": num_posts}};
    match status_database.update_one(filter, update, None).await {
        Ok(_) => return true,
        Err(_) => return false,
    };
}

async fn connect_database() -> Result<mongodb::Database, Box<dyn error::Error>> {
    let client_options = ClientOptions::parse(SERVER_IP).await?;

    let client = Client::with_options(client_options)?;

    let db = client.database("imgur_scraper");

    Ok(db)
}

async fn refresh_database_info(status_database: mongodb::Collection) -> Option<Status>{
    let filter = doc! {};
    let result = match status_database.find_one(filter, None).await {
        Ok(_document) => _document.unwrap(),
        Err(_) => return None,
    };

    let result: Status = match bson::from_bson(Bson::Document(result)) {
        Ok(res) => res,
        Err(_) => return None,
    };

    return Some(result);
}

#[tokio::main]
async fn main() -> () {
    
    let client = reqwest::Client::new();
    let database = connect_database().await.unwrap(); //Todo: If this fails we do actually want to panic.

    let images_database = database.collection("images");
    let posts_database = database.collection("posts");
    let status_database = database.collection("admin");

    let mut start_time_counter = get_time();
    let mut page_number = 0;
    println!("Server starting.");
    loop {
        println!("Loop starting.");
        if let Some(current_status) = refresh_database_info(status_database.clone()).await {
            //We have recieved updated data on how we should continue 
            if current_status.remaining_posts - current_status.to_store < 50 {
                if get_time() - start_time_counter > 12 * 60 * 60 * 1000 {
                    page_number = 1;
                    start_time_counter = get_time();
                } else {
                    page_number += 1;
                }
                println!("Requesting page {}", page_number);

                let posts = match request_posts(client.clone(), page_number).await {
                    Ok(res) => res,
                    Err(_e) => continue,
                };
                let num_posts = posts.len() as u32;
                let mut images = process_posts(posts.clone());

                println!("Downloading images.");
                //Download and Process Images
                for i in 0..images.len()-1 {
                    //Download
                    let mut error_count: u8 = 0;
                    let mut succeded: bool = false;
                    while !succeded && error_count <= 5 {
                        println!("Attempting download: {} of image {} out of {}.", error_count, i, images.len());
                        images[i] = match download_image(images[i].clone(), client.clone(), TEMP_IMAGE_STORAGE.to_owned()).await {
                            Ok(res) => {
                                succeded = true;
                                res
                            },
                            Err(e) => {
                                error_count += 1;
                                thread::sleep(time::Duration::from_millis(1000));
                                e
                            }, 
                        };
                    };
                    if images[i].errored {
                        println!("Error occurred, removing offending image at {}. Relevant error: {:?}", images[i].path, images[i].error);
                        images.swap_remove(i);
                        delete_image(&images[i]).await;
                    } else {
                        //Hash
                        images[i] = match hash_image(images[i].clone()) {
                            Ok(res) => res,
                            Err(e) => {
                                println!("Error occurred, removing offending image at {}. Relevant error: {:?}", e.path, e.error);
                                images.swap_remove(i);
                                delete_image(&images[i]).await;
                                e
                            }
                        };
                        if !images[i].errored {
                            //Analysis
                            println!("Running Analysis.");
                            let tesseract_scanner = leptess::LepTess::new(Some("./tessdata"), "eng").unwrap();
                            images[i].text = scan_image(&images[i], tesseract_scanner);
                            

                            //Delete
                            println!("Deleting image.");
                            delete_image(&images[i]).await;
                        }
                    }
                }

                println!("Uploading image data to db.");
                //Upload Images
                upload_images(images_database.clone(), images).await; //Add error checking for this

                println!("Uploading posts to db.");
                //Upload Posts
                upload_posts(posts_database.clone(), posts).await; //Add error checking for this

                println!("Updating db counter.");
                //Update counter
                update_database_info(num_posts, status_database.clone()).await; //TODO: Allow retrys until this succeeds.
            }
        }
        thread::sleep(time::Duration::from_millis(15000));
    } 
}
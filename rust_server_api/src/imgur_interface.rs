///This module handles connections to the imgur api and website.

//Imports
use std::cmp::min;
use std::path::PathBuf;
use bytes::BufMut;
use hyper::{ body::HttpBody as httpbody, client::ResponseFuture, Client, Uri };
use hyper_tls::HttpsConnector;
use tokio::{
    fs::{ create_dir_all, File },
    prelude::*
};
use url::Url;
use futures::TryFutureExt;
use std::error;
use std::path::Path;
use serde::{Deserialize};
use reqwest::header::USER_AGENT;
use anyhow::{Result, bail};
use crate::mongo_db_interface::Database;
use async_std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

///Config
const DEFAULT_MAX_CONNECTION: usize = 10;
const UNRECOVERABLE_THRESHOLD: f32 = 0.2;

///This struct holds data needed to request images from the imgur api.
#[derive(Deserialize, Debug, Clone)]
pub struct Image {
    id: String,
    title: Option<String>,
    description: Option<String>,
    link: String
}

///This struct models the data from the imgur api response.
#[derive(Deserialize, Debug, Clone)]
pub struct Post {
    id: String,
    title: Option<String>,
    description: Option<String>,
    datetime: u64,
    account_url: Option<String>,
    views: u32,
    link: String,
    is_album: bool,
    nsfw: Option<bool>,
    images_count: Option<u32>,
    is_ad: bool,
    images: Vec<Image>
}

///The response received from the imgur api.
#[derive(Deserialize, Debug)]
struct Response {
    data: Post,
}

///This models the imgur api response when only a single image is present.
#[derive(Deserialize, Clone, Debug)]
struct ImageRaw {
    id: String,
    title: Option<String>,
    description: Option<String>,
    datetime: u64,
    account_url: Option<String>,
    views: u32,
    link: String,
    nsfw: Option<bool>,
    is_ad: bool,
}

///This models the response received from the imgur api for a single image.
#[derive(Deserialize, Clone, Debug)]
struct ResponseImage {
    data: ImageRaw,
}

///This struct is the main function of this module. It is a downloader to acquire images from an imgur post.
pub struct Downloader {
    post_id: Uri,
    save_path: PathBuf,
    max_conn: usize,
    db: Database
}

///Creates and returns a filename from a url.
fn create_filename(url: &str) -> Result<String, Box<dyn error::Error>> {
    let tmp = &Url::parse(&url)?;
    let res = &tmp.path();
    let path = format!("{}", res[1..res.len()].to_owned());
    Ok(path)
}

///Collects the current system time.
fn get_time() -> u128 {
    SystemTime::now().duration_since(UNIX_EPOCH).expect("Time went backwards").as_millis()
}

impl Downloader {
    ///Generates a new downloader.
    pub fn new(post_id: &str, db: Database) -> Self {
        Downloader {
            post_id: post_id.parse::<Uri>().unwrap_or_else(|_| panic!("failed to parse URL: {}", post_id)),
            save_path: Path::new(post_id).to_path_buf(),
            max_conn: DEFAULT_MAX_CONNECTION,
            db: db
        }
    }
    ///The function which recieves the bytes when downloading an image.
    async fn recv(&self, fut: ResponseFuture) -> Result<bytes::BytesMut, anyhow::Error> {
        let mut buf = bytes::BytesMut::new();
        let mut res = fut.await?;

        while let Some(next) = res.data().await {
            let chunk = next?;
            buf.put(chunk);
        }

        Ok(buf)
    }
    ///Creates a downloader module to acquire the image. Returns a string with the url, and the downloaded image bytes.
    async fn get_downloader(&self, url: Uri) -> Result<(String, bytes::BytesMut), anyhow::Error> {
        let https = HttpsConnector::new();
        let client = Client::builder().build::<_, hyper::Body>(https);

        let content = self.recv(client.get(url.clone())).await?;

        Ok((url.to_string(), content))
    }
    ///Takes a url and the downloaded bytes of an image, and saves it to the drive.
    async fn download(&self, url: String, res: bytes::BytesMut) -> anyhow::Result<PathBuf> {
        let file_name = create_filename(&url).unwrap();

        create_dir_all(&self.save_path).await.unwrap();

        let save_path = self.save_path.join(&file_name);

        let mut file = File::create(&save_path).await.unwrap();
        file.write_all(&res).await.unwrap();
        Ok(save_path)
    }
    ///Takes an image path and scans it with tesseract OCR, returns any text it finds in the form of a string.
    async fn scan_image(&self, path: PathBuf) -> anyhow::Result<String>{
        let mut scanner = leptess::LepTess::new(Some("/home/ubuntu/PersonalProjects/0015_ImgurScraper/extension_contact_server/tessdata"), "eng").expect("Failed to load OCR Scanner.");
        scanner.set_image(&path).expect("Failed to set image for OCR scanner.");

        scanner.set_fallback_source_resolution(70);
        Ok(scanner.get_utf8_text().expect("Failed to get utf8 text for OCR."))
    }
    ///Manages to the multi-stage download of an image. First downloading, then scanning, then deleting.
    async fn dl(&self, uri: Uri) -> anyhow::Result<String> {
        let input_string = uri.to_string();
        let extension: Vec<&str> = input_string.split(".").collect();
        let extension = extension[extension.len() -1 ];
        if extension == "mp4" || extension == "gif" || extension == "gifv" {
            return Ok("".to_owned());
        }
        let client = self.get_downloader(uri);
        let text = match client
            .and_then(|(res, url)| self.download(res, url) )
            .and_then(|path| self.scan_image(path) )
        .await {
            Ok(f) => f,
            Err(e) => {
                println!("An error occured while downloading and scanning image {}", e);
                "".to_owned()
            }
        };  
        Ok(text)
    }
    ///Downloads all images from a post, carrying out OCR on them and returning a Post.
    pub async fn download_post_images(&self, mut input: Post) -> anyhow::Result<crate::mongo_db_interface::Post> {
        let mut urls_to_download: Vec<Uri> = vec![];
        let mut text_from_images: Vec<String> = vec![];
        let filter = crate::filter::Filter::new("/home/ubuntu/PersonalProjects/0015_ImgurScraper/extension_contact_server/filter_word_list.txt")?;
        let mut output: crate::mongo_db_interface::Post;
        if filter.is_unsafe(&input.title.clone().unwrap_or("".to_owned())) || filter.is_unsafe(&input.description.clone().unwrap_or("".to_owned())) {
            output = crate::mongo_db_interface::Post {
                id: input.id,
                images: vec![],
                post_url: input.link,
                datetime: get_time().to_string(),
                unrecoverable: Some(true),
                description: Some(input.description.unwrap_or("".to_owned())),
                title: Some(input.title.unwrap_or("".to_owned())),
            }
        } else {
            for image in &input.images {
                urls_to_download.push(image.link.parse::<Uri>()?);
            }
            
            while !urls_to_download.is_empty() {
                let mut clients_vec = Vec::with_capacity(self.max_conn);
    
                for _ in 0..min(self.max_conn, urls_to_download.len()) {
                    let download = self.dl(urls_to_download.remove(0));
                    clients_vec.push(download);
                }
                for res in futures::future::join_all(clients_vec).await {
                    text_from_images.push(res.unwrap());
                }
            }
    
            //Run check
            output = crate::mongo_db_interface::Post {
                id: input.id.clone(),
                images: vec![],
                post_url: input.link,
                datetime: get_time().to_string(),
                unrecoverable: Some(false),
                description: Some(input.description.clone().unwrap_or("".to_owned())),
                title: Some(input.title.clone().unwrap_or("".to_owned())),
            };
            let mut num_unrecoverable = 0;
            let mut num_images = 0;
            assert_eq!(text_from_images.len(), input.images.len());
            for (i, image) in input.images.iter_mut().enumerate() {
                //Check each image, then push it to the output arr.
    
                let mut unrecoverable = false;
                if filter.is_unsafe(&image.description.clone().unwrap_or("".to_owned())) {
                    unrecoverable = true;
                }
                if filter.is_unsafe(&text_from_images[i]) {
                    unrecoverable = true;
                }
                let new_image = crate::mongo_db_interface::Image {
                    id: image.id.clone(),
                    description: image.description.clone().unwrap_or("".to_owned()),
                    url: image.link.clone(),
                    unrecoverable: Some(unrecoverable),
                    image_ocr_text: Some(text_from_images[i].clone())
                };
                if unrecoverable {
                    num_unrecoverable += 1;
                }
                let extension: Vec<&str> = image.link.split(".").collect();
                let extension = extension[extension.len() - 1];
                if extension != "mp4" && extension != "gif" && extension != "gifv"{
                    num_images += 1;
                }
                output.images.push(new_image);
            };
            //Check # of (non-video) images marked as unrecoverable doesn't cross threshold.
            if (num_unrecoverable as f32 / num_images as f32) as f32 >= UNRECOVERABLE_THRESHOLD {
                output.unrecoverable = Some(true);
            }
            //Remove Folder
            if fs::remove_dir_all(&self.save_path).await.is_err() {
                println!("Failed to remove folder.");
            }
        }

        //Upload to DB
        if self.db.upload_post(output.clone()).await.is_err() {
            println!("Failed to upload post to db.");
        }

        //Return Result
        Ok(output)
    }
    ///Takes a url to imgur post, contacts the imgur inc api to collect data about the post.
    pub async fn get_post(&self) -> Result<Post, anyhow::Error> {
        let client = reqwest::Client::new();
        let mut url = format!("https://api.imgur.com/3/album/{}", self.post_id);
        let mut response = client
            .get(&url)
            .header(USER_AGENT, "PostmanRuntime/7.26.8")
            .header("Authorization", "Client-ID ")
            .header("Accept", "*/*")
            .header("Connection", "keep-alive")
            .send()
            .await?;

        if response.status().as_u16() == 404 {
            url = format!("https://api.imgur.com/3/image/{}", self.post_id);
            response = client
                .get(&url)
                .header(USER_AGENT, "PostmanRuntime/7.26.8")
                .header("Authorization", "Client-ID ")
                .header("Accept", "*/*")
                .header("Connection", "keep-alive")
                .send()
                .await?;
            
            let result = response.text().await?;
            //Process the response
            let v: ResponseImage = serde_json::from_str(&*result)?;
            let v = v.data;
            let post = Post {
                id: v.id.clone(),
                datetime: v.datetime,
                title: v.title.clone(),
                is_ad: v.is_ad,
                description: v.description.clone(),
                account_url: v.account_url,
                views: v.views,
                link: format!("https://imgur.com/gallery/{}", &v.id),
                is_album: false,
                nsfw: v.nsfw,
                images_count: Some(1),
                images: vec![Image {
                    id: v.id,
                    title: v.title,
                    description: v.description,
                    link: v.link,
                }]
            };

            Ok(post)
        } else {
            if response.status().as_u16() != 200 {
                bail!("Imgur server error: {}", response.text().await?);
            }
            let result = response.text().await?;
            let v: Response = serde_json::from_str(&*result)?;
            Ok(v.data)
        }
    }
}
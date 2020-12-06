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
use anyhow::{Result, anyhow, bail};
use crate::mongo_db_interface::Database;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

const MAIN_URL: &str = "https://www.imgur.com/gallery/";
const DEFAULT_MAX_CONNECTION: usize = 15;

#[derive(Deserialize, Debug, Clone)]
pub struct Image {
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
pub struct Post {
    id: String,
    title: Option<String>,
    description: Option<String>,
    datetime: u64,
    account_url: Option<String>,
    views: u32,
    link: String,
    is_album: bool,
    favourite: Option<bool>,
    nsfw: Option<bool>,
    images_count: Option<u32>,
    is_ad: bool,
    images: Vec<Image>
}

#[derive(Deserialize, Debug)]
struct Response {
    data: Post,
}

pub struct Downloader {
    post_id: Uri,
    save_path: PathBuf,
    max_conn: usize,
    db: Database
}

fn create_filename(url: &str) -> Result<String, Box<dyn error::Error>> {
    let tmp = &Url::parse(&url)?;
    let res = &tmp.path();
    let path = format!("{}", res[1..res.len()].to_owned());
    Ok(path)
}

fn get_time() -> u128 {
    SystemTime::now().duration_since(UNIX_EPOCH).expect("Time went backwards").as_millis()
}

impl Downloader {
    pub fn new(post_id: &str, db: Database) -> Self {
        Downloader {
            post_id: post_id.parse::<Uri>().unwrap_or_else(|_| panic!("failed to parse URL: {}", post_id)),
            save_path: Path::new(post_id).to_path_buf(),
            max_conn: DEFAULT_MAX_CONNECTION,
            db: db
        }
    }

    async fn recv(&self, fut: ResponseFuture) -> Result<bytes::BytesMut, anyhow::Error> {
        let mut buf = bytes::BytesMut::new();
        let mut res = fut.await?;

        while let Some(next) = res.data().await {
            let chunk = next?;
            buf.put(chunk);
        }

        Ok(buf)
    }

    async fn get_downloader(&self, url: Uri) -> Result<(String, bytes::BytesMut), anyhow::Error> {
        let https = HttpsConnector::new();
        let client = Client::builder().build::<_, hyper::Body>(https);

        let content = self.recv(client.get(url.clone())).await?;

        Ok((url.to_string(), content))
    }

    async fn download(&self, url: String, res: bytes::BytesMut) -> anyhow::Result<PathBuf> {
        let file_name = create_filename(&url).unwrap();

        create_dir_all(&self.save_path).await.unwrap();//map_err(|e| anyhow::Error::new(e))?;

        let save_path = self.save_path.join(&file_name);

        let mut file = File::create(&save_path).await.unwrap();//await.map_err(|e| anyhow::Error::new(e))?;
        file.write_all(&res).await.unwrap();//.await.map_err(|e| anyhow::Error::new(e))?;
        Ok(save_path)
    }

    fn scan_image(&self, path: PathBuf) -> anyhow::Result<String>{
        let mut scanner = leptess::LepTess::new(Some("./tessdata"), "eng").expect("Failed to load OCR Scanner.");
        match scanner.set_image(&path) {
            Ok(_) => {},
            Err(e) => {        
                fs::remove_file(&path);
                return Ok("".to_owned())
            }
        }
        scanner.set_source_resolution(70);
        let text = scanner.get_utf8_text().unwrap_or("".to_owned());
        fs::remove_file(&path);
        Ok(text)
    }

    async fn dl(&self, uri: Uri) -> anyhow::Result<String> {
        let client = self.get_downloader(uri);
        Ok(client
            .and_then(|(res, url)| self.download(res, url) )
            .and_then(|path| async move { self.scan_image(path) })
            // .and_then(|words| async move { self.upload_to_db(words, post).await })
            .await?)
    }

    pub async fn download_post_images(&self, mut input: Post) -> anyhow::Result<crate::mongo_db_interface::Post> {
        let mut urls_to_download: Vec<Uri> = vec![];
        let mut text_from_images: Vec<String> = vec![];
        for image in &input.images {
            urls_to_download.push(image.link.parse::<Uri>()?);
        }
        let image_url_reference = urls_to_download.clone();
        
        while !urls_to_download.is_empty() {
            let mut clients_vec = Vec::with_capacity(self.max_conn);

            for _ in 0..min(self.max_conn, urls_to_download.len()) {
                let download = self.dl(urls_to_download.remove(0));
                //Todo:
                //Scan the image using leptess to check for the text.
                //Upload to database
                clients_vec.push(download);
            }
            for res in futures::future::join_all(clients_vec).await {
                //A thing to do with each future.
                text_from_images.push(res.unwrap());
            }
        }

        //Run checks
        let filter = crate::filter::Filter::new("./filter_word_list.txt")?;
        let mut output = crate::mongo_db_interface::Post {
            id: input.id,
            images: vec![],
            post_url: input.link,
            datetime: get_time().to_string(),
            unrecoverable: Some(false),
            description: Some(input.description.clone().unwrap_or("".to_owned())),
        };
        //Check post description
        if filter.is_unsafe(&input.description.unwrap_or("".to_owned())) {
            output.unrecoverable = Some(true);
        }
        
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
                replacement_img: None,
                replacement_description: None,
                image_OCR_text: Some(text_from_images[i].clone())
            };
            output.images.push(new_image);
        };
        //Upload to DB
        self.db.upload_post(output.clone()).await;

        //Remove Folder
        fs::remove_dir_all(&self.save_path);

        //Return Result
        Ok(output)
    }

    pub async fn get_post(&self) -> Result<Post, anyhow::Error> {
        let client = reqwest::Client::new();
        let url = format!("https://api.imgur.com/3/album/{}", self.post_id);
        let response = client
            .get(&url)
            .header(USER_AGENT, "PostmanRuntime/7.26.8")
            .header("Authorization", "Client-ID ")
            .header("Accept", "*/*")
            .header("Connection", "keep-alive")
            .send()
            .await?;
        if response.status().as_u16() != 200 {
            bail!("Imgur server error: {}", response.text().await?);
        }
        let result = response.text().await?;
        let v: Response = serde_json::from_str(&*result)?;
        Ok(v.data)
    }
}
///This module handles connections to and from the imgur database.

//Imports
use mongodb::{Client, options::ClientOptions, bson::{doc, Bson}, bson};
use serde::{Serialize, Deserialize};
use anyhow::{Result};

///Database struct to handle connections to the database and various collections in the mongo db.
#[derive(Clone)]
pub struct Database {
    server_ip: String,
    db: mongodb::Database,
    admin: mongodb::Collection,
    posts: mongodb::Collection,
}
///Image struct models how images are stored in the database.
#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Image {
    pub id: String,
    pub description: String,
    pub url: String,
    pub unrecoverable: Option<bool>,
    pub image_ocr_text: Option<String>,
}
///Post struct models how posts are stored in the database.
#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Post {
    pub id: String,
    pub images: Vec<Image>,
    pub post_url: String,
    pub datetime: String,
    pub unrecoverable: Option<bool>,
    pub description: Option<String>,
    pub title: Option<String>,
}

impl Database {
    ///Creates a new database instance.
    pub async fn new(server_ip: &str) -> Result<Database, anyhow::Error> {
        let client_options = ClientOptions::parse(server_ip).await?;
        let client = Client::with_options(client_options)?;
        let db = client.database("imgur_scraper");
        Ok(Database {
            server_ip: server_ip.to_owned(),
            admin: db.collection("admin"),
            posts: db.collection("posts"),
            db: db,
        })
    }
    ///Uploads a single new post instance to the mongodb database.
    pub async fn upload_post(&self, post: Post) -> Result<mongodb::results::InsertOneResult, anyhow::Error> {
        let mut images: Vec<mongodb::bson::Document> = vec![];
        for image in post.images {
            let new_image = doc!{
                "id": image.id,
                "description": image.description,
                "url": image.url,
                "unrecoverable": image.unrecoverable.unwrap_or(true),
                "image_ocr_text" : image.image_ocr_text.unwrap_or("".to_owned()),
            };
            images.push(new_image);
        }
        let new_post = doc!{
            "id": post.id,
            "images": images,
            "post_url": post.post_url,
            "datetime": post.datetime,
            "unrecoverable": post.unrecoverable.unwrap_or(true),
            "description": post.description.unwrap_or("".to_owned()),
            "title": post.title.unwrap_or("".to_owned()),
        };
        
        let result = self.posts.insert_one(new_post, None).await?;
        Ok(result)
    }
    ///Searches for a post in the database, if it can't find the post then t returns an error.
    pub async fn get_post(&self, id: &str) -> Result<Post, anyhow::Error>{
        let filter = doc!{"id": id};
        let cursor = self.posts.find_one(filter, None).await?;
        match cursor {
            Some (doc) => {
                let data: Post = bson::from_bson(Bson::Document(doc))?;
                Ok(data)
            },
            None => Err(anyhow::anyhow!("Failed to find document")),
        }
    }
}
use mongodb::{Client, options::ClientOptions, bson::{doc, Bson}, bson};
use serde::{Serialize, Deserialize};
use anyhow::{Result, anyhow};

#[derive(Clone)]
pub struct Database {
    server_ip: String,
    db: mongodb::Database,
    admin: mongodb::Collection,
    posts: mongodb::Collection,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Image {
    pub id: String,
    pub description: String,
    pub url: String,
    pub unrecoverable: Option<bool>,
    pub replacement_img: Option<String>,
    pub replacement_description: Option<String>,
    pub image_OCR_text: Option<String>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Post {
    pub id: String,
    pub images: Vec<Image>,
    pub post_url: String,
    pub datetime: String,
    pub unrecoverable: Option<bool>,
    pub description: Option<String>
}

impl Database {
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
    pub async fn upload_post(&self, post: Post) -> Result<mongodb::results::InsertOneResult, anyhow::Error> {
        let mut images: Vec<mongodb::bson::Document> = vec![];
        for image in post.images {
            let new_image = doc!{
                "id": image.id,
                "description": image.description,
                "url": image.url,
                "unrecoverable": image.unrecoverable.unwrap_or(true),
                "replacement_img": image.replacement_img.unwrap_or("".to_owned()),
                "replacement_description": image.replacement_description.unwrap_or("".to_owned())
            };
            images.push(new_image);
        }
        let new_post = doc!{
            "id": post.id,
            "images": images,
            "post_url": post.post_url,
            "datetime": post.datetime,
            "unrecoverable": post.unrecoverable.unwrap_or(true)
        };
        
        let result = self.posts.insert_one(new_post, None).await?;
        Ok(result)
    }
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
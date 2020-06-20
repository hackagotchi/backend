use bson::doc;
use mongodb::{options::ClientOptions, Client, Collection, Database};
use std::env;

/// Returns a mongo client according to the configured mongo URL
pub async fn get_mongo_client() -> Result<Client, Box<dyn std::error::Error>> {
    let client_options = ClientOptions::parse(&env::var("MONGO_URL")?).await?;
    

    Ok(Client::with_options(client_options)?)
}

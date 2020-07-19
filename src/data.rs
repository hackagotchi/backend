use mongodb::{options::ClientOptions, Client, Collection, Database};
use std::env;

/// Returns a mongo client according to the configured mongo URL
pub async fn get_mongo_client() -> Result<Client, Box<dyn std::error::Error>> {
    let client_options = ClientOptions::parse(&env::var("MONGO_URL")?).await?;

    Ok(Client::with_options(client_options)?)
}

/// Returns a mongo database from get_mongo_client()
async fn get_mongo_database(database_name: &str) -> Result<Database, Box<dyn std::error::Error>> {
    let client = get_mongo_client().await?;

    Ok(client.database(database_name))
}

/// Returns the collection of users's hacksteads.
pub async fn hacksteads() -> Result<Collection, Box<dyn std::error::Error>> {
    Ok(get_mongo_database("hackagotchi")
        .await?
        .collection("hacksteads"))
}

//TODO: Add get_next_mongo_sequence_number

pub fn to_doc<S: serde::Serialize>(s: &S) -> Result<bson::Document, hcor::RequestError> {
    match bson::to_bson(s)? {
        bson::Bson::Document(d) => Ok(d),
        not_doc => Err(hcor::RequestError::NotDocument(not_doc)),
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn simple_mongo() {
        get_mongo_client().await.expect("no db");
    }

    #[tokio::test]
    async fn hackstead_count() {
        println!(
            "hackstead count: {}",
            hacksteads()
                .await
                .expect("no hacksteads")
                .count_documents(None, None)
                .await
                .expect("no hackstead count")
        );
    }
}

use serde::{Deserialize, Serialize};
mod contact;
use contact::UserContact;
pub mod routes;

#[derive(Serialize, Deserialize, Clone)]
pub struct User {
    pub id: uuid::Uuid,
    pub contact: UserContact,
}
impl User {
    pub fn new(contact: UserContact) -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            contact,
        }
    }
    pub fn request(&self) -> UserRequest {
        UserRequest { id: self.id }
    }
}

#[derive(Deserialize, Serialize, Copy, Clone)]
pub struct UserRequest {
    pub id: uuid::Uuid,
}

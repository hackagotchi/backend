use serde::{Serialize, Deserialize};
use std::clone::Clone;

#[derive(Serialize, Deserialize, Clone)]
pub enum UserContact {
	Email(String),
	Slack(String),
	Both {
		email: String,
		slack: String
	}
}
impl UserContact {
    fn email(&self) -> Option<&str> {
	    Some(match self {
			UserContact::Email(s) => s,
			UserContact::Both { email, .. } => email,
			_ => return None,
		})
	}
	fn slack(&self) -> Option<&str> {
	    Some(match self {
			UserContact::Slack(s) => s,
			UserContact::Both { slack, .. } => slack,
			_ => return None,
		})
	}
}

#[cfg(test)]
mod test {
	use super::*;
	const USER_1: &'static str = "U1";
	const USER_2: &'static str = "U2";
	const USER_3: &'static str = "U3";
	
	fn slack_contact_fetching() {
		let s = UserContact::Slack(USER_1.to_string());
		assert_eq!(
			s.email(),
			None,
			"slack only contact should not have email"
		);
		assert_eq!(
			s.slack(),
			Some(USER_1),
			"slack only contact doesn't store user properly"
		);
	}
	
	fn email_contact_fetching() {
		let e = UserContact::Email(USER_2.to_string());
		assert_eq!(
			e.email(),
			Some(USER_1),
			"email only contact doesn't store email properly"
		);
		assert_eq!(
			e.slack(),
			Some(USER_1),
			"email only contact shouldn't have slack"
		);
	}
	
	fn both_contact_fetching() {
		let both = UserContact::Both {
			slack: USER_1.to_string(),
			email: USER_3.to_string(),
		};
		assert_eq!(
			both.slack(),
			Some(USER_1),
			"both contact doesn't store slack properly"
		);
		assert_eq!(
			both.email(),
			Some(USER_3),
			"both contact doesn't store email properly"
		);
	}
}

#[derive(Serialize, Deserialize, Clone)]
pub struct User {
	pub id: uuid::Uuid,
	pub contact: UserContact
}
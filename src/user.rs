use crate::id::GenericId;
use bcrypt::{hash, verify, DEFAULT_COST};
use hyper::Request;
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::Instant;

const TOKEN_EXPIRY: Duration = Duration::from_secs(10);

pub type UserId = GenericId;

pub struct User {
    pub username: String,
    password: String,
}

impl User {
    fn new(username: &str, password: &str) -> User {
        User {
            username: username.to_string(),
            password: hash(password, DEFAULT_COST).unwrap(),
        }
    }
}

pub struct UserDatabase {
    users: HashMap<UserId, User>,
    usernames: HashMap<String, UserId>,
    tokens: HashMap<u128, UserId>,
    token_expiry: HashMap<u128, Instant>,
}

impl UserDatabase {
    pub fn new() -> UserDatabase {
        UserDatabase {
            users: HashMap::new(),
            usernames: HashMap::new(),
            tokens: HashMap::new(),
            token_expiry: HashMap::new(),
        }
    }

    pub fn add_user(&mut self, username: &str, password: &str) -> UserId {
        let id = UserId::new();
        self.users.insert(id, User::new(username, password));
        self.usernames.insert(username.to_string(), id);
        id
    }

    pub fn get_user(&self, id: UserId) -> Option<&User> {
        self.users.get(&id)
    }

    pub fn get_user_mut(&mut self, id: UserId) -> Option<&mut User> {
        self.users.get_mut(&id)
    }

    pub fn remove_user(&mut self, id: UserId) -> Option<User> {
        self.usernames.remove(&self.users[&id].username);
        self.users.remove(&id)
    }

    pub fn add_token(&mut self, user_id: UserId) -> u128 {
        let token = rand::random();
        self.tokens.insert(token, user_id);
        self.token_expiry
            .insert(token, Instant::now() + TOKEN_EXPIRY);
        token
    }

    pub fn get_user_id_by_token(&mut self, token: u128) -> Option<UserId> {
        let expired = if let Some(expiry) = self.token_expiry.get(&token) {
            Instant::now() > *expiry
        } else {
            false
        };

        if expired {
            self.tokens.remove(&token);
            self.token_expiry.remove(&token);
            return None;
        }

        self.tokens.get(&token).copied()
    }

    pub fn remove_token(&mut self, token: u128) {
        self.tokens.remove(&token);
    }

    pub fn get_user_id_by_username(&self, username: &str) -> Option<UserId> {
        self.usernames.get(username).copied()
    }

    pub fn try_login(&self, username: &str, password: &str) -> Option<UserId> {
        if let Some(id) = self.get_user_id_by_username(username) {
            if let Some(user) = self.get_user(id) {
                if verify(password, &user.password).unwrap() {
                    return Some(id);
                }
            }
        }
        None
    }
}

pub fn parse_login_string(body: &str) -> (String, String) {
    let mut username = String::new();
    let mut password = String::new();

    for part in body.split('&') {
        let parts: Vec<&str> = part.split('=').collect();
        if parts.len() != 2 {
            continue;
        }

        match parts[0] {
            "username" => username = parts[1].to_string(),
            "password" => password = parts[1].to_string(),
            _ => {}
        }
    }

    (username, password)
}

pub fn get_login_token(request: &Request<hyper::body::Incoming>) -> Option<u128> {
    request
        .headers()
        .get("cookie")
        .and_then(|cookie| cookie.to_str().ok())
        .and_then(|cookie| {
            for part in cookie.split(';') {
                let parts: Vec<&str> = part.trim().split('=').collect();
                if parts.len() != 2 {
                    continue;
                }

                if parts[0] == "login_token" {
                    return Some(parts[1].parse().unwrap());
                }
            }

            None
        })
}

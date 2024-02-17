use bcrypt::{hash, verify, BcryptResult, DEFAULT_COST};
use std::collections::HashMap;

pub struct User {
    username: String,
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
    users: HashMap<u128, User>,
    usernames: HashMap<String, u128>,
    tokens: HashMap<u128, u128>,
}

impl UserDatabase {
    pub fn new() -> UserDatabase {
        UserDatabase {
            users: HashMap::new(),
            usernames: HashMap::new(),
            tokens: HashMap::new(),
        }
    }

    pub fn add_user(&mut self, username: &str, password: &str) -> u128 {
        let id = rand::random();
        self.users.insert(id, User::new(username, password));
        self.usernames.insert(username.to_string(), id);
        id
    }

    pub fn get_user(&self, id: u128) -> Option<&User> {
        self.users.get(&id)
    }

    pub fn get_user_mut(&mut self, id: u128) -> Option<&mut User> {
        self.users.get_mut(&id)
    }

    pub fn remove_user(&mut self, id: u128) -> Option<User> {
        self.usernames.remove(&self.users[&id].username);
        self.users.remove(&id)
    }

    pub fn add_token(&mut self, user_id: u128) -> u128 {
        let token = rand::random();
        self.tokens.insert(token, user_id);
        token
    }

    pub fn get_user_id_by_token(&self, token: u128) -> Option<u128> {
        self.tokens.get(&token).copied()
    }

    pub fn remove_token(&mut self, token: u128) -> Option<u128> {
        self.tokens.remove(&token)
    }

    pub fn get_user_id_by_username(&self, username: &str) -> Option<u128> {
        self.usernames.get(username).copied()
    }

    pub fn try_login(&self, username: &str, password: &str) -> Option<u128> {
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

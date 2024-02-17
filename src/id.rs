#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GenericId {
    id: u128,
}

impl GenericId {
    pub fn new() -> Self {
        Self { id: rand::random() }
    }

    pub fn from_int(id: u128) -> Self {
        Self { id }
    }

    pub fn to_int(&self) -> u128 {
        self.id
    }
}

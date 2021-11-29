use std::fmt::{Debug, Display, Formatter};

pub struct MyError {
    message: String,
}

impl MyError {
    pub fn new(message: String) -> MyError {
        MyError { message }
    }
}

impl Debug for MyError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Error: {}", self.message)
    }
}

impl Display for MyError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Error: {}", self.message)
    }
}

impl std::error::Error for MyError {}

#[derive(Debug)]
pub enum Error {
    InternalError(Box<dyn std::error::Error>),
    AuthError(String),
}

impl<E: 'static + std::error::Error> From<E> for Error {
    fn from(err: E) -> Self {
        Error::InternalError(Box::new(err))
    }
}

pub type Result<T> = std::result::Result<T, Error>;

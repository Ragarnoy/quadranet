pub mod data;
pub mod ping;

pub const MAX_CONTENT_SIZE: usize = 64; // Define a maximum content size

/// The Content trait defines a set of methods that a content type must implement.
/// The trait also defines a constant SIZE, which is used to determine the size of the
/// content type at compile-time
pub trait Content {
    const SIZE: usize;
    fn as_bytes(&self) -> &[u8; Self::SIZE];
    fn from_bytes(bytes: &[u8; Self::SIZE]) -> Self where Self: Sized;
}
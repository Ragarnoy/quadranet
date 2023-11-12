use crate::message::content::Content;

const DATA_SIZE: usize = 64;

pub struct DataContent {
    data: [u8; DATA_SIZE], // Fixed size for this content type
}

impl Content for DataContent {
    const SIZE: usize = DATA_SIZE;

    fn as_bytes(&self) -> &[u8; Self::SIZE] {
        &self.data
    }

    fn from_bytes(bytes: &[u8; Self::SIZE]) -> Self {
        DataContent { data: *bytes }
    }
}
use thiserror::Error;
#[derive(Error, Debug)]
pub enum ProtocolError {
    #[error(transparent)]
    Network(#[from] NetworkError),

    #[error(transparent)]
    Rule(#[from] RuleError),
}

#[derive(Error, Debug)]
pub enum NetworkError {
    // 바이트 길이가 충분하지 않을 때 발생
    #[error("Message bytes are too short. (expected length: {expected_length}, actual length: {actual_length})")]
    ShortMsg {
        expected_length: usize,
        actual_length: usize,
    },

    #[error("Message bytes are too short.")]
    TooShortMsg,

    #[error("Message bytes are not valid for the operation. (message length: {input_length}")]
    InvalidMsg { input_length: usize },
}

// 올바르지 않은 유형의 메세지가 들어왔을 때 발생
#[derive(Error, Debug)]
pub enum RuleError {
    #[error("Unrecognized type number: {0}")]
    InvalidPacketType(usize)
}

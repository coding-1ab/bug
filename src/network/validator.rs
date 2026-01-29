use crate::network::error::NetworkError;
use crate::network::util::bytes_to_u16_be;

// 수신된 패킷의 길이 체크.
// 충분하면, 사용하게 될 바이트를 제외한 잔여 패킷의 사이즈를 리턴한다.
//  길이    |   유형  |   메세지
// 2 bytes | 1 byte | N bytes ..
pub fn validate_packet_length(bytes: &[u8]) -> Result<usize, NetworkError> {
    let expected_length = bytes_to_u16_be(bytes)? as usize;
    let actual_length = bytes.len().saturating_sub(2);

    // 메세지의 길이 필드에 들어있는 값만큼의 실제 바이트 배열이 들어오지 않았을 경우
    if actual_length < expected_length {
        return Err(NetworkError::ShortMsg { expected_length, actual_length, });
    }

    Ok(actual_length - expected_length)
}
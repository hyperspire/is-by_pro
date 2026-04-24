use aes_gcm::{aead::{generic_array::GenericArray, Aead, KeyInit}, Aes256Gcm, Nonce};
use crate::AES256_KEY;
use rand::Rng;

pub fn encrypt_message(plaintext: &str) -> Result<Vec<u8>, String> {
    let key_bytes = AES256_KEY.get().expect("AES256_KEY not initialized");
    let key = GenericArray::from_slice(key_bytes);
    let cipher = Aes256Gcm::new(key);
    
    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher.encrypt(nonce, plaintext.as_bytes())
        .map_err(|_| "Encryption failed".to_string())?;
    
    let mut result = Vec::with_capacity(12 + ciphertext.len());
    result.extend_from_slice(&nonce_bytes);
    result.extend_from_slice(&ciphertext);
    Ok(result)
}
pub fn decrypt_message(encrypted_data: &[u8]) -> Result<String, String> {
    if encrypted_data.len() < 12 {
        return Err("Invalid encrypted data: too short".to_string());
    }
    let (nonce_bytes, ciphertext) = encrypted_data.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);
    let key_bytes = AES256_KEY.get().expect("AES256_KEY not initialized");
    let key = GenericArray::from_slice(key_bytes);
    let cipher = Aes256Gcm::new(key);

    let decrypted_bytes = cipher.decrypt(nonce, ciphertext).map_err(|_| "Decryption failed".to_string())?;
    String::from_utf8(decrypted_bytes).map_err(|_| "Failed to convert decrypted bytes to string".to_string())
}
pub fn encode_dm_encrypted_payload(payload: &[u8]) -> String {
  const HEX: &[u8; 16] = b"0123456789abcdef";
  let mut encoded = String::with_capacity(5 + payload.len() * 2);
  encoded.push_str("enc1:");

  for byte in payload {
    encoded.push(HEX[(byte >> 4) as usize] as char);
    encoded.push(HEX[(byte & 0x0f) as usize] as char);
  }

  encoded
}
pub fn decode_dm_hex_payload(input: &str) -> Result<Vec<u8>, String> {
  if input.len() % 2 != 0 {
    return Err("Invalid encrypted DM payload length".to_string());
  }

  fn hex_value(ch: u8) -> Option<u8> {
    match ch {
      b'0'..=b'9' => Some(ch - b'0'),
      b'a'..=b'f' => Some(ch - b'a' + 10),
      b'A'..=b'F' => Some(ch - b'A' + 10),
      _ => None,
    }
  }

  let bytes = input.as_bytes();
  let mut decoded = Vec::with_capacity(bytes.len() / 2);
  let mut index = 0usize;
  while index < bytes.len() {
    let high = hex_value(bytes[index]).ok_or_else(|| "Invalid hex in encrypted DM payload".to_string())?;
    let low = hex_value(bytes[index + 1]).ok_or_else(|| "Invalid hex in encrypted DM payload".to_string())?;
    decoded.push((high << 4) | low);
    index += 2;
  }

  Ok(decoded)
}
pub fn encode_dm_message_for_storage(plaintext: &str) -> Result<String, String> {
  let encrypted = encrypt_message(plaintext)?;
  Ok(encode_dm_encrypted_payload(&encrypted))
}
pub fn decode_dm_message_from_storage(raw_message: &[u8]) -> Result<String, String> {
  if let Ok(decrypted) = decrypt_message(raw_message) {
    return Ok(decrypted);
  }

  let as_text = std::str::from_utf8(raw_message)
    .map_err(|_| "Failed to decode stored DM message as UTF-8".to_string())?;

  if let Some(hex_payload) = as_text.strip_prefix("enc1:") {
    let decoded = decode_dm_hex_payload(hex_payload)?;
    return decrypt_message(&decoded);
  }

  // Backward compatibility for legacy plaintext DM rows.
  Ok(as_text.to_string())
}

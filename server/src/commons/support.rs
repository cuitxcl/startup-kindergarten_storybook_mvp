pub fn now() -> chrono::DateTime<chrono::Utc> {
    chrono::Utc::now()
}

#[cfg(test)]
pub fn test_uuid(seed: u128) -> uuid::Uuid {
    uuid::Uuid::from_u128(seed)
}

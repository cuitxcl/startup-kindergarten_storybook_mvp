use uuid::Uuid;

pub fn now() -> chrono::DateTime<chrono::Utc> {
    chrono::Utc::now()
}

pub fn demo_uuid(seed: u128) -> Uuid {
    Uuid::from_u128(seed)
}

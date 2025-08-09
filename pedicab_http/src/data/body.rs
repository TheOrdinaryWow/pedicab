#[derive(Debug, serde::Deserialize)]
pub struct InputBody<T> {
    pub data: T,
}

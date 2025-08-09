use std::path::PathBuf;

pub mod dal;
pub mod data;
pub mod model;

pub fn new_db(path: PathBuf, flush_every_ms: u64) -> Result<sled::Db, sled::Error> {
    sled::Config::new()
        .path(path)
        .cache_capacity(1024 * 1024 * 8)
        .flush_every_ms(Some(flush_every_ms))
        .mode(sled::Mode::HighThroughput)
        .open()
}

use sled::Db;

pub mod rule;

#[derive(Clone)]
pub struct DataAccessLayer {
    pub rule: rule::RuleDataAccessLayer,
}

impl DataAccessLayer {
    pub fn new(db: Db) -> Self {
        DataAccessLayer {
            rule: rule::RuleDataAccessLayer::new(db),
        }
    }
}

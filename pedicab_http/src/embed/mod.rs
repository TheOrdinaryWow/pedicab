use rust_embed::Embed;

#[derive(Embed)]
#[allow_missing]
#[folder = "../frontend_dist"]
pub struct EmbeddedFile;

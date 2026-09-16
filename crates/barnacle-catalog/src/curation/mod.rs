mod config;
mod rules;

pub use config::AliasEntry;
pub use config::CurationConfig;
pub use config::ExcludeEntry;
pub use config::ExcludeReason;
pub use config::KeepEntry;
pub use config::LookalikeGroup;
pub use rules::COLLABORATION_PREFIXES;
pub use rules::Curated;
pub use rules::Removal;
pub use rules::VARIANT_SUFFIXES;
pub use rules::curate;

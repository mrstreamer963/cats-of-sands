use serde::{Deserialize, Serialize};

/// ID of one instance, unique for the lifetime of its simulation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SimId(pub u32);

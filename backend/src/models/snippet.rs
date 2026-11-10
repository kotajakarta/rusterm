use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Snippet {
    pub id: i64,
    pub name: String,
    pub command: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SnippetInput {
    pub name: String,
    pub command: String,
}

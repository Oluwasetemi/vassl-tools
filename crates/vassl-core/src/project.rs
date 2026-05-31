use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: i64,
    pub name: String,
    pub client_name: String,
    pub description: Option<String>,
    pub status: ProjectStatus,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectStatus {
    Active,
    Completed,
    Archived,
}

#[derive(Debug, Clone)]
pub struct NewProject {
    pub name: String,
    pub client_name: String,
    pub description: Option<String>,
}

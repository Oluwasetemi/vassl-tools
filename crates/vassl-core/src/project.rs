use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: i64,
    pub name: String,
    pub client_name: String,
    pub client_address: Option<String>,
    pub client_attn: Option<String>,
    pub client_tel: Option<String>,
    pub description: Option<String>,
    pub status: ProjectStatus,
    pub created_at: String,
    pub date_started: Option<String>,
    pub date_completed: Option<String>,
    pub technicians: Option<String>,
    pub client_contact: Option<String>,
    pub vassl_contact: Option<String>,
    pub signedoff_date: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectStatus {
    Active,
    Completed,
    Archived,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewProject {
    pub name: String,
    pub client_name: String,
    pub client_address: Option<String>,
    pub client_attn: Option<String>,
    pub client_tel: Option<String>,
    pub description: Option<String>,
    pub date_started: Option<String>,
    pub date_completed: Option<String>,
    pub technicians: Option<String>,
    pub client_contact: Option<String>,
    pub vassl_contact: Option<String>,
    pub signedoff_date: Option<String>,
}

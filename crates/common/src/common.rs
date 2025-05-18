use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct Config {
    pub host: String,
    pub user: String,
    pub password: String,
    pub namespace: String,
    pub database: String,
    pub table: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Plant {
    pub voltage: f32,
    pub date: DateTime<Local>,
}
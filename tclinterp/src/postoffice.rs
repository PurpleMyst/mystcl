use serde::{Deserialize, Serialize};

// TODO: pass around TclObj directly.

#[derive(Debug, Serialize, Deserialize)]
pub enum TclRequest {
    Eval(String),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum TclResponse {
    Eval(Result<String, String>),
}

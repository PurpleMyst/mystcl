use serde::{Deserialize, Serialize};

// TODO: pass around TclObj directly.

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum TclRequest {
    Eval(String),
    Call(Vec<String>),
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum TclResponse {
    Eval(Result<String, String>),
    Call(Result<String, String>),
}

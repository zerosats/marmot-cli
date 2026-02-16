use serde::Serialize;

#[derive(Serialize)]
pub struct Output<T: Serialize> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

impl<T: Serialize> Output<T> {
    pub fn ok(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn err(msg: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(msg.into()),
        }
    }
}

pub fn print_json<T: Serialize>(data: T) {
    let output = Output::ok(data);
    println!("{}", serde_json::to_string_pretty(&output).unwrap());
}

pub fn print_error(err: impl std::fmt::Display) {
    let output: Output<()> = Output::err(err.to_string());
    println!("{}", serde_json::to_string_pretty(&output).unwrap());
}

pub fn print_success<T: Serialize>(data: T) {
    print_json(data);
}

use serde_json::{from_slice, json, to_string, Value};
use std::error::Error;
use std::fs;

// Define a custom HandlerResult type for returning Value
type HandlerResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

pub async fn read_info(path: Option<String>) -> HandlerResult<Value> {
    // Use provided path or default to "data.json"
    let file_path = path.unwrap_or_else(|| "data.json".to_string());

    // Read and parse existing data, or initialize empty JSON if file doesn't exist
    let info: Value = match fs::read(&file_path) {
        Ok(data) => from_slice(&data).map_err(|e| {
            println!("Failed to parse JSON from {}: {}", file_path, e);
            Box::<dyn Error + Send + Sync>::from(e)
        })?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => json!({}),
        Err(e) => {
            println!("Failed to read {}: {}", file_path, e);
            return Err(Box::<dyn Error + Send + Sync>::from(e));
        }
    };

    Ok(info)
}

pub async fn write_info(data: String, path: Option<String>) -> HandlerResult<Value> {
    // Use provided path or default to "data.json"
    let file_path = path.unwrap_or_else(|| "data.json".to_string());

    // Parse the input data as JSON
    let info: Value = serde_json::from_str(&data).map_err(|e| {
        println!("Failed to parse input data as JSON: {}", e);
        Box::<dyn Error + Send + Sync>::from(e)
    })?;

    // Write the parsed data to the file
    fs::write(
        &file_path,
        to_string(&info).map_err(|e| {
            println!("Failed to serialize JSON to {}: {}", file_path, e);
            Box::<dyn Error + Send + Sync>::from(e)
        })?,
    )
    .map_err(|e| {
        println!("Failed to write to {}: {}", file_path, e);
        Box::<dyn Error + Send + Sync>::from(e)
    })?;

    Ok(info)
}

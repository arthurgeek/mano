use std::path::PathBuf;

pub fn parse_binary_path(mut args: impl Iterator<Item = String>) -> PathBuf {
    args.nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("mano"))
}

pub fn validate_binary(path: &PathBuf) -> Result<(), String> {
    if path.exists() {
        Ok(())
    } else {
        Err(format!(
            "Error: mano binary not found at {:?}\nUsage: mano-mcp [path-to-mano-binary]",
            path
        ))
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).ok_or("expected output declaration path")?;
    std::fs::write(path, ospf_rust_bridge::typescript_declarations())?;
    Ok(())
}

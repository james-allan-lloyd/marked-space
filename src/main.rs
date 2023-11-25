use std::collections::HashMap;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Hello, world!");
    let resp =
        reqwest::blocking::get("https://httpbin.org/ip")?.json::<HashMap<String, String>>()?;
    println!("{:#?}", resp);
    Ok(())
}

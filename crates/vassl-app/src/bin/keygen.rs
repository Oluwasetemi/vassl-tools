#![allow(dead_code)]
/// Developer-only keygen tool — never shipped in the app bundle.
/// Usage: cargo run --bin keygen -- <edition> <YYYY-MM-DD|never>
///
/// Examples:
///   cargo run --bin keygen -- alpha 2027-06-10
///   cargo run --bin keygen -- pro   never

#[path = "../license.rs"]
mod license;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("Usage: cargo run --bin keygen -- <edition> <YYYY-MM-DD|never>");
        eprintln!("  edition: alpha | beta | pro");
        eprintln!("  examples:");
        eprintln!("    cargo run --bin keygen -- alpha 2027-06-10");
        eprintln!("    cargo run --bin keygen -- pro   never");
        std::process::exit(1);
    }

    let edition = match args[1].as_str() {
        "alpha" => license::Edition::Alpha,
        "beta" => license::Edition::Beta,
        "pro" => license::Edition::Pro,
        other => {
            eprintln!("Unknown edition '{other}'. Use: alpha | beta | pro");
            std::process::exit(1);
        }
    };

    let expiry = match args[2].as_str() {
        "never" => None,
        date => match chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d") {
            Ok(d) => Some(d),
            Err(_) => {
                eprintln!("Invalid date '{date}'. Use YYYY-MM-DD or 'never'.");
                std::process::exit(1);
            }
        },
    };

    println!("{}", license::generate_key(edition, expiry));
}

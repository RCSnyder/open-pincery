use crate::cli::config::{load, save};
use crate::error::AppError;

pub fn run(url: String, token: String) -> Result<(), AppError> {
    let mut cfg = load()?;
    cfg.url = Some(url);
    cfg.token = Some(token);
    save(&cfg)?;
    println!("logged in");
    Ok(())
}

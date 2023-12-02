use anyhow::{anyhow, Error, Result};

pub fn join(errors: Vec<Error>) -> Result<()> {
    let mut errors = errors.into_iter();
    let Some(a) = errors.next() else {
        return Ok(());
    };
    let Some(b) = errors.next() else {
        return Err(a);
    };
    let errors = [a, b]
        .into_iter()
        .chain(errors)
        .map(|e| e.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    Err(anyhow!("Errors: {}", errors))
}

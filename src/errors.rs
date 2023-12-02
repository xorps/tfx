use anyhow::{anyhow, Error};

pub fn join(errors: Vec<Error>) -> Result<(), Error> {
    if errors.is_empty() {
        return Ok(());
    }

    let combined_message = errors
        .into_iter()
        .map(|e| e.to_string())
        .collect::<Vec<_>>()
        .join(", ");

    let error = anyhow!("Multiple errors occurred: {}", combined_message);

    Err(error)
}

use std::error::Error;

/// Trait for abstracting output operations in CNI command handlers
/// This allows for better testability and separation of concerns
pub trait OutputWriter {
    /// Write data to the primary output (stdout equivalent)
    fn write_output(&self, data: &str) -> Result<(), Box<dyn Error>>;

    /// Write error/warning data to the error output (stderr equivalent)
    fn write_error(&self, data: &str) -> Result<(), Box<dyn Error>>;
}

/// Standard implementation that writes to stdout/stderr
#[derive(Default)]
pub struct StandardOutputWriter;

impl OutputWriter for StandardOutputWriter {
    fn write_output(&self, data: &str) -> Result<(), Box<dyn Error>> {
        println!("{}", data);
        Ok(())
    }

    fn write_error(&self, data: &str) -> Result<(), Box<dyn Error>> {
        eprintln!("{}", data);
        Ok(())
    }
}

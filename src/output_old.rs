use std::cell::RefCell;
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

/// Mock implementation for testing that captures output
#[derive(Debug, Default)]
pub struct MockOutputWriter {
    output: RefCell<Vec<String>>,
    errors: RefCell<Vec<String>>,
}

impl MockOutputWriter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&self) {
        self.output.borrow_mut().clear();
        self.errors.borrow_mut().clear();
    }

    pub fn get_output(&self) -> Vec<String> {
        self.output.borrow().clone()
    }

    pub fn get_errors(&self) -> Vec<String> {
        self.errors.borrow().clone()
    }

    pub fn get_combined_output(&self) -> String {
        self.output.borrow().join("\n")
    }

    pub fn get_combined_errors(&self) -> String {
        self.errors.borrow().join("\n")
    }
}

impl OutputWriter for MockOutputWriter {
    fn write_output(&self, data: &str) -> Result<(), Box<dyn Error>> {
        self.output.borrow_mut().push(data.to_string());
        Ok(())
    }

    fn write_error(&self, data: &str) -> Result<(), Box<dyn Error>> {
        self.errors.borrow_mut().push(data.to_string());
        Ok(())
    }
}

/// Silent implementation that discards all output (useful for testing)
pub struct SilentOutputWriter;

impl OutputWriter for SilentOutputWriter {
    fn write_output(&self, _data: &str) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    fn write_error(&self, _data: &str) -> Result<(), Box<dyn Error>> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_output_writer() {
        let writer = MockOutputWriter::new();

        writer.write_output("test output").unwrap();
        writer.write_error("test error").unwrap();

        assert_eq!(writer.get_output(), vec!["test output"]);
        assert_eq!(writer.get_errors(), vec!["test error"]);
    }

    #[test]
    fn test_silent_writer() {
        let writer = SilentOutputWriter;

        // These should not panic
        writer.write_output("test").unwrap();
        writer.write_error("error").unwrap();
    }
}

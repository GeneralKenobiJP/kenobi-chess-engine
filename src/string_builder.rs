//! A simple string builder to accumulate lines of output
//! Used if there are multiple lines of output to easily and reliably insert newlines

pub struct StringBuilder {
    buffer: String,
}

impl StringBuilder {
    pub fn new() -> Self {
        StringBuilder {
            buffer: String::new(),
        }
    }

    /// Appends a line to the builder.
    /// Inserts a newline beforehand if the builder is not empty
    pub fn append_line(&mut self, line: &str) -> &mut Self {
        if !line.is_empty() && !self.buffer.is_empty() {
            self.buffer.push('\n'); // Append newline
        }
        self.buffer.push_str(line);
        self
    }

    /// Gets the string built by the builder
    pub fn build(self) -> String {
        self.buffer
    }
}
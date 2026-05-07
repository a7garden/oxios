//! Meta-command parsing for the CLI channel.
//!
//! Recognises dot-commands that control the interactive session:
//! `.quit`, `.help`, `.reset`, `.model`, `.persona`, `.clear`.

/// A parsed meta-command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MetaCommand {
    /// Quit the session.
    Quit,
    /// Show help text.
    Help,
    /// Reset the current session / conversation.
    Reset,
    /// Switch the active model. Carries the model name, if provided.
    Model(Option<String>),
    /// Switch the active persona. Carries the persona name, if provided.
    Persona(Option<String>),
    /// Clear the terminal screen.
    Clear,
}

impl MetaCommand {
    /// Attempt to parse a line as a meta-command.
    ///
    /// Returns `Some(MetaCommand)` if the line starts with `.`,
    /// or `None` if it is a regular user message.
    pub fn parse(line: &str) -> Option<Self> {
        let trimmed = line.trim();
        if !trimmed.starts_with('.') {
            return None;
        }

        let parts: Vec<&str> = trimmed.splitn(2, whitespace_or_end).collect();
        let cmd = parts[0];
        let arg = parts.get(1).map(|s| s.trim().to_string()).filter(|s| !s.is_empty());

        match cmd {
            ".quit" | ".exit" | ".q" => Some(Self::Quit),
            ".help" | ".h" | ".?" => Some(Self::Help),
            ".reset" | ".r" => Some(Self::Reset),
            ".model" | ".m" => Some(Self::Model(arg)),
            ".persona" | ".p" => Some(Self::Persona(arg)),
            ".clear" | ".cls" => Some(Self::Clear),
            _ => None,
        }
    }

    /// Returns the help text shown by `.help`.
    pub fn help_text() -> &'static str {
        r#"Oxios CLI — Meta-commands:
  .quit, .exit, .q   Exit the session
  .help, .h, .?      Show this help
  .reset, .r          Reset the current session
  .model, .m [NAME]   Show or switch the active model
  .persona, .p [NAME] Show or switch the active persona
  .clear, .cls        Clear the terminal screen
"#
    }
}

/// Helper: find the first whitespace or end-of-string.
fn whitespace_or_end(c: char) -> bool {
    c.is_whitespace()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_quit() {
        assert_eq!(MetaCommand::parse(".quit"), Some(MetaCommand::Quit));
        assert_eq!(MetaCommand::parse(".exit"), Some(MetaCommand::Quit));
        assert_eq!(MetaCommand::parse(".q"), Some(MetaCommand::Quit));
    }

    #[test]
    fn parse_help() {
        assert_eq!(MetaCommand::parse(".help"), Some(MetaCommand::Help));
        assert_eq!(MetaCommand::parse(".h"), Some(MetaCommand::Help));
    }

    #[test]
    fn parse_model_with_arg() {
        assert_eq!(
            MetaCommand::parse(".model gpt-4o"),
            Some(MetaCommand::Model(Some("gpt-4o".into())))
        );
    }

    #[test]
    fn parse_model_no_arg() {
        assert_eq!(MetaCommand::parse(".model"), Some(MetaCommand::Model(None)));
    }

    #[test]
    fn parse_persona_with_arg() {
        assert_eq!(
            MetaCommand::parse(".persona coder"),
            Some(MetaCommand::Persona(Some("coder".into())))
        );
    }

    #[test]
    fn not_a_command() {
        assert_eq!(MetaCommand::parse("hello world"), None);
        assert_eq!(MetaCommand::parse(" .quit"), None); // leading space
    }
}

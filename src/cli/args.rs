use anyhow::{Result, anyhow};
use std::iter::Iterator;

#[derive(Debug, Clone)]
pub enum CliCommand {
    Plain { path: String },
    Inspect { path: String },
    Tui { path: String },
    Note(NoteCommand),
    Bookmark(BookmarkCommand),
    Help,
    HelpKeys,
}

#[derive(Debug, Clone)]
pub enum NoteCommand {
    Set { path: String, note: String },
    Delete { path: String },
    Gc,
}

#[derive(Debug, Clone)]
pub enum BookmarkCommand {
    Add { name: String, path: String },
    Delete { name: String },
    List,
    Jump { name: String },
}

pub fn parse_args<I>(mut args: I) -> Result<CliCommand>
where
    I: Iterator<Item = String>,
{
    let Some(first) = args.next() else {
        return Ok(CliCommand::Plain {
            path: ".".to_string(),
        });
    };

    match first.as_str() {
        "-h" | "--help" => Ok(CliCommand::Help),
        "--help-keys" => Ok(CliCommand::HelpKeys),
        "--plain" => Ok(CliCommand::Plain {
            path: args.next().unwrap_or_else(|| ".".to_string()),
        }),
        "-i" => Ok(CliCommand::Inspect {
            path: args.next().unwrap_or_else(|| ".".to_string()),
        }),
        "-l" | "--list" => Ok(CliCommand::Tui {
            path: args.next().unwrap_or_else(|| ".".to_string()),
        }),
        "-s" => {
            let note = args.next().ok_or_else(|| anyhow!("missing note"))?;
            let path = args.next().ok_or_else(|| anyhow!("missing path"))?;
            Ok(CliCommand::Note(NoteCommand::Set { path, note }))
        }
        "-d" => {
            let path = args.next().ok_or_else(|| anyhow!("missing path"))?;
            Ok(CliCommand::Note(NoteCommand::Delete { path }))
        }
        "-gc" => Ok(CliCommand::Note(NoteCommand::Gc)),
        "-b" | "--bookmark" => parse_bookmark(args),
        "bookmark" => parse_bookmark(args),
        "note" => parse_note(args),
        other if other.starts_with('-') => Err(anyhow!("unknown flag: {other}")),
        path => Ok(CliCommand::Plain {
            path: path.to_string(),
        }),
    }
}

fn parse_note<I>(mut args: I) -> Result<CliCommand>
where
    I: Iterator<Item = String>,
{
    match args.next().as_deref() {
        Some("set") => {
            let path = args.next().ok_or_else(|| anyhow!("missing path"))?;
            let note = args.next().ok_or_else(|| anyhow!("missing note"))?;
            Ok(CliCommand::Note(NoteCommand::Set { path, note }))
        }
        Some("del") | Some("delete") | Some("rm") => {
            let path = args.next().ok_or_else(|| anyhow!("missing path"))?;
            Ok(CliCommand::Note(NoteCommand::Delete { path }))
        }
        Some("gc") => Ok(CliCommand::Note(NoteCommand::Gc)),
        _ => Err(anyhow!("unknown note command")),
    }
}

fn parse_bookmark<I>(mut args: I) -> Result<CliCommand>
where
    I: Iterator<Item = String>,
{
    match args.next().as_deref() {
        Some("add") => {
            let name = args
                .next()
                .ok_or_else(|| anyhow!("missing bookmark name"))?;
            let path = args.next().unwrap_or_else(|| ".".to_string());
            Ok(CliCommand::Bookmark(BookmarkCommand::Add { name, path }))
        }
        Some("list") | Some("ls") | None => Ok(CliCommand::Bookmark(BookmarkCommand::List)),
        Some("del") | Some("delete") | Some("rm") => {
            let name = args
                .next()
                .ok_or_else(|| anyhow!("missing bookmark name"))?;
            Ok(CliCommand::Bookmark(BookmarkCommand::Delete { name }))
        }
        Some(name) => Ok(CliCommand::Bookmark(BookmarkCommand::Jump {
            name: name.to_string(),
        })),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_default_to_plain() {
        let command = parse_args(std::iter::empty::<String>()).expect("parse default");
        match command {
            CliCommand::Plain { path } => assert_eq!(path, "."),
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parse_note_set_command() {
        let args = vec![
            "note".to_string(),
            "set".to_string(),
            "README.md".to_string(),
            "demo".to_string(),
        ];
        let command = parse_args(args.into_iter()).expect("parse note set");
        match command {
            CliCommand::Note(NoteCommand::Set { path, note }) => {
                assert_eq!(path, "README.md");
                assert_eq!(note, "demo");
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parse_bookmark_add_command() {
        let args = vec![
            "bookmark".to_string(),
            "add".to_string(),
            "docs".to_string(),
            "/tmp".to_string(),
        ];
        let command = parse_args(args.into_iter()).expect("parse bookmark add");
        match command {
            CliCommand::Bookmark(BookmarkCommand::Add { name, path }) => {
                assert_eq!(name, "docs");
                assert_eq!(path, "/tmp");
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }
}

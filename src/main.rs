mod cli;
mod paclog;

use anyhow::{anyhow, Result as AnyResult};
use chrono::NaiveDate;
use clap::StructOpt;
use cli::Cli;
use paclog::get_changes;
use regex::Regex;

fn parse_date(date_str: &str) -> AnyResult<NaiveDate> {
    NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
        .map_err(|_| anyhow!("Invalid date format: {}", date_str))
}

fn main() -> AnyResult<()> {
    let args = Cli::parse();

    let regexes = args
        .packages
        .iter()
        // Allow glob/regex with star
        .map(|s| Regex::new(&format!("^{}$", regex::escape(s).replace(r"\*", ".*"))))
        .collect::<Result<Vec<Regex>, regex::Error>>()?;

    let before = if let Some(before_str) = args.before {
        Some(parse_date(&before_str)?)
    } else {
        None
    };

    let after = if let Some(after_str) = args.after {
        Some(parse_date(&after_str)?)
    } else {
        None
    };

    let changes = get_changes(regexes)?;
    for change in changes {
        if let Some(before) = before {
            if change.date() > before {
                continue;
            }
        }
        if let Some(after) = after {
            if change.date() < after {
                continue;
            }
        }

        change.print()?;
    }

    Ok(())
}

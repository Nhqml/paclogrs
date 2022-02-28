mod cli;
mod paclog;

use clap::StructOpt;

use anyhow::Result as AnyResult;

use cli::Cli;
use paclog::get_changes;
use regex::Regex;

fn main() -> AnyResult<()> {
    let args = Cli::parse();

    let regexes = args
        .packages
        .iter()
        // Allow glob/regex with star
        .map(|s| Regex::new(&format!("^{}$", regex::escape(s).replace(r"\*", ".*"))))
        .collect::<Result<Vec<Regex>, regex::Error>>()?;

    for change in get_changes(regexes)? {
        change.print()?;
    }

    Ok(())
}

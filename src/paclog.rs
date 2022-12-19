use std::{
    fs::File,
    io::{BufRead, BufReader, Write},
};

use anyhow::anyhow;
use anyhow::Result as AnyResult;
use lazy_static::lazy_static;
use regex::Regex;
use termcolor::{BufferedStandardStream, Color, ColorChoice, ColorSpec, WriteColor};

#[derive(Debug)]
pub enum PacmanAction {
    Installed,
    Upgraded,
    Downgraded,
    Removed,
}

impl TryFrom<&str> for PacmanAction {
    type Error = anyhow::Error;

    fn try_from(action: &str) -> Result<Self, Self::Error> {
        match action {
            "installed" => Ok(Self::Installed),
            "upgraded" => Ok(Self::Upgraded),
            "downgraded" => Ok(Self::Downgraded),
            "removed" => Ok(Self::Removed),
            _ => Err(anyhow!("`{action}` is not a valid action!")),
        }
    }
}

lazy_static! {
    static ref PACKAGE_CHANGE_REGEX: Regex = Regex::new(
        r"\[(?P<date>.*)\] \[ALPM\] (?P<action>[[[:alpha:]]]+) (?P<package>[a-z0-9@_+][a-z0-9@._+-]*) \((?P<version>.*)\)"
    )
    .unwrap();

    static ref PACKAGE_VERSION_REGEX: Regex = Regex::new(
        r"(^[a-z0-9.:+-]+)(?: -> ([a-z0-9.:+-]+$))?"
    ).unwrap();
}

#[derive(Debug)]
pub struct PackageChange {
    name: String,
    datetime: String,
    action: PacmanAction,
    previous_version: Option<String>,
    current_version: Option<String>,
}

impl PackageChange {
    fn name_matches(name: &str, regexes: &[Regex]) -> bool {
        if regexes.is_empty() {
            return true;
        }

        for regex in regexes {
            if regex.is_match(name) {
                return true;
            }
        }
        false
    }

    pub fn from_line(line: String, regexes: &[Regex]) -> AnyResult<Self> {
        if let Some(cap) = PACKAGE_CHANGE_REGEX.captures(&line) {
            let name = String::from(
                cap.name("package")
                    .ok_or(anyhow!("No package name found"))?
                    .as_str(),
            );
            if !Self::name_matches(&name, regexes) {
                return Err(anyhow!(
                    "Package `{name}` does not match one of the provided Regex"
                ));
            }

            let action = PacmanAction::try_from(
                cap.name("action")
                    .ok_or(anyhow!("No PacmanAction found"))?
                    .as_str(),
            )?;
            let datetime = String::from(cap.name("date").ok_or(anyhow!("No date found"))?.as_str());

            if let Some(version_change) = PACKAGE_VERSION_REGEX.captures(
                cap.name("version")
                    .ok_or(anyhow!("No version change found"))?
                    .as_str(),
            ) {
                let (mut previous_version, mut current_version) = (None, None);

                let (lv, rv) = (
                    version_change.get(1).map(|m| m.as_str().to_string()),
                    version_change.get(2).map(|m| m.as_str().to_string()),
                );
                match action {
                    PacmanAction::Installed => {
                        current_version = Some(lv.ok_or(anyhow!("No current package version"))?);
                    }
                    PacmanAction::Upgraded | PacmanAction::Downgraded => {
                        previous_version = Some(lv.ok_or(anyhow!("No previous package version"))?);
                        current_version = Some(rv.ok_or(anyhow!("No current package version"))?);
                    }
                    PacmanAction::Removed => {
                        previous_version = Some(lv.ok_or(anyhow!("No previous package version"))?);
                    }
                }

                return Ok(PackageChange {
                    name,
                    datetime,
                    action,
                    previous_version,
                    current_version,
                });
            }
        }

        Err(anyhow!(
            "Unable to create a new PackageChange from `{}`",
            line
        ))
    }
}

impl PackageChange {
    pub fn print(&self) -> AnyResult<()> {
        let color_choice = if atty::is(atty::Stream::Stdout) {
            ColorChoice::Auto
        } else {
            ColorChoice::Never
        };

        let mut stdout = BufferedStandardStream::stdout(color_choice);

        stdout.set_color(ColorSpec::new().set_fg(Some(Color::White)).set_dimmed(true))?;
        stdout.write_all(format!("[{}]", self.datetime).as_bytes())?;

        match self.action {
            PacmanAction::Installed => {
                stdout.set_color(ColorSpec::new().set_fg(Some(Color::Green)))?;
                stdout.write_all(b" installed ")?;
            }
            PacmanAction::Upgraded => {
                stdout.set_color(ColorSpec::new().set_fg(Some(Color::Cyan)))?;
                stdout.write_all(b" upgraded ")?;
            }
            PacmanAction::Downgraded => {
                stdout.set_color(ColorSpec::new().set_fg(Some(Color::Magenta)))?;
                stdout.write_all(b" downgraded ")?;
            }
            PacmanAction::Removed => {
                stdout.set_color(ColorSpec::new().set_fg(Some(Color::Red)))?;
                stdout.write_all(b" removed ")?;
            }
        }

        stdout.set_color(
            ColorSpec::new()
                .set_fg(Some(Color::Yellow))
                .set_bold(true)
                .set_intense(true),
        )?;
        stdout.write_all(self.name.as_bytes())?;

        stdout.reset()?;
        stdout.write_all(b" (")?;

        match self.action {
            PacmanAction::Installed => {
                stdout.set_color(ColorSpec::new().set_fg(Some(Color::Cyan)))?;
                stdout.write_all(self.current_version.as_ref().unwrap().as_bytes())?;
            }
            PacmanAction::Upgraded | PacmanAction::Downgraded => {
                stdout.set_color(ColorSpec::new().set_fg(Some(Color::Magenta)))?;
                stdout.write_all(self.previous_version.as_ref().unwrap().as_bytes())?;

                stdout.reset()?;
                stdout.write_all(b" -> ")?;

                stdout.set_color(ColorSpec::new().set_fg(Some(Color::Cyan)))?;
                stdout.write_all(self.current_version.as_ref().unwrap().as_bytes())?;
            }
            PacmanAction::Removed => {
                stdout.set_color(ColorSpec::new().set_fg(Some(Color::Magenta)))?;
                stdout.write_all(self.previous_version.as_ref().unwrap().as_bytes())?;
            }
        }

        stdout.reset()?;
        stdout.write_all(b")\n")?;

        Ok(())
    }
}

const PACMAN_LOG_FILE: &str = "/var/log/pacman.log";

pub fn get_changes(regexes: Vec<Regex>) -> AnyResult<Vec<PackageChange>> {
    let file_bufreader = BufReader::new(File::open(PACMAN_LOG_FILE)?);

    let mut changes = Vec::new();
    for line in file_bufreader.lines().filter_map(|r| r.ok()) {
        if let Ok(change) = PackageChange::from_line(line, &regexes) {
            changes.push(change);
        }
    }

    Ok(changes)
}

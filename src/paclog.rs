use std::{
    fs::File,
    io::{BufRead, BufReader, Write},
};

use anyhow::anyhow;
use anyhow::Result as AnyResult;
use chrono::{DateTime, Local, NaiveDate, NaiveDateTime};
use lazy_static::lazy_static;
use regex::Regex;
use termcolor::{BufferedStandardStream, Color, ColorChoice, ColorSpec, WriteColor};

#[derive(Debug, PartialEq, PartialOrd)]
pub(crate) enum PacmanDateTime {
    WithTimezone(DateTime<Local>),
    WithoutTimezone(NaiveDateTime),
}

impl std::fmt::Display for PacmanDateTime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::WithTimezone(dt) => dt.format("%Y-%m-%d %H:%M"),
                Self::WithoutTimezone(dt) => dt.format("%Y-%m-%d %H:%M"),
            }
        )
    }
}

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
        r"\[(?P<datetime>.*)\] \[ALPM\] (?P<action>[[[:alpha:]]]+) (?P<package>[a-z0-9@_+][a-z0-9@._+-]*) \((?P<version>.*)\)"
    )
    .expect("Valid regex");

    static ref PACKAGE_VERSION_REGEX: Regex = Regex::new(
        r"(^[a-z0-9.:+-]+)(?: -> ([a-z0-9.:+-]+$))?"
    )
    .expect("Valid regex");
}

#[derive(Debug)]
pub struct PackageChange {
    name: String,
    datetime: PacmanDateTime,
    action: PacmanAction,
    previous_version: Option<String>,
    current_version: Option<String>,
}

impl PackageChange {
    fn matches_any_regex(name: &str, regexes: &[Regex]) -> bool {
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
            if !(regexes.is_empty() || Self::matches_any_regex(&name, regexes)) {
                return Err(anyhow!(
                    "Package `{name}` does not match one of the provided regexes"
                ));
            }

            let action = PacmanAction::try_from(
                cap.name("action")
                    .ok_or(anyhow!("No PacmanAction found"))?
                    .as_str(),
            )?;
            let datetime = cap
                .name("datetime")
                .ok_or(anyhow!("No datetime found"))?
                .as_str();

            let datetime = if let Ok(dt) = DateTime::parse_from_str(datetime, "%Y-%m-%dT%H:%M:%S%z")
            {
                PacmanDateTime::WithTimezone(dt.with_timezone(&Local))
            } else if let Ok(dt) = NaiveDateTime::parse_from_str(datetime, "%Y-%m-%d %H:%M") {
                PacmanDateTime::WithoutTimezone(dt)
            } else {
                println!("Unable to parse datetime from `{}`", datetime);
                return Err(anyhow!("Unable to parse datetime from `{}`", datetime));
            };

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

    pub fn date(&self) -> NaiveDate {
        match &self.datetime {
            PacmanDateTime::WithTimezone(dt) => dt.date_naive(),
            PacmanDateTime::WithoutTimezone(dt) => dt.date(),
        }
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
                stdout.write_all(
                    self.current_version
                        .as_ref()
                        .expect("Buffer written without error")
                        .as_bytes(),
                )?;
            }
            PacmanAction::Upgraded | PacmanAction::Downgraded => {
                stdout.set_color(ColorSpec::new().set_fg(Some(Color::Magenta)))?;
                stdout.write_all(
                    self.previous_version
                        .as_ref()
                        .expect("Buffer written without error")
                        .as_bytes(),
                )?;

                stdout.reset()?;
                stdout.write_all(b" -> ")?;

                stdout.set_color(ColorSpec::new().set_fg(Some(Color::Cyan)))?;
                stdout.write_all(
                    self.current_version
                        .as_ref()
                        .expect("Buffer written without error")
                        .as_bytes(),
                )?;
            }
            PacmanAction::Removed => {
                stdout.set_color(ColorSpec::new().set_fg(Some(Color::Magenta)))?;
                stdout.write_all(
                    self.previous_version
                        .as_ref()
                        .expect("Buffer written without error")
                        .as_bytes(),
                )?;
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
    for line in file_bufreader.lines().map_while(Result::ok) {
        if let Ok(change) = PackageChange::from_line(line, &regexes) {
            changes.push(change);
        }
    }

    Ok(changes)
}

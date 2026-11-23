use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use clap::CommandFactory;
use clap_complete::{Shell, generate};
use serde::Serialize;

use crate::cli::{Cli, CompletionShell, CompletionsCommand};
use crate::config::AppConfig;
use crate::output;

const BEGIN_MARKER: &str = "# >>> whatshell completions >>>";
const END_MARKER: &str = "# <<< whatshell completions <<<";

#[derive(Debug, Serialize)]
pub struct CompletionsInstallSummary {
    pub shell: String,
    pub installed: bool,
    pub dry_run: bool,
    pub path: String,
    pub block: String,
}

pub fn run(config: &AppConfig, args: CompletionsCommand) -> Result<()> {
    let shell = clap_shell(args.shell);
    if args.install {
        let path = install_path(args.shell)?;
        let block = managed_block(args.shell);
        if !args.dry_run {
            write_managed_block(&path, &block)?;
        }
        let summary = CompletionsInstallSummary {
            shell: shell_name(args.shell).into(),
            installed: !args.dry_run,
            dry_run: args.dry_run,
            path: path.display().to_string(),
            block: block.clone(),
        };
        if config.json {
            output::json_response(summary)
        } else if args.dry_run {
            println!(
                "would install completions for {} at {}",
                shell_name(args.shell),
                path.display()
            );
            println!("{block}");
            Ok(())
        } else {
            println!(
                "installed {} completions in {}",
                shell_name(args.shell),
                path.display()
            );
            match args.shell {
                CompletionShell::Bash => {
                    println!("reload with: source {}", path.display());
                }
                CompletionShell::Pwsh => {
                    println!("reload PowerShell or run: . {}", path.display());
                }
            }
            Ok(())
        }
    } else {
        let mut cmd = Cli::command();
        generate(shell, &mut cmd, "whatshell", &mut std::io::stdout());
        Ok(())
    }
}

fn clap_shell(shell: CompletionShell) -> Shell {
    match shell {
        CompletionShell::Bash => Shell::Bash,
        CompletionShell::Pwsh => Shell::PowerShell,
    }
}

fn shell_name(shell: CompletionShell) -> &'static str {
    match shell {
        CompletionShell::Bash => "bash",
        CompletionShell::Pwsh => "pwsh",
    }
}

fn managed_block(shell: CompletionShell) -> String {
    match shell {
        CompletionShell::Bash => {
            format!("{BEGIN_MARKER}\neval \"$(whatshell completions bash)\"\n{END_MARKER}\n")
        }
        CompletionShell::Pwsh => format!(
            "{BEGIN_MARKER}\nwhatshell completions pwsh | Out-String | Invoke-Expression\n{END_MARKER}\n"
        ),
    }
}

pub fn install_path(shell: CompletionShell) -> Result<PathBuf> {
    match shell {
        CompletionShell::Bash => {
            let home = env::var_os("HOME")
                .or_else(|| env::var_os("USERPROFILE"))
                .ok_or_else(|| anyhow!("HOME/USERPROFILE is not set"))?;
            Ok(PathBuf::from(home).join(".bashrc"))
        }
        CompletionShell::Pwsh => {
            if let Ok(profile) = env::var("PROFILE")
                && !profile.trim().is_empty()
            {
                return Ok(PathBuf::from(profile));
            }
            let docs = directories::UserDirs::new()
                .and_then(|dirs| dirs.document_dir().map(Path::to_path_buf))
                .ok_or_else(|| {
                    anyhow!("could not resolve Documents directory for PowerShell profile")
                })?;
            Ok(docs
                .join("PowerShell")
                .join("Microsoft.PowerShell_profile.ps1"))
        }
    }
}

pub fn write_managed_block(path: &Path, block: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create profile directory {}", parent.display()))?;
    }

    let existing = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(err) => {
            return Err(err).with_context(|| format!("read profile {}", path.display()));
        }
    };

    let updated = upsert_managed_block(&existing, block);
    let mut file =
        fs::File::create(path).with_context(|| format!("write profile {}", path.display()))?;
    file.write_all(updated.as_bytes())
        .with_context(|| format!("write profile {}", path.display()))?;
    Ok(())
}

pub fn upsert_managed_block(existing: &str, block: &str) -> String {
    let block = block.trim_end_matches('\n');
    if let Some(start) = existing.find(BEGIN_MARKER)
        && let Some(end_rel) = existing[start..].find(END_MARKER)
    {
        let end = start + end_rel + END_MARKER.len();
        let mut end = end;
        if existing[end..].starts_with('\r') {
            end += 1;
        }
        if existing[end..].starts_with('\n') {
            end += 1;
        }
        let mut out = String::with_capacity(existing.len() + block.len());
        out.push_str(&existing[..start]);
        out.push_str(block);
        out.push('\n');
        out.push_str(&existing[end..]);
        return out;
    }

    let mut out = existing.to_string();
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    if !out.is_empty() {
        out.push('\n');
    }
    out.push_str(block);
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upsert_appends_when_missing() {
        let updated = upsert_managed_block(
            "export PATH\n",
            managed_block(CompletionShell::Bash).trim_end(),
        );
        assert!(updated.contains(BEGIN_MARKER));
        assert!(updated.contains("eval \"$(whatshell completions bash)\""));
        assert!(updated.starts_with("export PATH\n"));
    }

    #[test]
    fn upsert_replaces_existing_block() {
        let existing = format!("before\n{BEGIN_MARKER}\nold\n{END_MARKER}\nafter\n");
        let updated =
            upsert_managed_block(&existing, managed_block(CompletionShell::Pwsh).trim_end());
        assert!(updated.contains("whatshell completions pwsh"));
        assert!(!updated.contains("old\n"));
        assert!(updated.starts_with("before\n"));
        assert!(updated.contains("after\n"));
        assert_eq!(updated.matches(BEGIN_MARKER).count(), 1);
    }
}

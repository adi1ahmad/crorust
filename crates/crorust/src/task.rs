use anyhow::{Context, Result};
use console::{style, Term};
use std::process::Command;
use std::time::Instant;

#[derive(Debug)]
enum ActivePhase {
    None,
    Running { name: String, started: Instant },
}

pub struct ProgressIndicator {
    total_steps: u32,
    completed_steps: u32,
    active_phase: ActivePhase,
    terminal: Term,
    verbose: bool,
}

impl ProgressIndicator {
    pub fn new(total_steps: u32, verbose: bool) -> Self {
        Self {
            total_steps,
            completed_steps: 0,
            active_phase: ActivePhase::None,
            terminal: Term::stdout(),
            verbose,
        }
    }

    fn step_prefix(&self) -> String {
        style(format!(
            "[{}/{}]",
            self.completed_steps.saturating_add(1),
            self.total_steps
        ))
        .bold()
        .to_string()
    }

    pub fn begin_phase(&mut self, descr: impl Into<String>) {
        if matches!(self.active_phase, ActivePhase::Running { .. }) {
            self.complete_phase_internal(true, true);
        }
        let name = descr.into();
        println!("{} {}", self.step_prefix(), name);
        self.active_phase = ActivePhase::Running {
            name,
            started: Instant::now(),
        };
    }

    fn complete_phase_internal(&mut self, skipped: bool, clear_last: bool) {
        if let ActivePhase::Running { ref name, started } = self.active_phase {
            if clear_last {
                let _ = self.terminal.clear_last_lines(1);
            }
            let status = if skipped {
                style("[SKIPPED]").dim().to_string()
            } else {
                let duration = started.elapsed();
                style(format!("[{}ms]", duration.as_millis()))
                    .dim()
                    .to_string()
            };
            println!("{} {} {}", self.step_prefix(), name, status);
            self.completed_steps += 1;
            self.active_phase = ActivePhase::None;
        }
    }

    pub fn complete_phase(&mut self) {
        self.complete_phase_internal(false, !self.verbose);
    }

    pub fn complete_verbose_phase(&mut self) {
        self.complete_phase_internal(false, false);
    }
}

pub struct CliTaskReporter {
    pub verbose: bool,
}

impl CliTaskReporter {
    pub fn new(verbose: bool) -> Self {
        Self { verbose }
    }
}

pub fn run(command: &mut Command) -> Result<()> {
    fn format_subprocess_error(command: &Command, status: Option<i32>) -> String {
        let status = if let Some(code) = status {
            format!(" exited with {code}")
        } else {
            Default::default()
        };
        format!("{} `{:?}`{}", style("[ERROR]").red(), command, status)
    }
    let status = command
        .status()
        .with_context(|| format_subprocess_error(command, None))?;
    if !status.success() {
        anyhow::bail!("{}", format_subprocess_error(command, status.code()));
    }
    Ok(())
}

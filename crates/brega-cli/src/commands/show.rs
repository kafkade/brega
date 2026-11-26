//! `brega show` — display task details.

use brega_core::domain::task::Task;

/// Format a task for detailed display (human-readable).
#[must_use]
pub fn format_task_detail(task: &Task) -> String {
    crate::display::format_task_detail(task, crate::display::OutputFormat::Table)
}

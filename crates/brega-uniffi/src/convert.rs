//! Conversions between `brega-core` / `brega-storage` domain types and
//! the `UniFFI`-friendly types in [`crate::types`].
//!
//! All UUID fields are mapped to/from hyphenated string form. Timestamps
//! use RFC 3339. JSON blobs are passed through as opaque strings.

use std::collections::BTreeMap;

use brega_core::domain::area::{Area, AreaPatch};
use brega_core::domain::focus::{FocusConfig, FocusSession, FocusState};
use brega_core::domain::habit::{Habit, HabitDirection, HabitEntry};
use brega_core::domain::project::{Project, ProjectStatus};
use brega_core::domain::tag::Tag;
use brega_core::domain::task::{NewTask, Priority, Task, TaskPatch, TaskStatus};
use brega_core::domain::time_block::{BlockSource, TimeBlock};
use brega_core::domain::uda::UdaValues;
use time::format_description::well_known::Rfc3339;
use uuid::Uuid;

use crate::error::BregaError;
use crate::types::{
    BregaArea, BregaAreaPatch, BregaBlockSource, BregaFocusConfig, BregaFocusSession,
    BregaFocusState, BregaHabit, BregaHabitDirection, BregaHabitEntry, BregaNewTask, BregaPriority,
    BregaProject, BregaProjectStatus, BregaTag, BregaTask, BregaTaskPatch, BregaTaskStatus,
    BregaTimeBlock,
};

// ── Helpers ──────────────────────────────────────────────────────────

fn format_ts(ts: time::OffsetDateTime) -> String {
    ts.format(&Rfc3339).unwrap_or_default()
}

fn format_optional_ts(ts: Option<time::OffsetDateTime>) -> Option<String> {
    ts.map(format_ts)
}

fn uuid_str(id: Uuid) -> String {
    id.to_string()
}

fn optional_uuid_str(id: Option<Uuid>) -> Option<String> {
    id.map(uuid_str)
}

pub fn parse_uuid(s: &str) -> Result<Uuid, BregaError> {
    Uuid::parse_str(s).map_err(|_| BregaError::InvalidInput {
        message: format!("invalid UUID: {s}"),
    })
}

pub fn parse_optional_uuid(s: Option<&str>) -> Result<Option<Uuid>, BregaError> {
    s.map(parse_uuid).transpose()
}

// ── Enum conversions ─────────────────────────────────────────────────

impl From<TaskStatus> for BregaTaskStatus {
    fn from(s: TaskStatus) -> Self {
        match s {
            TaskStatus::Inbox => Self::Inbox,
            TaskStatus::Pending => Self::Pending,
            TaskStatus::Started => Self::Started,
            TaskStatus::Done => Self::Done,
            TaskStatus::Cancelled => Self::Cancelled,
            TaskStatus::Someday => Self::Someday,
        }
    }
}

impl From<BregaTaskStatus> for TaskStatus {
    fn from(s: BregaTaskStatus) -> Self {
        match s {
            BregaTaskStatus::Inbox => Self::Inbox,
            BregaTaskStatus::Pending => Self::Pending,
            BregaTaskStatus::Started => Self::Started,
            BregaTaskStatus::Done => Self::Done,
            BregaTaskStatus::Cancelled => Self::Cancelled,
            BregaTaskStatus::Someday => Self::Someday,
        }
    }
}

impl From<Priority> for BregaPriority {
    fn from(p: Priority) -> Self {
        match p {
            Priority::Low => Self::Low,
            Priority::Medium => Self::Medium,
            Priority::High => Self::High,
        }
    }
}

impl From<BregaPriority> for Priority {
    fn from(p: BregaPriority) -> Self {
        match p {
            BregaPriority::Low => Self::Low,
            BregaPriority::Medium => Self::Medium,
            BregaPriority::High => Self::High,
        }
    }
}

impl From<ProjectStatus> for BregaProjectStatus {
    fn from(s: ProjectStatus) -> Self {
        match s {
            ProjectStatus::Active => Self::Active,
            ProjectStatus::Paused => Self::Paused,
            ProjectStatus::Someday => Self::Someday,
            ProjectStatus::Done => Self::Done,
            ProjectStatus::Cancelled => Self::Cancelled,
        }
    }
}

impl From<FocusState> for BregaFocusState {
    fn from(s: FocusState) -> Self {
        match s {
            FocusState::Working => Self::Working,
            FocusState::ShortBreak => Self::ShortBreak,
            FocusState::LongBreak => Self::LongBreak,
            FocusState::Paused => Self::Paused,
            FocusState::Aborted => Self::Aborted,
            FocusState::Completed => Self::Completed,
        }
    }
}

impl From<BlockSource> for BregaBlockSource {
    fn from(s: BlockSource) -> Self {
        match s {
            BlockSource::Manual => Self::Manual,
            BlockSource::Timer => Self::Timer,
            BlockSource::Pomodoro => Self::Pomodoro,
            BlockSource::Imported => Self::Imported,
        }
    }
}

impl From<HabitDirection> for BregaHabitDirection {
    fn from(d: HabitDirection) -> Self {
        match d {
            HabitDirection::Build => Self::Build,
            HabitDirection::Break => Self::Break,
        }
    }
}

impl From<BregaHabitDirection> for HabitDirection {
    fn from(d: BregaHabitDirection) -> Self {
        match d {
            BregaHabitDirection::Build => Self::Build,
            BregaHabitDirection::Break => Self::Break,
        }
    }
}

// ── Record conversions: core → FFI ───────────────────────────────────

impl From<Task> for BregaTask {
    fn from(t: Task) -> Self {
        Self {
            id: uuid_str(t.id),
            sid: t.sid,
            title: t.title,
            notes: t.notes,
            status: t.status.into(),
            area_id: optional_uuid_str(t.area_id),
            project_id: optional_uuid_str(t.project_id),
            heading_id: optional_uuid_str(t.heading_id),
            parent_id: optional_uuid_str(t.parent_id),
            start_date: t.start_date,
            deadline: t.deadline,
            scheduled_for: t.scheduled_for,
            recurrence: t.recurrence,
            priority: t.priority.map(Into::into),
            estimate_minutes: t.estimate_minutes,
            evening: t.evening,
            udas: t.udas.to_json(),
            tags: t.tags,
            depends_on: t.depends_on.into_iter().map(uuid_str).collect(),
            urgency: t.urgency,
            created_at: format_ts(t.created_at),
            modified_at: format_ts(t.modified_at),
            done_at: format_optional_ts(t.done_at),
            cancelled_at: format_optional_ts(t.cancelled_at),
            deleted_at: format_optional_ts(t.deleted_at),
        }
    }
}

impl From<Project> for BregaProject {
    fn from(p: Project) -> Self {
        Self {
            id: uuid_str(p.id),
            sid: p.sid,
            name: p.name,
            notes: p.notes,
            status: p.status.into(),
            area_id: optional_uuid_str(p.area_id),
            deadline: p.deadline,
            sort_order: p.sort_order,
            created_at: format_ts(p.created_at),
            modified_at: format_ts(p.modified_at),
            archived_at: format_optional_ts(p.archived_at),
        }
    }
}

impl From<Area> for BregaArea {
    fn from(a: Area) -> Self {
        Self {
            id: uuid_str(a.id),
            name: a.name,
            color: a.color,
            sort_order: a.sort_order,
            weekly_budget_minutes: a.weekly_budget_minutes,
            created_at: format_ts(a.created_at),
            modified_at: format_ts(a.modified_at),
            archived_at: format_optional_ts(a.archived_at),
        }
    }
}

impl From<Tag> for BregaTag {
    fn from(t: Tag) -> Self {
        Self {
            id: uuid_str(t.id),
            name: t.name,
            color: t.color,
        }
    }
}

impl From<TimeBlock> for BregaTimeBlock {
    fn from(b: TimeBlock) -> Self {
        Self {
            id: uuid_str(b.id),
            sid: b.sid,
            title: b.title,
            start_ts: format_ts(b.start_ts),
            end_ts: format_optional_ts(b.end_ts),
            project_id: optional_uuid_str(b.project_id),
            task_id: optional_uuid_str(b.task_id),
            notes: b.notes,
            source: b.source.into(),
            billable: b.billable,
            created_at: format_ts(b.created_at),
            modified_at: format_ts(b.modified_at),
        }
    }
}

impl From<FocusConfig> for BregaFocusConfig {
    fn from(c: FocusConfig) -> Self {
        Self {
            work_minutes: c.work_minutes,
            short_break_minutes: c.short_break_minutes,
            long_break_minutes: c.long_break_minutes,
            cycles_before_long_break: c.cycles_before_long_break,
        }
    }
}

impl From<BregaFocusConfig> for FocusConfig {
    fn from(c: BregaFocusConfig) -> Self {
        Self {
            work_minutes: c.work_minutes,
            short_break_minutes: c.short_break_minutes,
            long_break_minutes: c.long_break_minutes,
            cycles_before_long_break: c.cycles_before_long_break,
        }
    }
}

impl From<FocusSession> for BregaFocusSession {
    fn from(s: FocusSession) -> Self {
        Self {
            id: uuid_str(s.id),
            sid: s.sid,
            started_at: format_ts(s.started_at),
            ended_at: format_optional_ts(s.ended_at),
            task_id: optional_uuid_str(s.task_id),
            project_id: optional_uuid_str(s.project_id),
            planned_cycles: s.planned_cycles,
            completed_cycles: s.completed_cycles,
            state: s.state.into(),
            config: s.config.into(),
            created_at: format_ts(s.created_at),
            modified_at: format_ts(s.modified_at),
        }
    }
}

impl From<Habit> for BregaHabit {
    fn from(h: Habit) -> Self {
        let level_name = h.level_name().to_string();
        Self {
            id: uuid_str(h.id),
            sid: h.sid,
            title: h.title,
            identity: h.identity,
            cue: h.cue,
            craving: h.craving,
            response: h.response,
            reward: h.reward,
            direction: h.direction.into(),
            cadence: h.cadence,
            minimum: h.minimum,
            stack_after: optional_uuid_str(h.stack_after),
            stack_delay_s: h.stack_delay_s,
            area_id: optional_uuid_str(h.area_id),
            project_id: optional_uuid_str(h.project_id),
            level: h.level,
            xp: h.xp,
            streak_current: h.streak_current,
            streak_best: h.streak_best,
            level_name,
            created_at: format_ts(h.created_at),
            modified_at: format_ts(h.modified_at),
            archived_at: format_optional_ts(h.archived_at),
        }
    }
}

impl From<HabitEntry> for BregaHabitEntry {
    fn from(e: HabitEntry) -> Self {
        Self {
            id: uuid_str(e.id),
            habit_id: uuid_str(e.habit_id),
            occurred_at: format_ts(e.occurred_at),
            amount: e.amount,
            notes: e.notes,
            slip: e.slip,
            source: e.source,
            created_at: format_ts(e.created_at),
        }
    }
}

// ── Input conversions: FFI → core ────────────────────────────────────

impl BregaNewTask {
    /// Convert to the core `NewTask` type.
    pub(crate) fn to_core(&self) -> Result<NewTask, BregaError> {
        Ok(NewTask {
            title: self.title.clone(),
            notes: self.notes.clone(),
            status: self.status.map(Into::into),
            project_id: parse_optional_uuid(self.project_id.as_deref())?,
            area_id: parse_optional_uuid(self.area_id.as_deref())?,
            heading_id: parse_optional_uuid(self.heading_id.as_deref())?,
            parent_id: None,
            start_date: self.start_date.clone(),
            deadline: self.deadline.clone(),
            scheduled_for: self.scheduled_for.clone(),
            recurrence: self.recurrence.clone(),
            priority: self.priority.map(Into::into),
            estimate_minutes: self.estimate_minutes,
            evening: self.evening,
            udas: UdaValues::from_json(&self.udas),
            tags: self.tags.clone(),
        })
    }
}

impl BregaTaskPatch {
    /// Convert to the core `TaskPatch` type.
    ///
    /// For clearable fields, a `true` clear flag sets the core value to
    /// `Some(None)` (meaning "remove"), otherwise `Some(Some(value))`
    /// when the FFI field is `Some`, and `None` (meaning "don't change")
    /// when both are absent.
    pub(crate) fn to_core(&self) -> Result<TaskPatch, BregaError> {
        let notes = if self.clear_notes {
            Some(None)
        } else {
            self.notes.clone().map(Some)
        };

        let project_id = if self.clear_project {
            Some(None)
        } else {
            parse_optional_uuid(self.project_id.as_deref())?.map(Some)
        };

        let area_id = if self.clear_area {
            Some(None)
        } else {
            parse_optional_uuid(self.area_id.as_deref())?.map(Some)
        };

        let heading_id = if self.clear_heading {
            Some(None)
        } else {
            parse_optional_uuid(self.heading_id.as_deref())?.map(Some)
        };

        let start_date = if self.clear_start_date {
            Some(None)
        } else {
            self.start_date.clone().map(Some)
        };

        let deadline = if self.clear_deadline {
            Some(None)
        } else {
            self.deadline.clone().map(Some)
        };

        let scheduled_for = if self.clear_scheduled {
            Some(None)
        } else {
            self.scheduled_for.clone().map(Some)
        };

        let priority = if self.clear_priority {
            Some(None)
        } else {
            self.priority.map(|p| Some(p.into()))
        };

        let set_udas: BTreeMap<String, serde_json::Value> =
            serde_json::from_str(&self.set_udas).unwrap_or_default();

        Ok(TaskPatch {
            title: self.title.clone(),
            notes,
            status: self.status.map(Into::into),
            project_id,
            area_id,
            heading_id,
            start_date,
            deadline,
            scheduled_for,
            priority,
            evening: self.evening,
            set_udas,
            remove_udas: self.remove_uda_keys.clone(),
            add_tags: self.add_tags.clone(),
            remove_tags: self.remove_tags.clone(),
            add_deps: self.add_deps.clone(),
            remove_deps: self.remove_deps.clone(),
            estimate_minutes: if self.clear_estimate {
                Some(None)
            } else {
                self.estimate_minutes.map(Some)
            },
        })
    }
}

impl BregaAreaPatch {
    /// Convert to the core `AreaPatch` type.
    pub(crate) fn to_core(&self) -> AreaPatch {
        AreaPatch {
            name: self.name.clone(),
            color: if self.clear_color {
                Some(None)
            } else {
                self.color.clone().map(Some)
            },
            weekly_budget_minutes: if self.clear_budget {
                Some(None)
            } else {
                self.weekly_budget_minutes.map(Some)
            },
        }
    }
}

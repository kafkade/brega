//! Planning surface: effort estimates, area budgets, and the Weekly Review.

use brega_core::domain::duration;
use brega_core::domain::review::{AreaReview, ReviewInput, ReviewWeek, weekly_review};

use crate::BregaError;

/// Parse a duration such as `90m`, `2h`, or `1h30m` into whole minutes.
///
/// Used for task estimates and area weekly budgets.
///
/// # Errors
/// [`BregaError::InvalidInput`] for empty, malformed, zero, or too-large
/// input (the message says which).
#[uniffi::export]
pub fn parse_duration_minutes(text: &str) -> Result<u32, BregaError> {
    duration::parse_minutes(text).map_err(|e| BregaError::InvalidInput {
        message: e.to_string(),
    })
}

/// Format whole minutes as `45m`, `2h`, or `1h30m`.
#[uniffi::export]
#[must_use]
pub fn format_duration_minutes(minutes: u32) -> String {
    duration::format_minutes(minutes)
}

// ── Weekly Review ────────────────────────────────────────────────────

/// One row of the Weekly Review.
#[derive(Clone, Debug, uniffi::Record)]
pub struct BregaAreaReview {
    /// Area UUID; `None` for the "No area" row.
    pub area_id: Option<String>,
    /// Area name; `None` for the "No area" row.
    pub name: Option<String>,
    /// Whether the area is archived (listed only when it has activity).
    pub archived: bool,
    /// Weekly budget in minutes, if set.
    pub budget_minutes: Option<u32>,
    /// Tracked time this week, whole seconds.
    pub tracked_seconds: u64,
    /// Sum of estimates of open tasks planned this week, minutes.
    pub planned_minutes: u64,
    /// Open tasks planned this week.
    pub planned_tasks: u32,
    /// Open tasks planned this week without an estimate.
    pub unestimated: u32,
    /// Open tasks past their due date.
    pub overdue: u32,
    /// Open tasks moved later at least twice.
    pub slipped: u32,
    /// No open task planned this week.
    pub zero_planned: bool,
    /// Open high-priority tasks without a planned day.
    pub high_priority_unplanned: u32,
}

/// The Weekly Review for one ISO week.
#[derive(Clone, Debug, uniffi::Record)]
pub struct BregaWeeklyReview {
    /// Monday of the week (`YYYY-MM-DD`).
    pub monday: String,
    /// Active areas in display order, archived areas with activity, then
    /// the "No area" row when it has activity.
    pub areas: Vec<BregaAreaReview>,
}

/// The week to review, resolved by the caller in the device's time zone
/// from one clock reading.
#[derive(Clone, Debug, uniffi::Record)]
pub struct BregaReviewWeek {
    /// Local Monday of the ISO week (`YYYY-MM-DD`).
    pub monday: String,
    /// First instant of that local Monday (RFC 3339). Use
    /// `Calendar.startOfDay`, not a literal 00:00: some zones skip midnight.
    pub start: String,
    /// First instant of the following local Monday (RFC 3339), resolved
    /// independently (DST makes weeks 167 or 169 hours long).
    pub end: String,
    /// Local calendar date of "now" (`YYYY-MM-DD`), for overdue.
    pub today: String,
    /// Current instant (RFC 3339), for running timers.
    pub now: String,
}

fn invalid(message: impl Into<String>) -> BregaError {
    BregaError::InvalidInput {
        message: message.into(),
    }
}

fn parse_date(field: &str, s: &str) -> Result<time::Date, BregaError> {
    brega_core::domain::reschedule::slot_date(s)
        .filter(|_| s.len() == 10)
        .ok_or_else(|| invalid(format!("{field}: expected YYYY-MM-DD, got {s:?}")))
}

fn parse_instant(field: &str, s: &str) -> Result<time::OffsetDateTime, BregaError> {
    time::OffsetDateTime::parse(s, &time::format_description::well_known::Rfc3339).map_err(|_| {
        invalid(format!(
            "{field}: expected an RFC 3339 timestamp, got {s:?}"
        ))
    })
}

impl BregaReviewWeek {
    pub(crate) fn to_core(&self) -> Result<ReviewWeek, BregaError> {
        ReviewWeek::new(
            parse_date("monday", &self.monday)?,
            parse_instant("start", &self.start)?,
            parse_instant("end", &self.end)?,
            parse_date("today", &self.today)?,
            parse_instant("now", &self.now)?,
        )
        .map_err(|e| invalid(e.to_string()))
    }
}

impl From<AreaReview> for BregaAreaReview {
    fn from(r: AreaReview) -> Self {
        Self {
            area_id: r.area_id.map(|id| id.to_string()),
            name: r.name,
            archived: r.archived,
            budget_minutes: r.budget_minutes,
            tracked_seconds: r.tracked_seconds,
            planned_minutes: r.planned_minutes,
            planned_tasks: r.planned_tasks,
            unestimated: r.unestimated,
            overdue: r.overdue,
            slipped: r.slipped,
            zero_planned: r.zero_planned,
            high_priority_unplanned: r.high_priority_unplanned,
        }
    }
}

/// Load the review inputs from the vault and compute the review.
pub fn compute_weekly_review(
    conn: &rusqlite::Connection,
    week: &ReviewWeek,
) -> Result<BregaWeeklyReview, BregaError> {
    use brega_storage::repo::{area_repo, task_repo, task_reschedule_repo, time_block_repo};
    let areas = area_repo::list(conn, true)?;
    let tasks = task_repo::list(conn, true)?;
    let blocks = time_block_repo::list_overlapping(conn, week.start(), week.end())?;
    let reschedules = task_reschedule_repo::list_all(conn)?;
    let review = weekly_review(
        week,
        &ReviewInput {
            areas: &areas,
            tasks: &tasks,
            blocks: &blocks,
            reschedules: &reschedules,
        },
    );
    Ok(BregaWeeklyReview {
        monday: review.monday.to_string(),
        areas: review.areas.into_iter().map(Into::into).collect(),
    })
}

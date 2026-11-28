//! Weekly Review (R-12): per-area budget versus tracked and planned time
//! for one ISO week, plus signals of over-commitment.
//!
//! Pure computation, no I/O. The caller supplies the week as two instants
//! (local Monday start and next local Monday start) resolved with the
//! platform's time-zone database, so DST and skipped or repeated local
//! midnights are handled there rather than guessed here.
//!
//! Definitions (all per area; tasks without a known area go to a "No area"
//! row with `area_id: None`):
//! - **open task**: not done, cancelled, deleted, or Someday.
//! - **tracked**: time blocks overlapping the week, clipped to it, credited
//!   to the area of the block's task (running blocks end at `now`). Blocks
//!   of closed, deleted, or archived-area tasks still count.
//! - **planned**: sum of estimates of open tasks whose planned day
//!   (`scheduled_for`) falls in the week; open tasks planned in the week
//!   without an estimate are counted in `unestimated`, never guessed.
//! - **overdue**: open tasks whose due date is before `today`.
//! - **slipped**: open tasks moved to a later day at least twice (a save
//!   that moves both dates counts once).
//! - **zero planned**: no open task planned in the week.
//! - **high priority unplanned**: open high-priority tasks with no planned day.

use std::collections::{HashMap, HashSet};

use time::{Date, Duration, OffsetDateTime, Time, UtcOffset, Weekday};
use uuid::Uuid;

use crate::domain::area::Area;
use crate::domain::reschedule::{TaskReschedule, slot_date};
use crate::domain::task::{Priority, Task, TaskStatus};
use crate::domain::time_block::TimeBlock;

/// Why a review window was rejected.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ReviewError {
    /// The week must start on a Monday (ISO week).
    #[error("week must start on a Monday")]
    NotMonday,
    /// The week's end instant is not after its start.
    #[error("week end must be after its start")]
    EmptyWindow,
}

/// The week under review and the clock it is evaluated against.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReviewWeek {
    monday: Date,
    start: OffsetDateTime,
    end: OffsetDateTime,
    today: Date,
    now: OffsetDateTime,
}

impl ReviewWeek {
    /// A week from explicit instants: `start` is the first instant of local
    /// Monday and `end` the first instant of the following local Monday.
    /// `today` and `now` should come from the same clock reading.
    ///
    /// # Errors
    /// [`ReviewError::NotMonday`] or [`ReviewError::EmptyWindow`].
    pub fn new(
        monday: Date,
        start: OffsetDateTime,
        end: OffsetDateTime,
        today: Date,
        now: OffsetDateTime,
    ) -> Result<Self, ReviewError> {
        if monday.weekday() != Weekday::Monday {
            return Err(ReviewError::NotMonday);
        }
        if end <= start {
            return Err(ReviewError::EmptyWindow);
        }
        Ok(Self {
            monday,
            start,
            end,
            today,
            now,
        })
    }

    /// A week in a zone with one fixed UTC offset (no DST).
    ///
    /// # Errors
    /// [`ReviewError::NotMonday`].
    pub fn with_fixed_offset(
        monday: Date,
        offset: UtcOffset,
        today: Date,
        now: OffsetDateTime,
    ) -> Result<Self, ReviewError> {
        let start = monday.with_time(Time::MIDNIGHT).assume_offset(offset);
        Self::new(monday, start, start + Duration::weeks(1), today, now)
    }

    /// Monday of the week.
    #[must_use]
    pub const fn monday(&self) -> Date {
        self.monday
    }

    /// First instant of the week.
    #[must_use]
    pub const fn start(&self) -> OffsetDateTime {
        self.start
    }

    /// First instant after the week.
    #[must_use]
    pub const fn end(&self) -> OffsetDateTime {
        self.end
    }

    /// Whether a calendar day falls in the week (Monday..=Sunday).
    fn contains_day(&self, day: Date) -> bool {
        day >= self.monday && day < self.monday + Duration::weeks(1)
    }

    /// Overlap of `[from, to)` with the week, zero if none.
    fn overlap(&self, from: OffsetDateTime, to: OffsetDateTime) -> Duration {
        let lo = from.max(self.start);
        let hi = to.min(self.end);
        if hi > lo { hi - lo } else { Duration::ZERO }
    }
}

/// Everything the review reads. Pass all areas (archived included), all
/// tasks (closed and deleted included), the time blocks that may overlap
/// the week, and the reschedule log.
#[derive(Clone, Copy, Debug)]
pub struct ReviewInput<'a> {
    /// Areas.
    pub areas: &'a [Area],
    /// Tasks.
    pub tasks: &'a [Task],
    /// Time blocks.
    pub blocks: &'a [TimeBlock],
    /// Reschedule log.
    pub reschedules: &'a [TaskReschedule],
}

/// One row of the review.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AreaReview {
    /// Area id; `None` for the "No area" row.
    pub area_id: Option<Uuid>,
    /// Area name; `None` for the "No area" row.
    pub name: Option<String>,
    /// Whether the area is archived (shown only when it has activity).
    pub archived: bool,
    /// Weekly budget in minutes, if set.
    pub budget_minutes: Option<u32>,
    /// Tracked time in the week, whole seconds.
    pub tracked_seconds: u64,
    /// Sum of estimates of open tasks planned in the week.
    pub planned_minutes: u64,
    /// Open tasks planned in the week.
    pub planned_tasks: u32,
    /// Open tasks planned in the week without an estimate.
    pub unestimated: u32,
    /// Open tasks past their due date.
    pub overdue: u32,
    /// Open tasks moved later at least twice.
    pub slipped: u32,
    /// No open task planned in the week.
    pub zero_planned: bool,
    /// Open high-priority tasks without a planned day.
    pub high_priority_unplanned: u32,
}

impl AreaReview {
    const fn has_activity(&self) -> bool {
        self.tracked_seconds > 0
            || self.planned_tasks > 0
            || self.overdue > 0
            || self.slipped > 0
            || self.high_priority_unplanned > 0
    }
}

/// The review: active areas in display order (archived areas only when
/// they have activity), then the "No area" row when it has activity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WeeklyReview {
    /// Monday of the reviewed week.
    pub monday: Date,
    /// Rows.
    pub areas: Vec<AreaReview>,
}

#[derive(Default)]
struct Acc {
    tracked: Duration,
    row: AreaReview,
}

/// Compute the Weekly Review.
#[must_use]
pub fn weekly_review(week: &ReviewWeek, input: &ReviewInput<'_>) -> WeeklyReview {
    let known: HashSet<Uuid> = input.areas.iter().map(|a| a.id).collect();
    let bucket = |area: Option<Uuid>| area.filter(|id| known.contains(id));
    let task_area: HashMap<Uuid, Option<Uuid>> = input
        .tasks
        .iter()
        .map(|t| (t.id, bucket(t.area_id)))
        .collect();
    let mut acc: HashMap<Option<Uuid>, Acc> = HashMap::new();

    for block in input.blocks {
        let end = block.end_ts.unwrap_or(week.now);
        let overlap = week.overlap(block.start_ts, end);
        if overlap > Duration::ZERO {
            let area = block
                .task_id
                .and_then(|id| task_area.get(&id).copied().flatten());
            acc.entry(area).or_default().tracked += overlap;
        }
    }

    let slips = slip_counts(input.reschedules);
    for task in input.tasks.iter().filter(|t| is_open(t)) {
        let row = &mut acc.entry(bucket(task.area_id)).or_default().row;
        let planned_day = task.scheduled_for.as_deref().and_then(slot_date);
        if planned_day.is_some_and(|d| week.contains_day(d)) {
            row.planned_tasks += 1;
            match task.estimate_minutes {
                Some(m) => row.planned_minutes += u64::from(m),
                None => row.unestimated += 1,
            }
        }
        if task
            .deadline
            .as_deref()
            .and_then(slot_date)
            .is_some_and(|d| d < week.today)
        {
            row.overdue += 1;
        }
        if slips.get(&task.id).copied().unwrap_or(0) >= 2 {
            row.slipped += 1;
        }
        if task.priority == Some(Priority::High) && task.scheduled_for.is_none() {
            row.high_priority_unplanned += 1;
        }
    }

    let mut finish = |area: Option<&Area>| -> AreaReview {
        let Acc { tracked, mut row } = acc.remove(&area.map(|a| a.id)).unwrap_or_default();
        row.area_id = area.map(|a| a.id);
        row.name = area.map(|a| a.name.clone());
        row.archived = area.is_some_and(|a| a.archived_at.is_some());
        row.budget_minutes = area.and_then(|a| a.weekly_budget_minutes);
        row.tracked_seconds = u64::try_from(tracked.whole_seconds()).unwrap_or(0);
        row.zero_planned = row.planned_tasks == 0;
        row
    };

    let mut ordered: Vec<&Area> = input.areas.iter().collect();
    ordered.sort_by(|a, b| (a.sort_order, &a.name).cmp(&(b.sort_order, &b.name)));
    let mut rows: Vec<AreaReview> = ordered
        .into_iter()
        .map(|a| finish(Some(a)))
        .filter(|r| !r.archived || r.has_activity())
        .collect();
    let no_area = finish(None);
    if no_area.has_activity() {
        rows.push(no_area);
    }
    WeeklyReview {
        monday: week.monday,
        areas: rows,
    }
}

const fn is_open(task: &Task) -> bool {
    task.deleted_at.is_none()
        && !task.status.is_closed()
        && !matches!(task.status, TaskStatus::Someday)
}

/// Later-move count per task, counting moves saved together once.
fn slip_counts(log: &[TaskReschedule]) -> HashMap<Uuid, u32> {
    let edits: HashSet<(Uuid, OffsetDateTime)> =
        log.iter().map(|r| (r.task_id, r.created_at)).collect();
    let mut out = HashMap::new();
    for (task, _) in edits {
        *out.entry(task).or_insert(0) += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::domain::reschedule::RescheduleField;
    use crate::domain::time_block::BlockSource;
    use crate::domain::uda::UdaValues;
    use proptest::prelude::*;
    use time::macros::{date, datetime, offset};

    const MONDAY: Date = date!(2026 - 06 - 01);

    fn utc_week() -> ReviewWeek {
        ReviewWeek::with_fixed_offset(
            MONDAY,
            offset!(UTC),
            date!(2026 - 06 - 03),
            datetime!(2026-06-03 12:00 UTC),
        )
        .unwrap()
    }

    fn area(name: &str, sort: i32) -> Area {
        Area {
            id: Uuid::now_v7(),
            name: name.into(),
            color: None,
            sort_order: sort,
            weekly_budget_minutes: None,
            created_at: datetime!(2026-01-01 0:00 UTC),
            modified_at: datetime!(2026-01-01 0:00 UTC),
            archived_at: None,
        }
    }

    fn task(area: Option<&Area>) -> Task {
        Task {
            id: Uuid::now_v7(),
            sid: 1,
            title: "t".into(),
            notes: None,
            status: TaskStatus::Pending,
            area_id: area.map(|a| a.id),
            project_id: None,
            heading_id: None,
            parent_id: None,
            start_date: None,
            deadline: None,
            scheduled_for: None,
            recurrence: None,
            priority: None,
            estimate_minutes: None,
            evening: false,
            udas: UdaValues::default(),
            tags: Vec::new(),
            depends_on: Vec::new(),
            checklist: Vec::new(),
            urgency: 0.0,
            created_at: datetime!(2026-01-01 0:00 UTC),
            modified_at: datetime!(2026-01-01 0:00 UTC),
            done_at: None,
            cancelled_at: None,
            deleted_at: None,
        }
    }

    fn block(task: Option<&Task>, start: OffsetDateTime, end: Option<OffsetDateTime>) -> TimeBlock {
        TimeBlock {
            id: Uuid::now_v7(),
            sid: 1,
            title: "b".into(),
            start_ts: start,
            end_ts: end,
            project_id: None,
            task_id: task.map(|t| t.id),
            notes: None,
            source: BlockSource::Timer,
            billable: false,
            created_at: start,
            modified_at: start,
        }
    }

    fn moved(task: &Task, at: OffsetDateTime, field: RescheduleField) -> TaskReschedule {
        TaskReschedule {
            id: Uuid::now_v7(),
            task_id: task.id,
            field,
            from_slot: None,
            to_slot: None,
            created_at: at,
        }
    }

    fn review(
        week: &ReviewWeek,
        areas: &[Area],
        tasks: &[Task],
        blocks: &[TimeBlock],
        reschedules: &[TaskReschedule],
    ) -> WeeklyReview {
        weekly_review(
            week,
            &ReviewInput {
                areas,
                tasks,
                blocks,
                reschedules,
            },
        )
    }

    #[test]
    fn rejects_bad_windows() {
        let now = datetime!(2026-06-03 12:00 UTC);
        assert_eq!(
            ReviewWeek::with_fixed_offset(date!(2026 - 06 - 02), offset!(UTC), MONDAY, now),
            Err(ReviewError::NotMonday)
        );
        assert_eq!(
            ReviewWeek::new(MONDAY, now, now, MONDAY, now),
            Err(ReviewError::EmptyWindow)
        );
    }

    #[test]
    fn computes_each_metric_per_area() {
        let mut work = area("work", 0);
        work.weekly_budget_minutes = Some(600);
        let home = area("home", 1);

        let mut planned = task(Some(&work));
        planned.scheduled_for = Some("2026-06-02T09:00".into());
        planned.estimate_minutes = Some(90);
        let mut planned_sunday = task(Some(&work));
        planned_sunday.scheduled_for = Some("2026-06-07".into());
        planned_sunday.estimate_minutes = Some(30);
        let mut unestimated = task(Some(&work));
        unestimated.scheduled_for = Some("2026-06-04".into());
        let mut next_week = task(Some(&work));
        next_week.scheduled_for = Some("2026-06-08".into());
        next_week.estimate_minutes = Some(999);
        let mut done = task(Some(&work));
        done.status = TaskStatus::Done;
        done.scheduled_for = Some("2026-06-02".into());
        done.estimate_minutes = Some(999);
        let mut overdue = task(Some(&work));
        overdue.deadline = Some("2026-06-02".into());
        let mut due_today = task(Some(&work));
        due_today.deadline = Some("2026-06-03".into());
        let mut urgent = task(Some(&work));
        urgent.priority = Some(Priority::High);
        let mut someday = task(Some(&work));
        someday.status = TaskStatus::Someday;
        someday.priority = Some(Priority::High);
        let slipper = task(Some(&work));

        let t1 = datetime!(2026-05-20 10:00 UTC);
        let t2 = datetime!(2026-05-25 10:00 UTC);
        let log = [
            moved(&slipper, t1, RescheduleField::ScheduledFor),
            moved(&slipper, t2, RescheduleField::ScheduledFor),
            // One save moving both dates of `overdue` counts once.
            moved(&overdue, t1, RescheduleField::ScheduledFor),
            moved(&overdue, t1, RescheduleField::Deadline),
        ];
        let blocks = [block(
            Some(&done),
            datetime!(2026-06-02 9:00 UTC),
            Some(datetime!(2026-06-02 10:30 UTC)),
        )];
        let tasks = [
            planned,
            planned_sunday,
            unestimated,
            next_week,
            done,
            overdue,
            due_today,
            urgent,
            someday,
            slipper,
        ];
        let r = review(
            &utc_week(),
            &[home.clone(), work.clone()],
            &tasks,
            &blocks,
            &log,
        );

        assert_eq!(r.monday, MONDAY);
        assert_eq!(r.areas.len(), 2, "no 'No area' row without activity");
        let w = &r.areas[0];
        assert_eq!(w.area_id, Some(work.id), "ordered by sort_order");
        assert_eq!(w.budget_minutes, Some(600));
        assert_eq!(
            w.tracked_seconds,
            90 * 60,
            "closed tasks' blocks still count"
        );
        assert_eq!(w.planned_minutes, 120);
        assert_eq!(w.planned_tasks, 3);
        assert_eq!(w.unestimated, 1);
        assert_eq!(w.overdue, 1, "due today is not overdue");
        assert_eq!(w.slipped, 1);
        assert_eq!(w.high_priority_unplanned, 1, "Someday tasks are excluded");
        assert!(!w.zero_planned);

        let h = &r.areas[1];
        assert_eq!(h.area_id, Some(home.id));
        assert_eq!(h.budget_minutes, None);
        assert!(h.zero_planned);
        assert_eq!(h.tracked_seconds, 0);
    }

    #[test]
    fn unassigned_and_unknown_area_work_goes_to_the_no_area_row() {
        let a = area("a", 0);
        let ghost = area("ghost", 0); // not passed in: unknown id
        let mut loose = task(None);
        loose.scheduled_for = Some("2026-06-02".into());
        let orphan = task(Some(&ghost));
        let blocks = [
            block(
                Some(&orphan),
                datetime!(2026-06-02 9:00 UTC),
                Some(datetime!(2026-06-02 9:30 UTC)),
            ),
            block(
                None,
                datetime!(2026-06-02 10:00 UTC),
                Some(datetime!(2026-06-02 10:15 UTC)),
            ),
        ];
        let r = review(&utc_week(), &[a], &[loose, orphan], &blocks, &[]);
        let none = r.areas.last().unwrap();
        assert_eq!(none.area_id, None);
        assert_eq!(none.name, None);
        assert_eq!(none.tracked_seconds, 45 * 60);
        assert_eq!(none.planned_tasks, 1);
        assert_eq!(none.unestimated, 1);
    }

    #[test]
    fn archived_areas_show_only_with_activity() {
        let mut quiet = area("quiet", 0);
        quiet.archived_at = Some(datetime!(2026-05-01 0:00 UTC));
        let mut busy = area("busy", 1);
        busy.archived_at = Some(datetime!(2026-05-01 0:00 UTC));
        let t = task(Some(&busy));
        let blocks = [block(
            Some(&t),
            datetime!(2026-06-02 9:00 UTC),
            Some(datetime!(2026-06-02 9:10 UTC)),
        )];
        let r = review(&utc_week(), &[quiet, busy.clone()], &[t], &blocks, &[]);
        assert_eq!(r.areas.len(), 1);
        assert_eq!(r.areas[0].area_id, Some(busy.id));
        assert!(r.areas[0].archived);
    }

    #[test]
    fn blocks_are_clipped_to_the_week_and_running_blocks_end_now() {
        let a = area("a", 0);
        let t = task(Some(&a));
        let blocks = [
            // Sunday 23:00 before the week -> Monday 01:00: 1h inside.
            block(
                Some(&t),
                datetime!(2026-05-31 23:00 UTC),
                Some(datetime!(2026-06-01 1:00 UTC)),
            ),
            // Across midnight inside the week: fully counted (2h).
            block(
                Some(&t),
                datetime!(2026-06-02 23:00 UTC),
                Some(datetime!(2026-06-03 1:00 UTC)),
            ),
            // Running since 11:00, now is 12:00: 1h.
            block(Some(&t), datetime!(2026-06-03 11:00 UTC), None),
            // Entirely next week: 0.
            block(
                Some(&t),
                datetime!(2026-06-08 9:00 UTC),
                Some(datetime!(2026-06-08 10:00 UTC)),
            ),
            // End before start (bad data): 0.
            block(
                Some(&t),
                datetime!(2026-06-02 9:00 UTC),
                Some(datetime!(2026-06-02 8:00 UTC)),
            ),
        ];
        let r = review(&utc_week(), &[a], &[t], &blocks, &[]);
        assert_eq!(r.areas[0].tracked_seconds, 4 * 3600);
    }

    #[test]
    fn dst_weeks_have_their_real_length() {
        let a = area("a", 0);
        let t = task(Some(&a));
        // US spring forward on Sunday 2026-03-08: the week is 167h long.
        let spring = ReviewWeek::new(
            date!(2026 - 03 - 02),
            datetime!(2026-03-02 0:00 -5),
            datetime!(2026-03-09 0:00 -4),
            date!(2026 - 03 - 10),
            datetime!(2026-03-10 12:00 -4),
        )
        .unwrap();
        // US fall back on Sunday 2026-11-01: the week is 169h long.
        let fall = ReviewWeek::new(
            date!(2026 - 10 - 26),
            datetime!(2026-10-26 0:00 -4),
            datetime!(2026-11-02 0:00 -5),
            date!(2026 - 11 - 03),
            datetime!(2026-11-03 12:00 -5),
        )
        .unwrap();
        for (week, hours) in [(spring, 167), (fall, 169)] {
            let all = [block(
                Some(&t),
                datetime!(2026-01-01 0:00 UTC),
                Some(datetime!(2026-12-31 0:00 UTC)),
            )];
            let r = review(
                &week,
                std::slice::from_ref(&a),
                std::slice::from_ref(&t),
                &all,
                &[],
            );
            assert_eq!(r.areas[0].tracked_seconds, hours * 3600);
        }
    }

    #[test]
    fn planned_days_use_local_calendar_dates() {
        let a = area("a", 0);
        let mut sunday_night = task(Some(&a));
        sunday_night.scheduled_for = Some("2026-06-07T23:30".into());
        sunday_night.estimate_minutes = Some(30);
        let mut malformed = task(Some(&a));
        malformed.scheduled_for = Some("someday".into());
        malformed.estimate_minutes = Some(30);
        let r = review(&utc_week(), &[a], &[sunday_night, malformed], &[], &[]);
        assert_eq!(r.areas[0].planned_minutes, 30);
        assert_eq!(r.areas[0].planned_tasks, 1);
    }

    proptest! {
        /// Summing a block set over consecutive weeks equals its total
        /// duration clipped to those weeks.
        #[test]
        fn tracked_time_partitions_across_weeks(
            spans in proptest::collection::vec((0i64..(21 * 86_400), 0i64..(5 * 86_400)), 0..20)
        ) {
            let a = area("a", 0);
            let t = task(Some(&a));
            let origin = datetime!(2026-06-01 0:00 UTC);
            let blocks: Vec<TimeBlock> = spans
                .iter()
                .map(|&(s, len)| {
                    let start = origin + Duration::seconds(s);
                    block(Some(&t), start, Some(start + Duration::seconds(len)))
                })
                .collect();
            let now = origin + Duration::weeks(10);
            let total: u64 = (0..3)
                .map(|w| {
                    let week = ReviewWeek::with_fixed_offset(
                        MONDAY + Duration::weeks(w),
                        offset!(UTC),
                        MONDAY,
                        now,
                    )
                    .unwrap();
                    review(&week, std::slice::from_ref(&a), std::slice::from_ref(&t), &blocks, &[]).areas[0].tracked_seconds
                })
                .sum();
            let horizon = origin + Duration::weeks(3);
            let expected: i64 = blocks
                .iter()
                .map(|b| (b.end_ts.unwrap().min(horizon) - b.start_ts).whole_seconds().max(0))
                .sum();
            prop_assert_eq!(total, u64::try_from(expected).unwrap());
        }
    }
}

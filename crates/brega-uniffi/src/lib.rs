//! # brega-uniffi
//!
//! `UniFFI` scaffolding crate exposing the brega core API to Swift for
//! iOS, iPadOS, macOS, and watchOS apps.
//!
//! Per ADR-005, this crate owns the `unsafe` boundary for the workspace:
//! `UniFFI`-generated code uses `#[no_mangle] pub extern "C"` and
//! `unsafe` blocks. All other crates `forbid` `unsafe_code`.
//!
//! ## Architecture
//!
//! The entry point is [`Workspace`], an opaque object wrapping an
//! unlocked vault. All domain operations — task CRUD, time tracking,
//! focus sessions, habit logging — are methods on `Workspace`.
//!
//! Types are mirrored as UniFFI-compatible records (string UUIDs, RFC
//! 3339 timestamps, JSON blobs for UDAs/cadences). The conversion layer
//! in [`convert`] translates between these and the core domain types.
//!
//! ## Async (future)
//!
//! The current API is synchronous. ADR-005 §4.3.1 calls for `UniFFI`
//! async support backed by a `tokio` current-thread runtime owned by
//! this shim. That will land as a follow-up when the Swift app needs
//! non-blocking vault operations.

mod account;
mod convert;
mod error;
mod planning;
mod types;
mod workspace;

pub use account::{
    AccountLogin, BregaSessionMaterial, BregaSetupCode, BregaSignupBundle, account_login_start,
    account_signup_bundle, emergency_kit_text, parse_setup_code,
};
pub use error::BregaError;
pub use planning::{
    BregaAreaReview, BregaReviewWeek, BregaWeeklyReview, format_duration_minutes,
    parse_duration_minutes,
};
pub use types::*;
pub use workspace::{
    PairingAcceptSession, PairingInviteSession, Workspace, begin_pairing_accept, init_workspace,
    open_workspace,
};

uniffi::setup_scaffolding!();

#[cfg(test)]
mod tests {
    #![allow(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::missing_const_for_fn,
        clippy::panic
    )]

    use std::sync::Arc;

    use super::*;
    use tempfile::tempdir;

    fn temp_workspace() -> (tempfile::TempDir, Arc<Workspace>) {
        let dir = tempdir().expect("tempdir");
        let path = dir
            .path()
            .join("test.bregavault")
            .to_string_lossy()
            .to_string();
        let init = init_workspace(path, b"test-pw".to_vec()).expect("init workspace");
        (dir, init.workspace)
    }

    #[test]
    fn version_matches_crate() {
        assert_eq!(brega_core::VERSION, env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn init_and_open_roundtrip() {
        let dir = tempdir().expect("tempdir");
        let path = dir
            .path()
            .join("rt.bregavault")
            .to_string_lossy()
            .to_string();

        let init = init_workspace(path.clone(), b"pw123".to_vec()).expect("init");
        assert_eq!(init.workspace.vault_path(), path);
        init.workspace.lock().expect("lock");

        let ws2 = open_workspace(path.clone(), b"pw123".to_vec(), init.secret_key).expect("open");
        assert_eq!(ws2.vault_path(), path);
    }

    #[test]
    fn open_nonexistent_returns_vault_not_found() {
        let dir = tempdir().expect("tempdir");
        let path = dir
            .path()
            .join("nope.bregavault")
            .to_string_lossy()
            .to_string();
        let secret_key = brega_crypto::SecretKey::generate()
            .expect("generate")
            .to_emergency_kit(&[0_u8; 16]);
        match open_workspace(path, b"pw".to_vec(), secret_key) {
            Err(BregaError::VaultNotFound) => {}
            other => panic!("expected VaultNotFound, got {other:?}"),
        }
    }

    #[test]
    fn wrong_password_returns_invalid_credentials() {
        let dir = tempdir().expect("tempdir");
        let path = dir
            .path()
            .join("wp.bregavault")
            .to_string_lossy()
            .to_string();
        let init = init_workspace(path.clone(), b"correct".to_vec()).expect("init");
        init.workspace.lock().expect("lock");
        match open_workspace(path, b"wrong".to_vec(), init.secret_key) {
            Err(BregaError::InvalidCredentials) => {}
            other => panic!("expected InvalidCredentials, got {other:?}"),
        }
    }

    #[test]
    fn locked_workspace_returns_locked() {
        let (_dir, ws) = temp_workspace();
        ws.lock().expect("lock");
        match ws.list_tasks() {
            Err(BregaError::Locked) => {}
            other => panic!("expected Locked, got {other:?}"),
        }
    }

    #[test]
    fn task_crud() {
        let (_dir, ws) = temp_workspace();

        let input = BregaNewTask {
            title: "Buy groceries".into(),
            notes: Some("Milk, eggs, bread".into()),
            status: None,
            project_id: None,
            area_id: None,
            heading_id: None,
            start_date: None,
            deadline: None,
            scheduled_for: None,
            recurrence: None,
            priority: Some(BregaPriority::High),
            evening: false,
            udas: "{}".into(),
            tags: vec!["errands".into()],
            estimate_minutes: None,
        };
        let task = ws.add_task(input).expect("add task");
        assert_eq!(task.title, "Buy groceries");
        assert_eq!(task.status, BregaTaskStatus::Inbox);
        assert_eq!(task.priority, Some(BregaPriority::High));
        assert_eq!(task.tags, vec!["errands"]);

        let fetched = ws.get_task(task.sid).expect("get task");
        assert!(fetched.is_some());

        let tasks = ws.list_tasks().expect("list tasks");
        assert_eq!(tasks.len(), 1);

        let done = ws.complete_task(task.sid).expect("complete task");
        assert_eq!(done.status, BregaTaskStatus::Done);
        assert!(done.done_at.is_some());
    }

    #[test]
    fn task_modify_patch() {
        let (_dir, ws) = temp_workspace();

        let input = BregaNewTask {
            title: "Original".into(),
            notes: None,
            status: None,
            project_id: None,
            area_id: None,
            heading_id: None,
            start_date: None,
            deadline: None,
            scheduled_for: None,
            recurrence: None,
            priority: None,
            evening: false,
            udas: "{}".into(),
            tags: vec![],
            estimate_minutes: None,
        };
        let task = ws.add_task(input).expect("add task");

        let patch = BregaTaskPatch {
            title: Some("Updated".into()),
            notes: Some("New notes".into()),
            clear_notes: false,
            status: None,
            project_id: None,
            clear_project: false,
            area_id: None,
            clear_area: false,
            heading_id: None,
            clear_heading: false,
            start_date: None,
            clear_start_date: false,
            deadline: None,
            clear_deadline: false,
            scheduled_for: None,
            clear_scheduled: false,
            priority: Some(BregaPriority::Low),
            clear_priority: false,
            evening: None,
            set_udas: "{}".into(),
            remove_uda_keys: vec![],
            add_tags: vec!["new-tag".into()],
            remove_tags: vec![],
            add_deps: vec![],
            remove_deps: vec![],
            estimate_minutes: None,
            clear_estimate: false,
        };
        let updated = ws.modify_task(task.sid, patch).expect("modify task");
        assert_eq!(updated.title, "Updated");
        assert_eq!(updated.notes.as_deref(), Some("New notes"));
        assert_eq!(updated.priority, Some(BregaPriority::Low));
        assert!(updated.tags.contains(&"new-tag".to_string()));
    }

    #[test]
    fn project_crud() {
        let (_dir, ws) = temp_workspace();

        let input = BregaNewProject {
            name: "Website redesign".into(),
            notes: None,
            area_id: None,
            deadline: None,
        };
        let project = ws.add_project(input).expect("add project");
        assert_eq!(project.name, "Website redesign");

        let projects = ws.list_projects().expect("list projects");
        assert_eq!(projects.len(), 1);
    }

    #[test]
    fn area_crud() {
        let (_dir, ws) = temp_workspace();

        let input = BregaNewArea {
            name: "Health".into(),
            color: Some("#00FF00".into()),
            weekly_budget_minutes: None,
        };
        let area = ws.add_area(input).expect("add area");
        assert_eq!(area.name, "Health");
        assert_eq!(area.color, Some("#00FF00".into()));

        let areas = ws.list_areas().expect("list areas");
        assert_eq!(areas.len(), 1);
    }

    #[test]
    fn time_tracking() {
        let (_dir, ws) = temp_workspace();

        let input = BregaNewTimeBlock {
            title: "Coding".into(),
            task_sid: None,
            project_id: None,
            notes: None,
        };
        let block = ws.start_timer(input).expect("start timer");
        assert!(block.end_ts.is_none());

        let current = ws.current_timer().expect("current timer");
        assert!(current.is_some());

        let stopped = ws.stop_timer(block.sid).expect("stop timer");
        assert!(stopped.end_ts.is_some());

        let resumed = ws.resume_timer().expect("resume timer");
        assert!(resumed.end_ts.is_none());
    }

    #[test]
    fn focus_session_lifecycle() {
        let (_dir, ws) = temp_workspace();

        let input = BregaNewFocusSession {
            task_sid: None,
            project_id: None,
            planned_cycles: 2,
            config: BregaFocusConfig {
                work_minutes: 25,
                short_break_minutes: 5,
                long_break_minutes: 15,
                cycles_before_long_break: 4,
            },
        };
        let session = ws.start_focus(input).expect("start focus");
        assert_eq!(session.state, BregaFocusState::Working);

        let status = ws.focus_status().expect("focus status");
        assert!(status.is_some());

        let paused = ws.pause_focus(session.sid).expect("pause");
        assert_eq!(paused.state, BregaFocusState::Paused);

        let resumed = ws.resume_focus(session.sid).expect("resume");
        assert_eq!(resumed.state, BregaFocusState::Working);

        let cycle1 = ws.complete_focus_cycle(session.sid).expect("cycle 1");
        assert_eq!(cycle1.completed_cycles, 1);
        assert_eq!(cycle1.state, BregaFocusState::ShortBreak);

        let skipped = ws.skip_focus_break(session.sid).expect("skip break");
        assert_eq!(skipped.state, BregaFocusState::Working);

        let cycle2 = ws.complete_focus_cycle(session.sid).expect("cycle 2");
        assert_eq!(cycle2.completed_cycles, 2);
        assert_eq!(cycle2.state, BregaFocusState::Completed);
    }

    #[test]
    fn habit_lifecycle() {
        let (_dir, ws) = temp_workspace();

        let input = BregaNewHabit {
            title: "Read 10 pages".into(),
            identity: Some("I am a reader".into()),
            cue: None,
            craving: None,
            response: None,
            reward: None,
            direction: BregaHabitDirection::Build,
            cadence: r#""daily""#.into(),
            minimum: r#""boolean""#.into(),
            stack_after: None,
            stack_delay_s: 0,
            area_id: None,
            project_id: None,
        };
        let habit = ws.add_habit(input).expect("add habit");
        assert_eq!(habit.title, "Read 10 pages");
        assert_eq!(habit.level_name, "Spark");

        let habits = ws.list_habits().expect("list habits");
        assert_eq!(habits.len(), 1);

        let entry = ws
            .log_habit(habit.sid, "1".into(), Some("Great session".into()), false)
            .expect("log habit");
        assert!(!entry.slip);
        assert_eq!(entry.notes.as_deref(), Some("Great session"));

        ws.archive_habit(habit.sid).expect("archive habit");
        let habits = ws.list_habits().expect("list habits after archive");
        assert!(habits.is_empty());
    }

    #[test]
    fn tags_list() {
        let (_dir, ws) = temp_workspace();

        let input = BregaNewTask {
            title: "Tagged task".into(),
            notes: None,
            status: None,
            project_id: None,
            area_id: None,
            heading_id: None,
            start_date: None,
            deadline: None,
            scheduled_for: None,
            recurrence: None,
            priority: None,
            evening: false,
            udas: "{}".into(),
            tags: vec!["alpha".into(), "beta".into()],
            estimate_minutes: None,
        };
        ws.add_task(input).expect("add task");

        let tags = ws.list_tags().expect("list tags");
        assert_eq!(tags.len(), 2);
        let names: Vec<&str> = tags.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"alpha"));
        assert!(names.contains(&"beta"));
    }

    #[test]
    fn duration_helpers_parse_and_format() {
        assert_eq!(parse_duration_minutes("1h30m").expect("parse"), 90);
        assert!(matches!(
            parse_duration_minutes("0m"),
            Err(BregaError::InvalidInput { .. })
        ));
        assert_eq!(format_duration_minutes(90), "1h30m");
    }

    #[test]
    fn weekly_review_reflects_estimates_budgets_and_timers() {
        use time::format_description::well_known::Rfc3339;
        use time::{Duration, OffsetDateTime, Time};

        let (_dir, ws) = temp_workspace();
        let area = ws
            .add_area(BregaNewArea {
                name: "work".into(),
                color: None,
                weekly_budget_minutes: None,
            })
            .expect("area");
        let area = ws
            .update_area(
                area.id,
                BregaAreaPatch {
                    name: None,
                    color: None,
                    clear_color: false,
                    weekly_budget_minutes: Some(parse_duration_minutes("10h").expect("10h")),
                    clear_budget: false,
                },
            )
            .expect("budget");
        assert_eq!(area.weekly_budget_minutes, Some(600));

        let now = OffsetDateTime::now_utc();
        let today = now.date();
        let monday = today - Duration::days(i64::from(today.weekday().number_days_from_monday()));
        let start = monday.with_time(Time::MIDNIGHT).assume_utc();
        let task = ws
            .add_task(BregaNewTask {
                title: "write spec".into(),
                notes: None,
                status: None,
                project_id: None,
                area_id: Some(area.id.clone()),
                heading_id: None,
                start_date: None,
                deadline: None,
                scheduled_for: Some(today.to_string()),
                recurrence: None,
                priority: None,
                evening: false,
                udas: "{}".into(),
                tags: vec![],
                estimate_minutes: Some(90),
            })
            .expect("task");
        assert_eq!(task.estimate_minutes, Some(90));
        ws.start_timer(BregaNewTimeBlock {
            title: "spec".into(),
            task_sid: Some(task.sid),
            project_id: None,
            notes: None,
        })
        .expect("timer");

        let fmt = |t: OffsetDateTime| t.format(&Rfc3339).expect("fmt");
        let review = ws
            .weekly_review(BregaReviewWeek {
                monday: monday.to_string(),
                start: fmt(start),
                end: fmt(start + Duration::weeks(1)),
                today: today.to_string(),
                now: fmt(now + Duration::minutes(5)),
            })
            .expect("review");
        assert_eq!(review.monday, monday.to_string());
        let row = &review.areas[0];
        assert_eq!(row.area_id.as_deref(), Some(area.id.as_str()));
        assert_eq!(row.budget_minutes, Some(600));
        assert_eq!(row.planned_minutes, 90);
        assert!(!row.zero_planned);
        assert!(row.tracked_seconds >= 299, "running timer counts up to now");

        assert!(matches!(
            ws.weekly_review(BregaReviewWeek {
                monday: today.next_day().map(|d| d.to_string()).unwrap_or_default(),
                start: fmt(start),
                end: fmt(start),
                today: today.to_string(),
                now: fmt(now),
            }),
            Err(BregaError::InvalidInput { .. })
        ));
    }
}

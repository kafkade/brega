// Integration tests for the BregaSwift wrapper against the real Rust core
// (via the BregaFFI XCFramework). These run on macOS through `swift test`.
//
// Build the FFI first:
//
//     cargo xtask xcframework
//
// then:
//
//     cd bindings/swift && swift test

import Foundation
import XCTest

@testable import BregaSwift

final class BregaSwiftTests: XCTestCase {
    /// A throwaway vault path under a unique temp directory.
    private func makeVaultPath() throws -> String {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("brega-tests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        return dir.appendingPathComponent("test.bregavault").path
    }

    private static let password = Data("test-password".utf8)

    // MARK: - Lifecycle

    func testCreateOpenLockRoundTrip() async throws {
        let path = try makeVaultPath()

        let (ws, secretKey) = try await BregaWorkspace.create(path: path, password: Self.password)
        XCTAssertEqual(ws.path, path)
        try await ws.lock()

        // Locked workspace rejects further calls.
        do {
            _ = try await ws.listTasks()
            XCTFail("expected listTasks to throw after lock")
        } catch let error as BregaError {
            guard case .Locked = error else {
                return XCTFail("expected .Locked, got \(error)")
            }
        }

        // Reopen with the right password and Secret Key succeeds.
        let reopened = try await BregaWorkspace.open(
            path: path, password: Self.password, secretKey: secretKey
        )
        XCTAssertEqual(reopened.path, path)
    }

    func testOpenWrongPasswordThrows() async throws {
        let path = try makeVaultPath()
        let (ws, secretKey) = try await BregaWorkspace.create(path: path, password: Self.password)
        try await ws.lock()

        do {
            _ = try await BregaWorkspace.open(
                path: path, password: Data("wrong".utf8), secretKey: secretKey
            )
            XCTFail("expected open to throw on wrong password")
        } catch let error as BregaError {
            guard case .InvalidCredentials = error else {
                return XCTFail("expected .InvalidCredentials, got \(error)")
            }
        }
    }

    // MARK: - Tasks (the acceptance round-trip)

    func testTaskRoundTrip() async throws {
        let (ws, _) = try await BregaWorkspace.create(path: makeVaultPath(), password: Self.password)

        let task = try await ws.addTask(
            BregaNewTask(
                title: "Buy groceries",
                notes: "Milk, eggs, bread",
                status: nil,
                projectId: nil,
                areaId: nil,
                headingId: nil,
                startDate: nil,
                deadline: nil,
                scheduledFor: nil,
                recurrence: nil,
                priority: .high,
                evening: false,
                udas: "{}",
                tags: ["errands"]
            )
        )
        XCTAssertEqual(task.title, "Buy groceries")
        XCTAssertEqual(task.status, .inbox)
        XCTAssertEqual(task.priority, .high)
        XCTAssertEqual(task.tags, ["errands"])

        let fetched = try await ws.getTask(sid: task.sid)
        XCTAssertNotNil(fetched)
        XCTAssertEqual(fetched?.title, "Buy groceries")

        let all = try await ws.listTasks()
        XCTAssertEqual(all.count, 1)

        let patch = BregaTaskPatch(
            title: "Buy more groceries",
            notes: nil,
            clearNotes: false,
            status: nil,
            projectId: nil,
            clearProject: false,
            areaId: nil,
            clearArea: false,
            headingId: nil,
            clearHeading: false,
            startDate: nil,
            clearStartDate: false,
            deadline: nil,
            clearDeadline: false,
            scheduledFor: nil,
            clearScheduled: false,
            priority: .low,
            clearPriority: false,
            evening: nil,
            setUdas: "{}",
            removeUdaKeys: [],
            addTags: ["shopping"],
            removeTags: [],
            addDeps: [],
            removeDeps: []
        )
        let modified = try await ws.modifyTask(sid: task.sid, patch: patch)
        XCTAssertEqual(modified.title, "Buy more groceries")
        XCTAssertEqual(modified.priority, .low)
        XCTAssertTrue(modified.tags.contains("shopping"))

        let done = try await ws.completeTask(sid: task.sid)
        XCTAssertEqual(done.status, .done)
        XCTAssertNotNil(done.doneAt)
    }

    // MARK: - Projects & areas

    func testProjectAndAreaCrud() async throws {
        let (ws, _) = try await BregaWorkspace.create(path: makeVaultPath(), password: Self.password)

        let area = try await ws.addArea(BregaNewArea(name: "Health", color: "#00FF00"))
        XCTAssertEqual(area.name, "Health")
        let areas = try await ws.listAreas()
        XCTAssertEqual(areas.count, 1)

        let project = try await ws.addProject(
            BregaNewProject(name: "Website redesign", notes: nil, areaId: nil, deadline: nil)
        )
        XCTAssertEqual(project.name, "Website redesign")
        let projects = try await ws.listProjects()
        XCTAssertEqual(projects.count, 1)

        let fetched = try await ws.getProject(sid: project.sid)
        XCTAssertEqual(fetched?.name, "Website redesign")
    }

    // MARK: - Tags

    func testTagsList() async throws {
        let (ws, _) = try await BregaWorkspace.create(path: makeVaultPath(), password: Self.password)
        _ = try await ws.addTask(
            BregaNewTask(
                title: "Tagged",
                notes: nil,
                status: nil,
                projectId: nil,
                areaId: nil,
                headingId: nil,
                startDate: nil,
                deadline: nil,
                scheduledFor: nil,
                recurrence: nil,
                priority: nil,
                evening: false,
                udas: "{}",
                tags: ["alpha", "beta"]
            )
        )
        let names = try await ws.listTags().map(\.name)
        XCTAssertTrue(names.contains("alpha"))
        XCTAssertTrue(names.contains("beta"))
    }

    // MARK: - Time tracking

    func testTimeTracking() async throws {
        let (ws, _) = try await BregaWorkspace.create(path: makeVaultPath(), password: Self.password)

        let block = try await ws.startTimer(
            BregaNewTimeBlock(title: "Coding", taskSid: nil, projectId: nil, notes: nil)
        )
        XCTAssertNil(block.endTs)
        let current = try await ws.currentTimer()
        XCTAssertNotNil(current)

        let stopped = try await ws.stopTimer(sid: block.sid)
        XCTAssertNotNil(stopped.endTs)

        let resumed = try await ws.resumeTimer()
        XCTAssertNil(resumed.endTs)
        let blocks = try await ws.listTimeBlocks()
        XCTAssertFalse(blocks.isEmpty)
    }

    // MARK: - Focus sessions

    func testFocusLifecycle() async throws {
        let (ws, _) = try await BregaWorkspace.create(path: makeVaultPath(), password: Self.password)

        let session = try await ws.startFocus(
            BregaNewFocusSession(
                taskSid: nil,
                projectId: nil,
                plannedCycles: 2,
                config: BregaFocusConfig(
                    workMinutes: 25,
                    shortBreakMinutes: 5,
                    longBreakMinutes: 15,
                    cyclesBeforeLongBreak: 4
                )
            )
        )
        XCTAssertEqual(session.state, .working)
        let status = try await ws.focusStatus()
        XCTAssertNotNil(status)

        let paused = try await ws.pauseFocus(sid: session.sid)
        XCTAssertEqual(paused.state, .paused)
        let resumed = try await ws.resumeFocus(sid: session.sid)
        XCTAssertEqual(resumed.state, .working)

        let cycle1 = try await ws.completeFocusCycle(sid: session.sid)
        XCTAssertEqual(cycle1.completedCycles, 1)
        XCTAssertEqual(cycle1.state, .shortBreak)

        let skipped = try await ws.skipFocusBreak(sid: session.sid)
        XCTAssertEqual(skipped.state, .working)

        let cycle2 = try await ws.completeFocusCycle(sid: session.sid)
        XCTAssertEqual(cycle2.completedCycles, 2)
        XCTAssertEqual(cycle2.state, .completed)
    }

    // MARK: - Habits

    func testHabitLifecycle() async throws {
        let (ws, _) = try await BregaWorkspace.create(path: makeVaultPath(), password: Self.password)

        let habit = try await ws.addHabit(
            BregaNewHabit(
                title: "Read 10 pages",
                identity: "I am a reader",
                cue: nil,
                craving: nil,
                response: nil,
                reward: nil,
                direction: .build,
                cadence: "\"daily\"",
                minimum: "\"boolean\"",
                stackAfter: nil,
                stackDelayS: 0,
                areaId: nil,
                projectId: nil
            )
        )
        XCTAssertEqual(habit.title, "Read 10 pages")
        let habits = try await ws.listHabits()
        XCTAssertEqual(habits.count, 1)
        let fetchedHabit = try await ws.getHabit(sid: habit.sid)
        XCTAssertNotNil(fetchedHabit)

        let entry = try await ws.logHabit(
            habitSid: habit.sid,
            amount: "1",
            notes: "Great session",
            slip: false
        )
        XCTAssertFalse(entry.slip)
        XCTAssertEqual(entry.notes, "Great session")

        try await ws.archiveHabit(sid: habit.sid)
        let remaining = try await ws.listHabits()
        XCTAssertTrue(remaining.isEmpty)
    }

    // MARK: - Planning

    func testWeeklyReviewWithBudgetAndEstimate() async throws {
        let (ws, _) = try await BregaWorkspace.create(path: makeVaultPath(), password: Self.password)
        let area = try await ws.addArea(BregaNewArea(name: "Work", color: nil))
        let budgeted = try await ws.updateArea(
            id: area.id,
            patch: BregaAreaPatch(weeklyBudgetMinutes: try parseDurationMinutes(text: "10h"))
        )
        XCTAssertEqual(budgeted.weeklyBudgetMinutes, 600)

        let week = try XCTUnwrap(BregaReviewWeek.containing())
        _ = try await ws.addTask(
            BregaNewTask(
                title: "Write spec", notes: nil, status: nil, projectId: nil, areaId: area.id,
                headingId: nil, startDate: nil, deadline: nil, scheduledFor: week.today,
                recurrence: nil, priority: nil, evening: false, udas: "{}", tags: [],
                estimateMinutes: 90
            )
        )
        let review = try await ws.weeklyReview(week)
        let row = try XCTUnwrap(review.areas.first)
        XCTAssertEqual(row.areaId, area.id)
        XCTAssertEqual(row.budgetMinutes, 600)
        XCTAssertEqual(row.plannedMinutes, 90)
        XCTAssertFalse(row.zeroPlanned)
        XCTAssertEqual(formatDurationMinutes(minutes: 90), "1h30m")
    }

    func testReviewWeekFollowsLocalDST() throws {
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = try XCTUnwrap(TimeZone(identifier: "America/New_York"))
        let parse = ISO8601DateFormatter()
        parse.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        let wednesday = try XCTUnwrap(parse.date(from: "2026-03-04T15:00:00.000Z"))
        let week = try XCTUnwrap(BregaReviewWeek.containing(wednesday, calendar: calendar))
        XCTAssertEqual(week.monday, "2026-03-02")
        let start = try XCTUnwrap(parse.date(from: week.start))
        let end = try XCTUnwrap(parse.date(from: week.end))
        XCTAssertEqual(end.timeIntervalSince(start), 167 * 3600, "spring-forward week")
        XCTAssertEqual(week.today, "2026-03-04")
    }
}

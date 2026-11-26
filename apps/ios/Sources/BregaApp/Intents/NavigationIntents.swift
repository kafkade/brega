import AppIntents
import Foundation

// MARK: - Show Today Intent

/// Siri: "Show today in Brega"
///
/// Opens the app to the Today view and returns a brief summary
/// of today's agenda via dialog.
struct ShowTodayIntent: AppIntent {
    static var title: LocalizedStringResource = "Show Today"
    static var description: IntentDescription = "Opens brega and shows today's tasks."
    static var openAppWhenRun = true

    func perform() async throws -> some IntentResult & ProvidesDialog & OpensIntent {
        let client = try await VaultGateway.shared.client()
        let tasks = try await client.listTasks(filter: .today)
        let count = tasks.count
        let message = count == 0
            ? "All clear — no tasks today! 🎉"
            : "\(count) task\(count == 1 ? "" : "s") today. Opening brega…"

        return .result(opensIntent: OpenViewIntent(view: .today), dialog: "\(message)")
    }
}

// MARK: - Open View Intent

/// Siri: "Open inbox in Brega"
///
/// Navigates the app to a specific view. Uses the `brega://` deep-link
/// scheme handled by BregaApp's `.onOpenURL` handler.
struct OpenViewIntent: AppIntent {
    static var title: LocalizedStringResource = "Open View"
    static var description: IntentDescription = "Opens a specific view in brega."
    static var openAppWhenRun = true

    @Parameter(title: "View")
    var view: BregaView

    init() {}

    init(view: BregaView) {
        self.view = view
    }

    func perform() async throws -> some IntentResult & ProvidesDialog {
        return .result(dialog: "Opening \(view.rawValue)…")
    }
}

// MARK: - Run Report Intent

/// Siri: "Run standup report"
///
/// Executes a named report and returns a summary dialog.
/// In production, this would generate the full report via CoreActor.
struct RunReportIntent: AppIntent {
    static var title: LocalizedStringResource = "Run Report"
    static var description: IntentDescription = "Runs a custom report in brega."
    static var openAppWhenRun = false

    @Parameter(title: "Report")
    var report: ReportEntity

    func perform() async throws -> some IntentResult & ProvidesDialog {
        // TODO: In production, execute the report via CoreActor and
        // return structured results. For now, return a mock summary.
        switch report.id {
        case "r-standup":
            let client = try await VaultGateway.shared.client()
            let today = try await client.listTasks(filter: .today)
            return .result(dialog: "Standup: \(today.count) tasks today. Report ready in brega.")

        case "r-weekly":
            return .result(dialog: "Weekly Review: 0 tasks completed. Open brega for details.")

        case "r-time":
            let client = try await VaultGateway.shared.client()
            let blocks = try await client.listTimeBlocks()
            let total = blocks.reduce(0.0) { $0 + $1.duration }
            let hours = Int(total / 3600)
            let minutes = Int((total.truncatingRemainder(dividingBy: 3600)) / 60)
            return .result(dialog: "Time today: \(hours)h \(minutes)m tracked.")

        default:
            return .result(dialog: "Report '\(report.name)' is not available yet.")
        }
    }
}

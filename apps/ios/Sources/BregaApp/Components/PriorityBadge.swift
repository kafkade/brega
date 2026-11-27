import SwiftUI

/// Small colored circle indicating task priority.
struct PriorityBadge: View {
    let priority: Priority

    private var color: Color {
        switch priority {
        case .high: BregaTheme.Colors.priorityHigh
        case .medium: BregaTheme.Colors.priorityMedium
        case .low: BregaTheme.Colors.priorityLow
        }
    }

    var body: some View {
        Circle()
            .fill(color)
            .frame(width: 8, height: 8)
            .accessibilityLabel("\(priority.accessibilityName) priority")
    }
}

private extension Priority {
    var accessibilityName: String {
        switch self {
        case .high: "High"
        case .medium: "Medium"
        case .low: "Low"
        }
    }
}

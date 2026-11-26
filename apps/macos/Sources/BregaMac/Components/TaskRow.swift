// NOTE: Shared with apps/ios — extract to shared package when apps/shared is created.

import SwiftUI

/// A single task row for list views.
///
/// Shows priority indicator, title, tags, deadline, and urgency.
/// Swipe actions for complete and delete.
struct TaskRow: View {
    let task: TaskItem
    var onComplete: (() -> Void)?
    var onDelete: (() -> Void)?

    var body: some View {
        HStack(spacing: BregaTheme.Spacing.sm) {
            // Priority indicator
            if let priority = task.priority {
                PriorityBadge(priority: priority)
            }

            VStack(alignment: .leading, spacing: BregaTheme.Spacing.xxs) {
                HStack {
                    Text(task.title)
                        .font(.body)
                        .strikethrough(task.status == .done || task.status == .cancelled)
                        .foregroundStyle(
                            task.status == .done || task.status == .cancelled
                                ? .secondary : .primary
                        )

                    if task.evening {
                        Image(systemName: "moon.fill")
                            .font(.caption2)
                            .foregroundStyle(.secondary)
                            .accessibilityLabel("Evening task")
                    }
                }

                HStack(spacing: BregaTheme.Spacing.xs) {
                    if let deadline = task.deadline {
                        DeadlineLabel(date: deadline)
                    }

                    if !task.tags.isEmpty {
                        HStack(spacing: BregaTheme.Spacing.xxs) {
                            ForEach(task.tags.prefix(3), id: \.self) { tag in
                                TagChip(name: tag)
                            }
                            if task.tags.count > 3 {
                                Text("+\(task.tags.count - 3)")
                                    .font(.caption2)
                                    .foregroundStyle(.secondary)
                            }
                        }
                    }
                }
            }

            Spacer()

            Text(String(format: "%.1f", task.urgency))
                .font(.caption)
                .foregroundStyle(.secondary)
                .monospacedDigit()
                .accessibilityLabel("Urgency \(String(format: "%.1f", task.urgency))")
        }
        .padding(.vertical, BregaTheme.Spacing.xxs)
        .accessibilityElement(children: .combine)
        .swipeActions(edge: .leading) {
            if task.status != .done {
                Button {
                    onComplete?()
                } label: {
                    Label("Done", systemImage: "checkmark")
                }
                .tint(BregaTheme.Colors.success)
            }
        }
        .swipeActions(edge: .trailing, allowsFullSwipe: false) {
            Button(role: .destructive) {
                onDelete?()
            } label: {
                Label("Delete", systemImage: "trash")
            }
        }
    }
}

/// Deadline label with color coding.
private struct DeadlineLabel: View {
    let date: Date

    private var isOverdue: Bool {
        date < Date()
    }

    private var isToday: Bool {
        Calendar.current.isDateInToday(date)
    }

    var body: some View {
        Label {
            Text(date, style: .date)
                .font(.caption)
        } icon: {
            Image(systemName: "calendar")
                .font(.caption2)
        }
        .foregroundStyle(isOverdue ? .red : isToday ? .orange : .secondary)
        .accessibilityLabel(isOverdue ? "Overdue" : isToday ? "Due today" : "Due \(date.formatted(date: .abbreviated, time: .omitted))")
    }
}

import SwiftUI

/// Small pill-shaped tag label.
struct TagChip: View {
    let name: String
    var color: Color = .secondary

    var body: some View {
        Text(name)
            .font(.caption2)
            .padding(.horizontal, BregaTheme.Spacing.xs)
            .padding(.vertical, BregaTheme.Spacing.xxs)
            .background(color.opacity(0.15), in: Capsule())
            .foregroundStyle(color)
    }
}

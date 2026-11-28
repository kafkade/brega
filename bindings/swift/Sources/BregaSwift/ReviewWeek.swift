import BregaFFI
import Foundation

extension BregaReviewWeek {
    /// The ISO week containing `date`, resolved in `calendar`'s time zone.
    ///
    /// Week boundaries come from Foundation's time-zone database: each is
    /// the first instant of its local Monday (not a literal 00:00, which
    /// some zones skip), so DST weeks are 167 or 169 hours long. `today`
    /// and `now` come from the same `date`.
    public static func containing(_ date: Date = Date(), calendar: Calendar = .current) -> BregaReviewWeek? {
        var iso = Calendar(identifier: .iso8601)
        iso.timeZone = calendar.timeZone
        guard let interval = iso.dateInterval(of: .weekOfYear, for: date) else { return nil }
        let instant = ISO8601DateFormatter()
        instant.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        let day = DateFormatter()
        day.calendar = iso
        day.timeZone = iso.timeZone
        day.locale = Locale(identifier: "en_US_POSIX")
        day.dateFormat = "yyyy-MM-dd"
        return BregaReviewWeek(
            monday: day.string(from: interval.start),
            start: instant.string(from: interval.start),
            end: instant.string(from: interval.end),
            today: day.string(from: date),
            now: instant.string(from: date)
        )
    }
}

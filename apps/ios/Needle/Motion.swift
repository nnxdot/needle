import SwiftUI
import UIKit

/// Needle's one way to move things. Every animation goes through here, so Reduce motion (the
/// iPhone's, or Needle's own switch in Settings › Appearance) stops all of them in one place:
/// moves, springs, and zooms then rest, and only plain fades stay.
@MainActor
enum Motion {
    /// Reduce motion is on: the iPhone's switch, or Needle's.
    static var reduced: Bool {
        UIAccessibility.isReduceMotionEnabled || Store.shared?.app.reduceMotion == true
    }

    /// `animation`, or none with Reduce motion: for moves, springs, and growing.
    static func of(_ animation: Animation) -> Animation? {
        reduced ? nil : animation
    }

    /// `animation`, or a short plain fade with Reduce motion: for what only appears or goes.
    /// What appears starts fast and settles (ease-out), never slow at first.
    static func fade(_ animation: Animation = .easeOut(duration: 0.2)) -> Animation {
        reduced ? .easeInOut(duration: 0.2) : animation
    }

    /// A view coming in or going: `transition`, or a plain fade with Reduce motion.
    static func transition(_ transition: AnyTransition) -> AnyTransition {
        reduced ? .opacity : transition
    }

    /// A spring with no bounce that starts at the finger's speed: `velocity` in points a second,
    /// `distance` the points still to go (signed, as the velocity), for what goes on after a drag
    /// ends. None with Reduce motion.
    static func thrown(_ velocity: CGFloat, over distance: CGFloat, duration: Double = 0.35, bounce: Double = 0) -> Animation? {
        let start = abs(distance) > 1 ? Double(velocity / distance) : 0
        return of(.interpolatingSpring(duration: duration, bounce: bounce, initialVelocity: min(max(start, -30), 30)))
    }

    /// A drag past an end: it goes less and less far, as iOS's own lists do, never past `limit`.
    static func rubberBand(_ offset: CGFloat, limit: CGFloat) -> CGFloat {
        guard limit > 0 else { return 0 }
        let pulled = abs(offset)
        let moved = (1 - 1 / (pulled * 0.55 / limit + 1)) * limit
        return offset < 0 ? -moved : moved
    }
}

/// What a key does shows at once: a keyboard command never waits for an animation.
@MainActor
@discardableResult
func withoutMotion<Result>(_ body: () throws -> Result) rethrows -> Result {
    var transaction = Transaction()
    transaction.disablesAnimations = true
    return try withTransaction(transaction, body)
}

/// `withAnimation`, through `Motion`: no animation with Reduce motion.
@MainActor
func withMotion<Result>(_ animation: Animation, _ body: () throws -> Result) rethrows -> Result {
    try withAnimation(Motion.of(animation), body)
}

/// `withAnimation` for a fade, through `Motion`: a plain short fade with Reduce motion.
@MainActor
func withFade<Result>(_ animation: Animation = .easeOut(duration: 0.2), _ body: () throws -> Result) rethrows -> Result {
    try withAnimation(Motion.fade(animation), body)
}

extension View {
    /// `.animation(_:value:)`, through `Motion`: none with Reduce motion.
    func motion<V: Equatable>(_ animation: Animation, value: V) -> some View {
        modifier(MotionModifier(animation: animation, value: value))
    }

    /// A small bounce of the symbol on each change of `trigger` (the user's own press), resting
    /// with Reduce motion.
    func pressBounce<V: Equatable>(_ trigger: V) -> some View {
        modifier(PressBounce(trigger: trigger))
    }
}

private struct MotionModifier<V: Equatable>: ViewModifier {
    @Environment(\.accessibilityReduceMotion) private var system
    @Environment(Store.self) private var store
    let animation: Animation
    let value: V

    func body(content: Content) -> some View {
        content.animation(system || store.app.reduceMotion ? nil : animation, value: value)
    }
}

private struct PressBounce<V: Equatable>: ViewModifier {
    @Environment(\.accessibilityReduceMotion) private var system
    @Environment(Store.self) private var store
    let trigger: V

    func body(content: Content) -> some View {
        if system || store.app.reduceMotion {
            content
        } else {
            content.symbolEffect(.bounce, options: .nonRepeating, value: trigger)
        }
    }
}

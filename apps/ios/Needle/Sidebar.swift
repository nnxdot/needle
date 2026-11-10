import SwiftUI

/// The sidebar of a wide window, as the Mac's (apps/macos Sidebar.swift) and the system's own
/// lists: the chosen row on a quiet capsule of the ink, its icon in the accent and its words in
/// the ink; the other icons quiet. Amber marks only what plays and the one main button
/// (NEEDLE-2.0.md, rule 4), not the page open. The sidebar also takes the dark or light of the
/// page beside it, with the page's fade, so a page in its cover's colours never stands beside a
/// sidebar of the other kind.
struct SidebarLook: UIViewRepresentable {
    /// The page beside the sidebar is dark.
    let dark: Bool
    /// The chosen row's icon: the accent (on a dark sidebar and on a light one).
    let accent: UIColor
    /// The change comes at once (Reduce Motion).
    let reduced: Bool

    func makeCoordinator() -> SidebarStyler { SidebarStyler() }

    func makeUIView(context: Context) -> Probe {
        let view = Probe()
        view.styler = context.coordinator
        view.isUserInteractionEnabled = false
        view.isHidden = true
        return view
    }

    /// Attaches as soon as it is in the window, before the sidebar is first drawn.
    final class Probe: UIView {
        weak var styler: SidebarStyler?

        override func didMoveToWindow() {
            super.didMoveToWindow()
            styler?.attach(from: self)
        }
    }

    func updateUIView(_ view: Probe, context: Context) {
        let styler = context.coordinator
        let (dark, accent, reduced) = (dark, accent, reduced)
        // After this update is laid out: the tab bar controller is an ancestor's.
        DispatchQueue.main.async {
            styler.attach(from: view)
            styler.accent = accent
            styler.side(dark: dark, fade: !reduced)
        }
    }
}

/// Styles the rows of the system's sidebar (`UITabBarController.Sidebar`) and sets its light or
/// dark. Whatever SwiftUI asks of the sidebar goes on to SwiftUI's own delegate.
@MainActor
final class SidebarStyler: NSObject, UITabBarController.Sidebar.Delegate {
    private weak var controller: UITabBarController?
    private weak var inner: (any UITabBarController.Sidebar.Delegate)?
    private var dark: Bool?
    var accent: UIColor = .tintColor {
        didSet { if accent != oldValue { reconfigure() } }
    }

    /// The chosen row's capsule: the ink, faint (as the Mac's chosen icon on the folded strip).
    private static let chosen = UIColor { $0.userInterfaceStyle == .dark ? UIColor(white: 1, alpha: 0.12) : UIColor(white: 0, alpha: 0.08) }

    func attach(from view: UIView) {
        #if DEBUG
        // Test builds: -plain-sidebar leaves the system's sidebar as it is, to compare.
        if CommandLine.arguments.contains("-plain-sidebar") { return }
        #endif
        guard let root = view.window?.rootViewController, let found = Self.tabs(in: root) else { return }
        controller = found
        guard found.sidebar.delegate !== self else { return }
        inner = found.sidebar.delegate
        found.sidebar.delegate = self
        #if DEBUG
        if CommandLine.arguments.contains("-dump-sidebar") {
            SidebarDump.note("attached; SwiftUI's was \(String(describing: inner)); shown \(Self.sidebar(in: found) != nil)")
            SidebarDump.write(found)
        }
        #endif
        #if DEBUG
        // Test builds: -close-sidebar closes the sidebar a moment after the start, as its button
        // does (-sidebar-hidden only sets where it starts, which a window on its side ignores).
        if CommandLine.arguments.contains("-close-sidebar") {
            DispatchQueue.main.asyncAfter(deadline: .now() + 3) { found.sidebar.isHidden = true }
        }
        #endif
        // The sidebar learns what its delegate does when it is made: one already shown is made
        // again at once (no animation, so nothing is seen), and asks this one for its rows.
        if Self.sidebar(in: found) != nil {
            UIView.performWithoutAnimation {
                found.sidebar.isHidden = true
                found.view.layoutIfNeeded()
                found.sidebar.isHidden = false
                found.view.layoutIfNeeded()
            }
        }
    }

    /// The first tab bar controller under `vc` (not one presented over it).
    private static func tabs(in vc: UIViewController) -> UITabBarController? {
        if let tabs = vc as? UITabBarController { return tabs }
        for child in vc.children {
            if let found = tabs(in: child) { return found }
        }
        return nil
    }

    /// Every row again, after the accent changed.
    private func reconfigure() {
        guard let controller else { return }
        func walk(_ tabs: [UITab]) {
            for tab in tabs {
                controller.sidebar.reconfigureItem(for: tab)
                if let group = tab as? UITabGroup { walk(group.children) }
            }
        }
        walk(controller.tabs)
    }

    /// The sidebar dark or light, as the page beside it, with the page's fade.
    func side(dark: Bool, fade: Bool) {
        guard let controller, let side = Self.sidebar(in: controller) else { return }
        let style: UIUserInterfaceStyle = dark ? .dark : .light
        guard side.overrideUserInterfaceStyle != style else {
            self.dark = dark
            return
        }
        // The first time at once: the sidebar comes in as it should be.
        if fade, self.dark != nil {
            UIView.transition(with: side, duration: 0.35, options: [.transitionCrossDissolve, .curveEaseOut, .allowUserInteraction]) {
                side.overrideUserInterfaceStyle = style
                side.layoutIfNeeded()
            }
        } else {
            side.overrideUserInterfaceStyle = style
        }
        self.dark = dark
    }

    /// The sidebar's own view, with its glass: the widest part of the controller's views that
    /// holds the list along the window's leading edge and is no part of the page.
    private static func sidebar(in controller: UITabBarController) -> UIView? {
        guard let root = controller.view, !controller.sidebar.isHidden else { return nil }
        var lists: [UICollectionView] = []
        func find(_ view: UIView) {
            if let list = view as? UICollectionView { lists.append(list) }
            view.subviews.forEach(find)
        }
        find(root)
        let width = root.bounds.width
        let leftToRight = root.effectiveUserInterfaceLayoutDirection == .leftToRight
        for list in lists where list.window != nil {
            let frame = list.convert(list.bounds, to: root)
            let atEdge = leftToRight ? frame.minX < 40 : frame.maxX > width - 40
            guard atEdge, frame.width < width * 0.5, frame.height > root.bounds.height * 0.5 else { continue }
            var side: UIView = list
            while let up = side.superview, up !== root, up.bounds.width < width * 0.5 { side = up }
            return side
        }
        return nil
    }

    // MARK: UITabBarController.Sidebar.Delegate

    override func responds(to selector: Selector!) -> Bool {
        super.responds(to: selector) || inner?.responds(to: selector) == true
    }

    override func forwardingTarget(for selector: Selector!) -> Any? {
        inner?.responds(to: selector) == true ? inner : nil
    }

    func tabBarController(_ tabBarController: UITabBarController, sidebar: UITabBarController.Sidebar, itemFor request: UITabSidebarItem.Request) -> UITabSidebarItem {
        let item = inner?.tabBarController?(tabBarController, sidebar: sidebar, itemFor: request) ?? UITabSidebarItem(request: request)
        #if DEBUG
        if CommandLine.arguments.contains("-dump-sidebar") { SidebarDump.note("made \(item.content)") }
        #endif
        self.tabBarController(tabBarController, sidebar: sidebar, update: item)
        return item
    }

    func tabBarController(_ tabBarController: UITabBarController, sidebar: UITabBarController.Sidebar, update item: UITabSidebarItem) {
        inner?.tabBarController?(tabBarController, sidebar: sidebar, update: item)
        #if DEBUG
        if CommandLine.arguments.contains("-dump-sidebar") {
            SidebarDump.note("\(item.content) selected \(item.configurationState.isSelected) content \(type(of: item.contentConfiguration)) background \(item.backgroundConfiguration)")
        }
        #endif
        // A section's action (New Playlist): in the ink, as the rows, not the accent.
        if case .action = item.content, var content = item.contentConfiguration as? UIListContentConfiguration {
            content.imageProperties.tintColor = .secondaryLabel
            content.imageProperties.tintColorTransformer = nil
            content.textProperties.color = .label
            content.textProperties.colorTransformer = nil
            item.contentConfiguration = content
            return
        }
        // A section's heading (Library, Playlists) keeps the system's look.
        guard case .tab(let tab) = item.content, !(tab is UITabGroup) else { return }
        let selected = item.configurationState.isSelected
        if selected {
            var background = item.backgroundConfiguration
            background.backgroundColor = Self.chosen
            background.backgroundColorTransformer = nil
            item.backgroundConfiguration = background
        } else {
            item.backgroundConfiguration = item.defaultBackgroundConfiguration()
        }
        if var content = item.contentConfiguration as? UIListContentConfiguration {
            content.imageProperties.tintColor = selected ? accent : .secondaryLabel
            content.imageProperties.tintColorTransformer = nil
            content.textProperties.color = .label
            content.textProperties.colorTransformer = nil
            item.contentConfiguration = content
        }
    }

    func tabBarController(_ tabBarController: UITabBarController, sidebarVisibilityWillChange sidebar: UITabBarController.Sidebar,
                          animator: any UITabBarController.Sidebar.Animating) {
        inner?.tabBarController?(tabBarController, sidebarVisibilityWillChange: sidebar, animator: animator)
        // Shown again: its light or dark as before.
        animator.addCompletion { [weak self] in
            guard let self, let dark = self.dark else { return }
            self.dark = nil
            self.side(dark: dark, fade: false)
        }
    }
}

extension UIColor {
    /// The chosen row's icon on a dark sidebar and on a light one: the theme's amber has a
    /// deeper shade on light (as Day's); a colour of the listener's (or the cover's) is the same
    /// on both.
    static func sidebarAccent(_ accent: Color) -> UIColor {
        guard accent == ThemeColors.amber || accent == ThemeColors.amberOnLight else { return UIColor(accent) }
        let dark = UIColor(ThemeColors.amber), light = UIColor(ThemeColors.amberOnLight)
        return UIColor { $0.userInterfaceStyle == .dark ? dark : light }
    }
}

#if DEBUG
/// Test builds: -dump-sidebar writes the tab bar controller's views (kinds and places) to
/// needle-sidebar.log, once, to see where the sidebar is.
@MainActor
enum SidebarDump {
    private static var done = false
    private static var notes: [String] = []

    static func note(_ line: String) {
        notes.append(line)
        try? notes.suffix(60).joined(separator: "\n").write(toFile: NSTemporaryDirectory() + "needle-sidebar-items.log", atomically: true, encoding: .utf8)
    }

    static func write(_ controller: UITabBarController) {
        guard !done else { return }
        done = true
        // Once the sidebar has come in.
        Task {
            try? await Task.sleep(for: .seconds(4))
            dump(controller)
        }
    }

    private static func dump(_ controller: UITabBarController) {
        guard let root = controller.view else { return }
        var lines: [String] = []
        func vcs(_ vc: UIViewController, _ depth: Int) {
            lines.append(String(repeating: "  ", count: depth) + "VC \(type(of: vc)) \(vc.view.frame) style \(vc.overrideUserInterfaceStyle.rawValue)")
            vc.children.forEach { vcs($0, depth + 1) }
        }
        vcs(controller, 0)
        func views(_ view: UIView, _ depth: Int) {
            guard depth < 9 else { return }
            lines.append(String(repeating: "  ", count: depth) + "\(type(of: view)) \(view.convert(view.bounds, to: root))")
            view.subviews.forEach { views($0, depth + 1) }
        }
        views(root, 0)
        let window = controller.view.window
        lines.append("window safe area \(window?.safeAreaInsets ?? .zero), status bar \(window?.windowScene?.statusBarManager?.statusBarFrame ?? .zero)")
        lines.append("sidebar delegate was \(String(describing: controller.sidebar.delegate))")
        try? lines.joined(separator: "\n").write(toFile: NSTemporaryDirectory() + "needle-sidebar.log", atomically: true, encoding: .utf8)
    }
}
#endif

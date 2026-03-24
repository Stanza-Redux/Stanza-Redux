import SwiftUI
import Stanza

private typealias AppRootView = StanzaRootView
private typealias AppDelegate = StanzaAppDelegate

/// The entry point to the app simply loads the App implementation from SPM module.
@main struct AppMain: App {
    #if canImport(UIKit)
    @UIApplicationDelegateAdaptor(AppMainDelegate.self) var appDelegate
    #endif
    @Environment(\.scenePhase) private var scenePhase

    var body: some Scene {
        WindowGroup {
            StanzaRootView()
                .onOpenURL { url in
                    if url.pathExtension.lowercased() == "epub" {
                        AppDelegate.shared.openEpubFile(url: url)
                    }
                }
        }
        .onChange(of: scenePhase) { oldPhase, newPhase in
            switch newPhase {
            case .active:
                AppDelegate.shared.onResume()
            case .inactive:
                AppDelegate.shared.onPause()
            case .background:
                AppDelegate.shared.onStop()
            @unknown default:
                print("unknown app phase: \(newPhase)")
            }
        }
    }
}

#if canImport(UIKit)
typealias AppDelegateAdaptor = UIApplicationDelegateAdaptor
typealias AppMainDelegateBase = UIApplicationDelegate
typealias AppType = UIApplication
#elseif canImport(AppKit)
typealias AppDelegateAdaptor = NSApplicationDelegateAdaptor
typealias AppMainDelegateBase = NSApplicationDelegate
typealias AppType = NSApplication
#endif

@MainActor final class AppMainDelegate: NSObject, AppMainDelegateBase {
    let application = AppType.shared

    #if canImport(UIKit)
    func application(_ application: UIApplication, willFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey : Any]? = nil) -> Bool {
        AppDelegate.shared.onInit()
        return true
    }

    func application(_ application: UIApplication, didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey : Any]? = nil) -> Bool {
        AppDelegate.shared.onLaunch()
        return true
    }

    func applicationWillTerminate(_ application: UIApplication) {
        AppDelegate.shared.onDestroy()
    }

    func applicationDidReceiveMemoryWarning(_ application: UIApplication) {
        AppDelegate.shared.onLowMemory()
    }

    // Handle opening .epub files from other apps or the Files app
    func application(_ app: UIApplication, open url: URL, options: [UIApplication.OpenURLOptionsKey : Any] = [:]) -> Bool {
        if url.pathExtension.lowercased() == "epub" {
            AppDelegate.shared.openEpubFile(url: url)
            return true
        }
        return false
    }

    // support for SkipNotify.fetchNotificationToken()

    func application(_ application: UIApplication, didRegisterForRemoteNotificationsWithDeviceToken deviceToken: Data) {
        NotificationCenter.default.post(name: NSNotification.Name("didRegisterForRemoteNotificationsWithDeviceToken"), object: application, userInfo: ["deviceToken": deviceToken])
    }

    func application(_ application: UIApplication, didFailToRegisterForRemoteNotificationsWithError error: any Error) {
        NotificationCenter.default.post(name: NSNotification.Name("didFailToRegisterForRemoteNotificationsWithError"), object: application, userInfo: ["error": error])
    }
    #elseif canImport(AppKit)
    func applicationWillFinishLaunching(_ notification: Notification) {
        AppDelegate.shared.onInit()
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        AppDelegate.shared.onLaunch()
    }

    func applicationWillTerminate(_ application: Notification) {
        AppDelegate.shared.onDestroy()
    }
    #endif

}

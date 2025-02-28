import SwiftUI
import Stanza

/// The entry point to the app simply loads the App implementation from SPM module.
@main struct AppMain: App {
    #if canImport(UIKit)
    @UIApplicationDelegateAdaptor(AppMainDelete.self) var appDelegate
    #endif
    @Environment(\.scenePhase) private var scenePhase

    var body: some Scene {
        WindowGroup {
            StanzaRootView()
        }
        .onChange(of: scenePhase) { oldPhase, newPhase in
            switch newPhase {
            case .active:
                StanzaAppDelegate.shared.onResume(appDelegate.application!)
            case .inactive:
                StanzaAppDelegate.shared.onPause(appDelegate.application!)
            case .background:
                StanzaAppDelegate.shared.onStop(appDelegate.application!)
            @unknown default:
                print("unknown app phase: \(newPhase)")
            }
        }
    }
}

#if canImport(UIKit)
class AppMainDelete: UIResponder, UIApplicationDelegate {
    unowned var application: UIApplication? = nil

    func application(_ application: UIApplication, didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey : Any]?) -> Bool {
        self.application = application
        StanzaAppDelegate.shared.onStart(application)
        return true
    }

    func applicationWillTerminate(_ application: UIApplication) {
        StanzaAppDelegate.shared.onDestroy(application)
    }

    func applicationDidReceiveMemoryWarning(_ application: UIApplication) {
        StanzaAppDelegate.shared.onLowMemory(application)
    }
}
#endif

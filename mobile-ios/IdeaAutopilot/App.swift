// iOS twin of mobile/app/src/main/java/.../MainActivity.java: a WKWebView shell around the SAME page
// (mobile/app/src/main/assets/index.html, bundled as the "assets" folder). All logic lives in the page.
import UIKit
import WebKit

@main
final class AppDelegate: UIResponder, UIApplicationDelegate {
    var window: UIWindow?

    func application(_ application: UIApplication, didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]?) -> Bool {
        window = UIWindow(frame: UIScreen.main.bounds)
        window?.rootViewController = WebController()
        window?.makeKeyAndVisible()
        return true
    }
}

final class WebController: UIViewController, WKNavigationDelegate, WKUIDelegate {
    private var web: WKWebView!
    private let bg = UIColor(red: 0x0f / 255.0, green: 0x12 / 255.0, blue: 0x18 / 255.0, alpha: 1)

    override var preferredStatusBarStyle: UIStatusBarStyle { .lightContent }

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = bg
        let cfg = WKWebViewConfiguration()
        // IAApp.fontScale(): the phone's text size, applied by the page to everything at once (ISSUE-104 on Android)
        let scale = UIFontMetrics.default.scaledValue(for: 17) / 17
        let bridge = "window.IAApp = { fontScale: function () { return \(scale); } };"
        cfg.userContentController.addUserScript(WKUserScript(source: bridge, injectionTime: .atDocumentStart, forMainFrameOnly: true))
        web = WKWebView(frame: .zero, configuration: cfg)
        web.isOpaque = false
        web.backgroundColor = bg
        web.scrollView.backgroundColor = bg
        web.navigationDelegate = self
        web.uiDelegate = self
        web.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(web)
        // keep the page clear of the notch and the home bar, like the Android shell pads to the system bars
        let g = view.safeAreaLayoutGuide
        NSLayoutConstraint.activate([
            web.topAnchor.constraint(equalTo: g.topAnchor),
            web.bottomAnchor.constraint(equalTo: g.bottomAnchor),
            web.leadingAnchor.constraint(equalTo: g.leadingAnchor),
            web.trailingAnchor.constraint(equalTo: g.trailingAnchor),
        ])
        guard let index = Bundle.main.url(forResource: "index", withExtension: "html", subdirectory: "assets") else {
            fatalError("assets/index.html is missing from the app bundle (see mobile-ios/README.md)")
        }
        web.loadFileURL(index, allowingReadAccessTo: index.deletingLastPathComponent())
    }

    // links in reports open in Safari, not inside the app
    func webView(_ webView: WKWebView, decidePolicyFor action: WKNavigationAction, decisionHandler: @escaping (WKNavigationActionPolicy) -> Void) {
        if let url = action.request.url, url.scheme != "file", action.navigationType == .linkActivated || action.targetFrame == nil {
            UIApplication.shared.open(url)
            decisionHandler(.cancel)
            return
        }
        decisionHandler(.allow)
    }

    func webView(_ webView: WKWebView, createWebViewWith configuration: WKWebViewConfiguration, for action: WKNavigationAction, windowFeatures: WKWindowFeatures) -> WKWebView? {
        if let url = action.request.url { UIApplication.shared.open(url) }
        return nil
    }

    // WKWebView shows no dialogs by itself: without these, confirm() is always "cancel" and prompt() always empty
    func webView(_ webView: WKWebView, runJavaScriptAlertPanelWithMessage message: String, initiatedByFrame frame: WKFrameInfo, completionHandler: @escaping () -> Void) {
        let a = UIAlertController(title: nil, message: message, preferredStyle: .alert)
        a.addAction(UIAlertAction(title: "OK", style: .default) { _ in completionHandler() })
        present(a, animated: true)
    }

    func webView(_ webView: WKWebView, runJavaScriptConfirmPanelWithMessage message: String, initiatedByFrame frame: WKFrameInfo, completionHandler: @escaping (Bool) -> Void) {
        let a = UIAlertController(title: nil, message: message, preferredStyle: .alert)
        a.addAction(UIAlertAction(title: "ยกเลิก", style: .cancel) { _ in completionHandler(false) })
        a.addAction(UIAlertAction(title: "ตกลง", style: .default) { _ in completionHandler(true) })
        present(a, animated: true)
    }

    func webView(_ webView: WKWebView, runJavaScriptTextInputPanelWithPrompt prompt: String, defaultText: String?, initiatedByFrame frame: WKFrameInfo, completionHandler: @escaping (String?) -> Void) {
        let a = UIAlertController(title: nil, message: prompt, preferredStyle: .alert)
        a.addTextField { $0.text = defaultText }
        a.addAction(UIAlertAction(title: "ยกเลิก", style: .cancel) { _ in completionHandler(nil) })
        a.addAction(UIAlertAction(title: "ตกลง", style: .default) { _ in completionHandler(a.textFields?.first?.text) })
        present(a, animated: true)
    }
}

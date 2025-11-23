// ViewController.mm - Native NSView-based CEF rendering with Rust/wgpu
//
// Architecture:
//   NSWindow
//   ├── MTKView (wgpu surface) - Rust renders UI here
//   └── NSView (CEF browser) - macOS composites CEF on top at viewport rect
//
// CEF renders to its own native NSView. macOS composites it over the wgpu surface.
// This avoids the OSR pixel copy overhead and provides native scrolling/input.

#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Weverything"
#import "include/cef_app.h"
#import "include/cef_browser.h"
#import "include/cef_client.h"
#pragma clang diagnostic pop

#import <Cocoa/Cocoa.h>
#import <Metal/Metal.h>
#import <QuartzCore/CAMetalLayer.h>
#import <CoreVideo/CoreVideo.h>
#import "ViewController.h"
#import "../rune_ffi.h"

@class RustRenderView;

#pragma mark - CEF Handler for Native View

// Forward declaration for navigation callback
@class ViewController;
static ViewController* g_viewController = nil;

// Forward declare syncCefViewPosition for use in displayLinkCallback
@interface ViewController (CefSync)
- (void)syncCefViewPosition;
@end

class CefNativeHandler : public CefClient,
                         public CefLifeSpanHandler,
                         public CefDisplayHandler,
                         public CefLoadHandler,
                         public CefRequestHandler,
                         public CefContextMenuHandler {
public:
    CefRefPtr<CefBrowser> browser;
    NSView* __weak parentView;

    CefNativeHandler(NSView* parent) : parentView(parent) {}

    virtual CefRefPtr<CefLifeSpanHandler> GetLifeSpanHandler() override { return this; }
    virtual CefRefPtr<CefDisplayHandler> GetDisplayHandler() override { return this; }
    virtual CefRefPtr<CefLoadHandler> GetLoadHandler() override { return this; }
    virtual CefRefPtr<CefRequestHandler> GetRequestHandler() override { return this; }
    virtual CefRefPtr<CefContextMenuHandler> GetContextMenuHandler() override { return this; }

    virtual void OnAfterCreated(CefRefPtr<CefBrowser> b) override {
        browser = b;
        NSLog(@"CEF browser created (native view mode)");
    }

    virtual void OnTitleChange(CefRefPtr<CefBrowser> browser, const CefString& title) override {
        NSString* nsTitle = [NSString stringWithUTF8String:title.ToString().c_str()];
        NSLog(@"CEF title changed: %@", nsTitle);
    }

    // CefDisplayHandler - URL changed
    virtual void OnAddressChange(CefRefPtr<CefBrowser> browser, CefRefPtr<CefFrame> frame, const CefString& url) override {
        if (frame->IsMain()) {
            std::string urlStr = url.ToString();
            bool canGoBack = browser->CanGoBack();
            bool canGoForward = browser->CanGoForward();

            // Update Rust navigation state
            rune_ffi_update_navigation_state(urlStr.c_str(), canGoBack, canGoForward, false);

            // Update the address bar to show the new URL
            rune_ffi_set_address_bar_url(urlStr.c_str());

            NSLog(@"CEF URL changed: %s (back=%d, forward=%d)", urlStr.c_str(), canGoBack, canGoForward);
        }
    }

    // CefLoadHandler - loading state changed
    virtual void OnLoadingStateChange(CefRefPtr<CefBrowser> browser, bool isLoading, bool canGoBack, bool canGoForward) override {
        // Get current URL
        CefRefPtr<CefFrame> frame = browser->GetMainFrame();
        std::string url = frame ? frame->GetURL().ToString() : "";

        // Update Rust navigation state
        rune_ffi_update_navigation_state(url.c_str(), canGoBack, canGoForward, isLoading);

        NSLog(@"CEF loading state: loading=%d back=%d forward=%d", isLoading, canGoBack, canGoForward);
    }

    // CefLoadHandler - load started
    virtual void OnLoadStart(CefRefPtr<CefBrowser> browser, CefRefPtr<CefFrame> frame, TransitionType transition_type) override {
        if (frame->IsMain()) {
            NSLog(@"CEF load started");
        }
    }

    // CefLoadHandler - load ended
    virtual void OnLoadEnd(CefRefPtr<CefBrowser> browser, CefRefPtr<CefFrame> frame, int httpStatusCode) override {
        if (frame->IsMain()) {
            NSLog(@"CEF load ended: status=%d", httpStatusCode);
        }
    }

    // CefLifeSpanHandler - Prevent pop-ups, load in same window instead
    virtual bool OnBeforePopup(CefRefPtr<CefBrowser> browser,
                               CefRefPtr<CefFrame> frame,
                               int popup_id,
                               const CefString& target_url,
                               const CefString& target_frame_name,
                               WindowOpenDisposition target_disposition,
                               bool user_gesture,
                               const CefPopupFeatures& popupFeatures,
                               CefWindowInfo& windowInfo,
                               CefRefPtr<CefClient>& client,
                               CefBrowserSettings& settings,
                               CefRefPtr<CefDictionaryValue>& extra_info,
                               bool* no_javascript_access) override {
        // Load the URL in the current browser instead of opening a popup
        std::string url = target_url.ToString();
        NSLog(@"Popup blocked, loading in same window: %s", url.c_str());

        // Navigate the main frame to the popup URL
        CefRefPtr<CefFrame> mainFrame = browser->GetMainFrame();
        if (mainFrame) {
            mainFrame->LoadURL(target_url);
        }

        // Return true to cancel the popup
        return true;
    }

    // CefRequestHandler - Handle window.open() and target="_blank" links
    virtual bool OnOpenURLFromTab(CefRefPtr<CefBrowser> browser,
                                  CefRefPtr<CefFrame> frame,
                                  const CefString& target_url,
                                  WindowOpenDisposition target_disposition,
                                  bool user_gesture) override {
        // For new tab/window requests, load in the same window
        if (target_disposition != CEF_WOD_CURRENT_TAB) {
            std::string url = target_url.ToString();
            NSLog(@"New tab/window request redirected to same window: %s", url.c_str());

            CefRefPtr<CefFrame> mainFrame = browser->GetMainFrame();
            if (mainFrame) {
                mainFrame->LoadURL(target_url);
            }

            // Return true to cancel the default behavior
            return true;
        }

        // Allow normal navigation in current tab
        return false;
    }

    // CefContextMenuHandler - Handle right-click context menus
    virtual void OnBeforeContextMenu(CefRefPtr<CefBrowser> browser,
                                     CefRefPtr<CefFrame> frame,
                                     CefRefPtr<CefContextMenuParams> params,
                                     CefRefPtr<CefMenuModel> model) override {
        // Clear the context menu to disable it and prevent crashes
        // This suppresses the right-click menu entirely
        model->Clear();
    }

    virtual bool OnContextMenuCommand(CefRefPtr<CefBrowser> browser,
                                      CefRefPtr<CefFrame> frame,
                                      CefRefPtr<CefContextMenuParams> params,
                                      int command_id,
                                      cef_event_flags_t event_flags) override {
        // No custom commands since we cleared the menu
        return false;
    }

private:
    IMPLEMENT_REFCOUNTING(CefNativeHandler);
};

#pragma mark - Rust Render View (wgpu surface)

@interface RustRenderView : NSView
@property (nonatomic) float scaleFactor;
@property (nonatomic, weak) id resizeDelegate;
@end

@protocol RustRenderViewResizeDelegate
- (void)renderViewDidResize;
- (void)rustRendererInitialized;
@end

@implementation RustRenderView {
    CAMetalLayer* _metalLayer;
    CVDisplayLinkRef _displayLink;
    BOOL _initialized;
    NSTrackingArea* _trackingArea;
}

- (instancetype)initWithFrame:(NSRect)frameRect {
    self = [super initWithFrame:frameRect];
    if (self) {
        self.wantsLayer = YES;
        self.layerContentsRedrawPolicy = NSViewLayerContentsRedrawDuringViewResize;

        _metalLayer = [CAMetalLayer layer];
        _metalLayer.device = MTLCreateSystemDefaultDevice();
        _metalLayer.pixelFormat = MTLPixelFormatBGRA8Unorm;
        _metalLayer.framebufferOnly = NO;
        self.layer = _metalLayer;
        _initialized = NO;

        [self updateTrackingAreas];
    }
    return self;
}

- (void)updateTrackingAreas {
    if (_trackingArea) {
        [self removeTrackingArea:_trackingArea];
    }

    NSTrackingAreaOptions options = NSTrackingMouseMoved | NSTrackingMouseEnteredAndExited |
                                    NSTrackingActiveInKeyWindow | NSTrackingInVisibleRect;
    _trackingArea = [[NSTrackingArea alloc] initWithRect:self.bounds
                                                 options:options
                                                   owner:self
                                                userInfo:nil];
    [self addTrackingArea:_trackingArea];
}

- (void)viewDidMoveToWindow {
    [super viewDidMoveToWindow];

    if (self.window && !_initialized) {
        _scaleFactor = self.window.backingScaleFactor;
        CGSize size = self.bounds.size;
        uint32_t width = (uint32_t)(size.width * _scaleFactor);
        uint32_t height = (uint32_t)(size.height * _scaleFactor);

        _metalLayer.contentsScale = _scaleFactor;
        _metalLayer.drawableSize = CGSizeMake(width, height);

        const char *packagePath = getenv("RUNE_PACKAGE_PATH");
        const char *pathArg = (packagePath && packagePath[0] != 0) ? packagePath : NULL;

        if (rune_ffi_init(width, height, _scaleFactor, (__bridge void*)_metalLayer, pathArg)) {
            NSLog(@"Rust renderer initialized: %dx%d scale=%.1f", width, height, _scaleFactor);
            _initialized = YES;

            CVDisplayLinkCreateWithActiveCGDisplays(&_displayLink);
            CVDisplayLinkSetOutputCallback(_displayLink, &displayLinkCallback, (__bridge void*)self);
            CVDisplayLinkStart(_displayLink);

            if ([self.resizeDelegate respondsToSelector:@selector(rustRendererInitialized)]) {
                [self.resizeDelegate rustRendererInitialized];
            }
        } else {
            NSLog(@"Failed to initialize Rust renderer");
        }
    }
}

static CVReturn displayLinkCallback(CVDisplayLinkRef displayLink,
                                    const CVTimeStamp* now,
                                    const CVTimeStamp* outputTime,
                                    CVOptionFlags flagsIn,
                                    CVOptionFlags* flagsOut,
                                    void* context) {
    dispatch_async(dispatch_get_main_queue(), ^{
        @autoreleasepool {
            // Update toolbar loading state and spinner animation before render
            rune_ffi_update_toolbar_loading();

            rune_ffi_render();

            // Sync CEF view position immediately after render
            // This ensures layout changes (like sidebar toggle) are applied instantly
            if (g_viewController) {
                [g_viewController syncCefViewPosition];
            }
        }
    });
    return kCVReturnSuccess;
}

- (void)setFrameSize:(NSSize)newSize {
    [super setFrameSize:newSize];

    if (_initialized && newSize.width > 0 && newSize.height > 0) {
        _scaleFactor = self.window.backingScaleFactor;
        uint32_t width = (uint32_t)(newSize.width * _scaleFactor);
        uint32_t height = (uint32_t)(newSize.height * _scaleFactor);

        _metalLayer.drawableSize = CGSizeMake(width, height);
        rune_ffi_resize(width, height);

        if ([self.resizeDelegate respondsToSelector:@selector(renderViewDidResize)]) {
            [self.resizeDelegate renderViewDidResize];
        }
    }
}

- (void)dealloc {
    if (_displayLink) {
        CVDisplayLinkStop(_displayLink);
        CVDisplayLinkRelease(_displayLink);
    }
    rune_ffi_shutdown();
}

- (BOOL)acceptsFirstResponder { return YES; }

- (void)keyDown:(NSEvent *)event {
    if (!_initialized) { [super keyDown:event]; return; }
    rune_ffi_key_event(event.keyCode, true);
    NSString *chars = [event characters];
    if (chars.length > 0) {
        const char *utf8 = [chars UTF8String];
        if (utf8 && utf8[0] != 0) rune_ffi_text_input(utf8);
    }
}

- (void)keyUp:(NSEvent *)event {
    if (!_initialized) { [super keyUp:event]; return; }
    rune_ffi_key_event(event.keyCode, false);
}

- (void)mouseDown:(NSEvent *)event {
    [self.window makeFirstResponder:self];
    NSPoint p = [self flipPoint:[self convertPoint:[event locationInWindow] fromView:nil]];
    rune_ffi_mouse_click(p.x * _scaleFactor, p.y * _scaleFactor, true);
}

- (void)mouseUp:(NSEvent *)event {
    NSPoint p = [self flipPoint:[self convertPoint:[event locationInWindow] fromView:nil]];
    rune_ffi_mouse_click(p.x * _scaleFactor, p.y * _scaleFactor, false);
}

- (void)mouseMoved:(NSEvent *)event {
    NSPoint p = [self flipPoint:[self convertPoint:[event locationInWindow] fromView:nil]];
    rune_ffi_mouse_move(p.x * _scaleFactor, p.y * _scaleFactor);
}

- (void)mouseDragged:(NSEvent *)event {
    NSPoint p = [self flipPoint:[self convertPoint:[event locationInWindow] fromView:nil]];
    rune_ffi_mouse_move(p.x * _scaleFactor, p.y * _scaleFactor);
}

- (NSPoint)flipPoint:(NSPoint)p {
    return NSMakePoint(p.x, self.bounds.size.height - p.y);
}

@end

#pragma mark - CEF Browser View (native NSView)

@interface CefBrowserView : NSView
@property (nonatomic) CefRefPtr<CefBrowser> browser;
@property (nonatomic) float scaleFactor;
@end

@implementation CefBrowserView

- (instancetype)initWithFrame:(NSRect)frameRect {
    self = [super initWithFrame:frameRect];
    if (self) {
        self.wantsLayer = YES;
        self.layerContentsRedrawPolicy = NSViewLayerContentsRedrawOnSetNeedsDisplay;
        self.layer.backgroundColor = [[NSColor clearColor] CGColor];

        // Enable layer optimizations for smooth scrolling
        self.layer.drawsAsynchronously = YES;

        _scaleFactor = 2.0; // Default, will be updated
    }
    return self;
}

- (void)viewDidMoveToWindow {
    [super viewDidMoveToWindow];
    if (self.window) {
        _scaleFactor = self.window.backingScaleFactor;
        self.layer.contentsScale = _scaleFactor;
    }
}

- (BOOL)acceptsFirstResponder { return YES; }
- (BOOL)canBecomeKeyView { return YES; }
- (BOOL)isFlipped { return YES; } // Match CEF's coordinate system

// Enable smooth trackpad scrolling gestures
- (BOOL)wantsScrollEventsForSwipeTrackingOnAxis:(NSEventGestureAxis)axis {
    return YES;
}

// Don't interfere with scroll events - let them pass through naturally to CEF's child view
// CEF creates its own internal view hierarchy that handles scrolling

@end

#pragma mark - ViewController

@interface ViewController() <RustRenderViewResizeDelegate>
@property (nonatomic, strong) RustRenderView* renderView;
@property (nonatomic, strong) CefBrowserView* cefView;
@property (nonatomic, strong) NSTimer* navigationTimer;
@end

@implementation ViewController {
    CefRefPtr<CefNativeHandler> _cefHandler;
    BOOL _cefBrowserCreated;
    CGRect _lastWebViewRect;
}

#pragma mark - Navigation Command Processing

- (void)processNavigationCommands {
    // Process all pending navigation commands
    while (rune_ffi_has_navigation_command()) {
        RuneNavigationCommand cmd = rune_ffi_pop_navigation_command();

        switch (cmd.command_type) {
            case RUNE_NAV_LOAD_URL:
                if (cmd.url) {
                    NSString* urlString = [NSString stringWithUTF8String:cmd.url];
                    [self navigateToURL:urlString];
                    rune_ffi_free_string(cmd.url);
                }
                break;

            case RUNE_NAV_GO_BACK:
                [self navigateBack];
                break;

            case RUNE_NAV_GO_FORWARD:
                [self navigateForward];
                break;

            case RUNE_NAV_RELOAD:
                [self navigateReload];
                break;

            case RUNE_NAV_STOP:
                [self navigateStop];
                break;

            case RUNE_NAV_NONE:
            default:
                break;
        }
    }
}

- (void)navigateToURL:(NSString*)urlString {
    if (!_cefHandler || !_cefHandler->browser) {
        NSLog(@"Navigation: browser not ready, URL=%@", urlString);
        return;
    }

    // Check render target
    uint32_t renderTarget = rune_ffi_get_render_target([urlString UTF8String]);

    if (renderTarget == RUNE_RENDER_IR) {
        // TODO: Switch to IR rendering mode
        NSLog(@"Navigation: IR rendering requested for %@ (not yet implemented)", urlString);
        // For now, still load in CEF but log the intent
    }

    NSLog(@"Navigation: loading URL %@", urlString);

    CefRefPtr<CefFrame> frame = _cefHandler->browser->GetMainFrame();
    if (frame) {
        frame->LoadURL([urlString UTF8String]);
    }
}

- (void)navigateBack {
    if (_cefHandler && _cefHandler->browser && _cefHandler->browser->CanGoBack()) {
        NSLog(@"Navigation: go back");
        _cefHandler->browser->GoBack();
    } else {
        NSLog(@"Navigation: cannot go back");
    }
}

- (void)navigateForward {
    if (_cefHandler && _cefHandler->browser && _cefHandler->browser->CanGoForward()) {
        NSLog(@"Navigation: go forward");
        _cefHandler->browser->GoForward();
    } else {
        NSLog(@"Navigation: cannot go forward");
    }
}

- (void)navigateReload {
    if (_cefHandler && _cefHandler->browser) {
        NSLog(@"Navigation: reload");
        _cefHandler->browser->Reload();
    }
}

- (void)navigateStop {
    if (_cefHandler && _cefHandler->browser) {
        NSLog(@"Navigation: stop loading");
        _cefHandler->browser->StopLoad();
    }
}

- (void)viewDidLoad {
    [super viewDidLoad];

    // Store reference for CEF callbacks
    g_viewController = self;

    // Create wgpu render view (bottom layer)
    _renderView = [[RustRenderView alloc] initWithFrame:self.view.bounds];
    _renderView.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
    _renderView.resizeDelegate = self;
    [self.view addSubview:_renderView];

    // Create CEF browser view (top layer, positioned at viewport rect)
    _cefView = [[CefBrowserView alloc] initWithFrame:NSZeroRect];
    _cefView.hidden = YES; // Hidden until we know the viewport rect
    [self.view addSubview:_cefView positioned:NSWindowAbove relativeTo:_renderView];

    _cefHandler = new CefNativeHandler(_cefView);
    _cefBrowserCreated = NO;
    _lastWebViewRect = CGRectZero;

    // Start navigation command polling timer
    _navigationTimer = [NSTimer scheduledTimerWithTimeInterval:0.05  // 50ms = 20Hz polling
                                                        target:self
                                                      selector:@selector(processNavigationCommands)
                                                      userInfo:nil
                                                       repeats:YES];
}

- (void)rustRendererInitialized {
    NSLog(@"Rust renderer initialized, waiting for layout...");

    // Render a few frames to compute layout
    rune_ffi_render();
    rune_ffi_render();
    rune_ffi_render();

    // Try to get the WebView rect from layout
    [self createCefBrowserWhenLayoutReady:0];
}

- (void)createCefBrowserWhenLayoutReady:(int)attempt {
    float x = 0, y = 0, w = 0, h = 0;

    // Check if layout-computed rect is available
    BOOL hasLayout = rune_ffi_get_webview_rect(&x, &y, &w, &h) && w > 0 && h > 0;

    if (hasLayout) {
        NSLog(@"Got WebView rect from layout: (%.1f, %.1f) %.0fx%.0f [logical]", x, y, w, h);
        _lastWebViewRect = CGRectMake(x, y, w, h);
        [self createCefBrowserAtRect:_lastWebViewRect];
    } else if (attempt < 10) {
        NSLog(@"Layout not ready (attempt %d), rendering and retrying...", attempt);
        rune_ffi_render();
        dispatch_after(dispatch_time(DISPATCH_TIME_NOW, (int64_t)(0.05 * NSEC_PER_SEC)), dispatch_get_main_queue(), ^{
            [self createCefBrowserWhenLayoutReady:attempt + 1];
        });
    } else {
        // Fall back to default size
        uint32_t sw = 0, sh = 0;
        rune_ffi_get_webview_size(&sw, &sh);
        NSLog(@"Layout timeout, using spec size: %dx%d", sw, sh);
        _lastWebViewRect = CGRectMake(100, 100, sw > 0 ? sw : 800, sh > 0 ? sh : 600);
        [self createCefBrowserAtRect:_lastWebViewRect];
    }
}

- (void)createCefBrowserAtRect:(CGRect)rect {
    if (_cefBrowserCreated) return;
    _cefBrowserCreated = YES;

    // Get URL from IR package
    char* urlCStr = rune_ffi_get_webview_url();
    NSString* url = urlCStr ? [NSString stringWithUTF8String:urlCStr] : @"https://www.google.com";
    if (urlCStr) rune_ffi_free_string(urlCStr);

    // Position the CEF view at the viewport rect
    // Convert from logical coords to view coords (Y is flipped)
    float scale = _renderView.scaleFactor;
    CGFloat viewHeight = self.view.bounds.size.height;

    // rect is in logical pixels from top-left; NSView coords are from bottom-left
    NSRect frame = NSMakeRect(
        rect.origin.x,
        viewHeight - rect.origin.y - rect.size.height,
        rect.size.width,
        rect.size.height
    );

    NSLog(@"Positioning CEF view at: (%.1f, %.1f) %.0fx%.0f [view coords]",
          frame.origin.x, frame.origin.y, frame.size.width, frame.size.height);

    _cefView.frame = frame;
    _cefView.hidden = NO;

    // Create CEF browser in the native view
    NSLog(@"Creating native CEF browser: URL=%@ size=%.0fx%.0f", url, rect.size.width, rect.size.height);

    CefWindowInfo windowInfo;
    // Use the CEF view's native window handle
    NSView* cefNsView = _cefView;
    windowInfo.SetAsChild((__bridge CefWindowHandle)cefNsView,
                          CefRect(0, 0, (int)rect.size.width, (int)rect.size.height));

    CefBrowserSettings browserSettings;
    // Enable smooth scrolling and hardware acceleration
    browserSettings.windowless_frame_rate = 60;

    CefBrowserHost::CreateBrowser(windowInfo, _cefHandler, [url UTF8String], browserSettings, nullptr, nullptr);

    // Update the CEF view's scale factor
    _cefView.scaleFactor = _renderView.scaleFactor;

    // Register the native CEF view with the Rust side for hit testing
    rune_ffi_set_cef_view((__bridge void*)_cefView);

    // Note: Layout sync now happens in displayLinkCallback after each render
    // This ensures immediate response to layout changes (sidebar toggle, etc.)
}

- (void)syncCefViewPosition {
    float x = 0, y = 0, w = 0, h = 0;

    if (!rune_ffi_get_webview_rect(&x, &y, &w, &h) || w <= 0 || h <= 0) {
        return;
    }

    CGRect newRect = CGRectMake(x, y, w, h);

    // Only update if rect changed (avoid unnecessary work on every frame)
    if (CGRectEqualToRect(newRect, _lastWebViewRect)) {
        return;
    }

    _lastWebViewRect = newRect;

    // Convert to NSView coordinates (flip Y)
    CGFloat viewHeight = self.view.bounds.size.height;
    NSRect frame = NSMakeRect(
        x,
        viewHeight - y - h,
        w,
        h
    );

    // Update the CEF view frame immediately
    _cefView.frame = frame;

    // Notify CEF browser of the resize
    if (_cefHandler && _cefHandler->browser && _cefHandler->browser.get()) {
        CefRefPtr<CefBrowserHost> host = _cefHandler->browser->GetHost();
        if (host) {
            host->NotifyMoveOrResizeStarted();
            host->WasResized();
        }
    }

    // Update Rust side
    rune_ffi_position_cef_view(x, y, w, h);
}

- (void)renderViewDidResize {
    // Sync CEF view position immediately after window resize
    // The display link callback also syncs on every frame for additional safety
    [self syncCefViewPosition];
}

- (void)dealloc {
    [_navigationTimer invalidate];
    _navigationTimer = nil;
    g_viewController = nil;
}

@end

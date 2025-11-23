// ViewController.mm - CEF browser with Rust/wgpu rendering
// CEF pixels are uploaded to Rust for GPU texture rendering

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

#pragma mark - CEF Handler

class CefHandler : public CefClient, public CefRenderHandler, public CefLifeSpanHandler {
public:
    CefRefPtr<CefBrowser> cefBrowser;
    RustRenderView* __weak view;
    CGSize webviewSize;
    float scaleFactor;

    CefHandler(RustRenderView* renderView) : view(renderView), webviewSize(CGSizeMake(800, 600)), scaleFactor(2.0) {}

    virtual CefRefPtr<CefRenderHandler> GetRenderHandler() override { return this; }
    virtual CefRefPtr<CefLifeSpanHandler> GetLifeSpanHandler() override { return this; }
    virtual void OnAfterCreated(CefRefPtr<CefBrowser> browser) override { this->cefBrowser = browser; }

    virtual void GetViewRect(CefRefPtr<CefBrowser> browser, CefRect& rect) override {
        // Return size in logical pixels - CEF will multiply by scale factor
        rect.Set(0, 0, (int)webviewSize.width, (int)webviewSize.height);
    }

    virtual bool GetScreenInfo(CefRefPtr<CefBrowser> browser, CefScreenInfo& screenInfo) override {
        screenInfo.device_scale_factor = scaleFactor;
        return true;
    }

    virtual void OnPaint(CefRefPtr<CefBrowser> browser,
                         PaintElementType type,
                         const RectList& dirtyRects,
                         const void* buffer,
                         int width,
                         int height) override;

private:
    IMPLEMENT_REFCOUNTING(CefHandler);
};

#pragma mark - Rust Render View

@interface RustRenderView : NSView
@property (nonatomic) float scaleFactor;
@property (nonatomic, weak) id resizeDelegate;
- (void)uploadCefPixels:(const void*)buffer width:(int)width height:(int)height;
@end

@protocol RustRenderViewResizeDelegate
- (void)renderViewDidResize;
- (void)rustRendererInitialized;
- (void)forwardMouseDown:(NSEvent*)event point:(NSPoint)point;
- (void)forwardMouseUp:(NSEvent*)event point:(NSPoint)point;
- (void)forwardMouseMove:(NSEvent*)event point:(NSPoint)point;
- (void)forwardMouseDrag:(NSEvent*)event point:(NSPoint)point;
- (void)forwardKeyDown:(NSEvent*)event;
- (void)forwardKeyUp:(NSEvent*)event;
- (void)forwardScroll:(NSEvent*)event point:(NSPoint)point;
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

        // Set up tracking area for mouse move events
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
            rune_ffi_render();
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

- (void)uploadCefPixels:(const void*)buffer width:(int)width height:(int)height {
    if (!buffer || width <= 0 || height <= 0 || !_initialized) return;

    // CEF is on main thread in windowless mode - call directly
    rune_ffi_upload_webview_pixels(NULL, (const uint8_t*)buffer, (uint32_t)width, (uint32_t)height, (uint32_t)width * 4);
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

    // Also forward key events to the resize delegate (ViewController) so it
    // can route them into CEF when the WebView has focus.
    if ([self.resizeDelegate respondsToSelector:@selector(forwardKeyDown:)]) {
        [(id)self.resizeDelegate forwardKeyDown:event];
    }
}

- (void)keyUp:(NSEvent *)event {
    if (!_initialized) { [super keyUp:event]; return; }
    rune_ffi_key_event(event.keyCode, false);

    if ([self.resizeDelegate respondsToSelector:@selector(forwardKeyUp:)]) {
        [(id)self.resizeDelegate forwardKeyUp:event];
    }
}

- (void)mouseDown:(NSEvent *)event {
    [self.window makeFirstResponder:self];
    NSPoint p = [self flipPoint:[self convertPoint:[event locationInWindow] fromView:nil]];
    rune_ffi_mouse_click(p.x * _scaleFactor, p.y * _scaleFactor, true);

    // Forward to CEF if in WebView
    if ([self.resizeDelegate respondsToSelector:@selector(forwardMouseDown:point:)]) {
        [(id)self.resizeDelegate forwardMouseDown:event point:p];
    }
}

- (void)mouseUp:(NSEvent *)event {
    NSPoint p = [self flipPoint:[self convertPoint:[event locationInWindow] fromView:nil]];
    rune_ffi_mouse_click(p.x * _scaleFactor, p.y * _scaleFactor, false);

    if ([self.resizeDelegate respondsToSelector:@selector(forwardMouseUp:point:)]) {
        [(id)self.resizeDelegate forwardMouseUp:event point:p];
    }
}

- (void)mouseMoved:(NSEvent *)event {
    NSPoint p = [self flipPoint:[self convertPoint:[event locationInWindow] fromView:nil]];
    rune_ffi_mouse_move(p.x * _scaleFactor, p.y * _scaleFactor);

    if ([self.resizeDelegate respondsToSelector:@selector(forwardMouseMove:point:)]) {
        [(id)self.resizeDelegate forwardMouseMove:event point:p];
    }
}

- (void)mouseDragged:(NSEvent *)event {
    NSPoint p = [self flipPoint:[self convertPoint:[event locationInWindow] fromView:nil]];
    rune_ffi_mouse_move(p.x * _scaleFactor, p.y * _scaleFactor);

    if ([self.resizeDelegate respondsToSelector:@selector(forwardMouseDrag:point:)]) {
        [(id)self.resizeDelegate forwardMouseDrag:event point:p];
    }
}

- (void)scrollWheel:(NSEvent *)event {
    // Forward scroll position (in view-local logical coords) to delegate so it
    // can route wheel events into CEF when appropriate.
    NSPoint p = [self flipPoint:[self convertPoint:[event locationInWindow] fromView:nil]];
    if ([self.resizeDelegate respondsToSelector:@selector(forwardScroll:point:)]) {
        [(id)self.resizeDelegate forwardScroll:event point:p];
    } else {
        [super scrollWheel:event];
    }
}

- (NSPoint)flipPoint:(NSPoint)p {
    return NSMakePoint(p.x, self.bounds.size.height - p.y);
}

@end

// CEF OnPaint implementation
void CefHandler::OnPaint(CefRefPtr<CefBrowser> browser,
                         PaintElementType type,
                         const RectList& dirtyRects,
                         const void* buffer,
                         int width,
                         int height) {
    RustRenderView* v = view;
    if (v) {
        [v uploadCefPixels:buffer width:width height:height];
    }
}

#pragma mark - ViewController

@interface ViewController() <RustRenderViewResizeDelegate>
@property (nonatomic, strong) RustRenderView* renderView;
@property (nonatomic, strong) NSTimer* resizeTimer;
@end

@implementation ViewController {
    CefRefPtr<CefHandler> cefHandler;
    CGRect _lastWebViewRect;  // Cached WebView rect in logical coords
    BOOL _mouseInWebView;     // Track if mouse is inside WebView for enter/leave events
    BOOL _cefHasFocus;        // Track whether CEF should receive keyboard events
    BOOL _isDraggingInWebView; // Track if drag started in WebView (continue even outside bounds)
}

- (void)viewDidLoad {
    [super viewDidLoad];

    _renderView = [[RustRenderView alloc] initWithFrame:self.view.bounds];
    _renderView.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
    _renderView.resizeDelegate = self;
    [self.view addSubview:_renderView];

    cefHandler = new CefHandler(_renderView);
    _lastWebViewRect = CGRectZero;
    _mouseInWebView = NO;
    _cefHasFocus = NO;
    _isDraggingInWebView = NO;
}

- (void)rustRendererInitialized {
    cefHandler->scaleFactor = _renderView.scaleFactor;

    // Force multiple renders to ensure layout is computed and rect is stored
    // The first render computes layout, subsequent renders store the transformed rect
    rune_ffi_render();
    rune_ffi_render();
    rune_ffi_render();

    // Now try to get the size - if not available, poll until it is
    [self createCefBrowserWhenLayoutReady:0];
}

- (void)createCefBrowserWhenLayoutReady:(int)attempt {
    float x = 0, y = 0;
    uint32_t w = 0, h = 0;

    // Check if layout-computed size is available
    BOOL hasLayout = rune_ffi_get_webview_position(&x, &y) && rune_ffi_get_webview_size(&w, &h) && w > 0 && h > 0;

    if (hasLayout) {
        // x, y are in logical pixels from FFI (already transformed), w, h are logical pixels
        // No scale conversion needed - all values are in logical coords
        NSLog(@"Got layout-computed WebView size: %dx%d at (%.1f, %.1f) [logical]", w, h, x, y);
        _lastWebViewRect = CGRectMake(x, y, (float)w, (float)h);
        [self createCefBrowserWithSize:CGSizeMake(w, h)];
    } else if (attempt < 10) {
        // Not ready yet, render another frame and try again
        NSLog(@"Layout not ready (attempt %d), rendering and retrying...", attempt);
        rune_ffi_render();
        dispatch_after(dispatch_time(DISPATCH_TIME_NOW, (int64_t)(0.05 * NSEC_PER_SEC)), dispatch_get_main_queue(), ^{
            [self createCefBrowserWhenLayoutReady:attempt + 1];
        });
    } else {
        // Give up and use spec size
        rune_ffi_get_webview_size(&w, &h);
        NSLog(@"Layout timeout, using spec WebView size: %dx%d", w, h);
        [self createCefBrowserWithSize:CGSizeMake(w, h)];
    }
}

- (void)createCefBrowserWithSize:(CGSize)size {
    cefHandler->webviewSize = size;

    // Get URL
    char* urlCStr = rune_ffi_get_webview_url();
    NSString* url = urlCStr ? [NSString stringWithUTF8String:urlCStr] : @"https://www.google.com";
    if (urlCStr) rune_ffi_free_string(urlCStr);

    NSLog(@"Creating CEF browser: %.0fx%.0f URL=%@", size.width, size.height, url);

    // Create browser with correct size
    CefWindowInfo info;
    info.SetAsWindowless([self.view.window windowRef]);
    CefBrowserSettings settings;
    CefBrowserHost::CreateBrowser(info, cefHandler, [url UTF8String], settings, nullptr, nullptr);
}

- (void)syncCefSizeWithLayout {
    float x = 0, y = 0;
    uint32_t w = 0, h = 0;

    if (rune_ffi_get_webview_position(&x, &y) && rune_ffi_get_webview_size(&w, &h) && w > 0 && h > 0) {
        // x, y are already in logical pixels from FFI (transformed coords)
        _lastWebViewRect = CGRectMake(x, y, (float)w, (float)h);

        // Update CEF size if changed
        if ((int)cefHandler->webviewSize.width != (int)w || (int)cefHandler->webviewSize.height != (int)h) {
            NSLog(@"Updating CEF size: %dx%d -> %dx%d", (int)cefHandler->webviewSize.width, (int)cefHandler->webviewSize.height, w, h);
            cefHandler->webviewSize = CGSizeMake(w, h);

            if (cefHandler->cefBrowser && cefHandler->cefBrowser.get()) {
                cefHandler->cefBrowser->GetHost()->WasResized();
                cefHandler->cefBrowser->GetHost()->Invalidate(PET_VIEW);
            }
        }
    }
}

- (void)renderViewDidResize {
    [self.resizeTimer invalidate];
    self.resizeTimer = [NSTimer scheduledTimerWithTimeInterval:0.1
                                                        target:self
                                                      selector:@selector(syncCefSizeWithLayout)
                                                      userInfo:nil
                                                       repeats:NO];
}

#pragma mark - Mouse Event Forwarding to CEF

- (BOOL)isPointInWebView:(NSPoint)point localPoint:(NSPoint*)local {
    if (CGRectIsEmpty(_lastWebViewRect)) {
        NSLog(@"isPointInWebView: rect is empty");
        return NO;
    }

    // Debug: log first click to verify coordinate systems
    static BOOL logged = NO;
    if (!logged) {
        NSLog(@"isPointInWebView: point=(%.1f, %.1f) rect=(%.1f, %.1f, %.1f, %.1f)",
              point.x, point.y,
              _lastWebViewRect.origin.x, _lastWebViewRect.origin.y,
              _lastWebViewRect.size.width, _lastWebViewRect.size.height);
        logged = YES;
    }

    if (CGRectContainsPoint(_lastWebViewRect, point)) {
        if (local) {
            local->x = point.x - _lastWebViewRect.origin.x;
            local->y = point.y - _lastWebViewRect.origin.y;
        }
        return YES;
    }
    return NO;
}

- (int)modifiersForEvent:(NSEvent*)event {
    int m = 0;
    NSUInteger flags = [event modifierFlags];
    if (flags & NSEventModifierFlagControl) m |= EVENTFLAG_CONTROL_DOWN;
    if (flags & NSEventModifierFlagShift) m |= EVENTFLAG_SHIFT_DOWN;
    if (flags & NSEventModifierFlagOption) m |= EVENTFLAG_ALT_DOWN;
    if (flags & NSEventModifierFlagCommand) m |= EVENTFLAG_COMMAND_DOWN;

    switch ([event type]) {
        case NSEventTypeLeftMouseDown:
        case NSEventTypeLeftMouseUp:
        case NSEventTypeLeftMouseDragged:
            m |= EVENTFLAG_LEFT_MOUSE_BUTTON;
            break;
        case NSEventTypeRightMouseDown:
        case NSEventTypeRightMouseUp:
            m |= EVENTFLAG_RIGHT_MOUSE_BUTTON;
            break;
        default:
            break;
    }
    return m;
}

- (void)sendCefMouseClick:(NSPoint)local event:(NSEvent*)event isUp:(BOOL)isUp {
    if (!cefHandler->cefBrowser) return;

    CefMouseEvent me;
    me.x = (int)local.x;
    me.y = (int)local.y;
    me.modifiers = [self modifiersForEvent:event];

    CefBrowserHost::MouseButtonType btn = MBT_LEFT;
    if ([event type] == NSEventTypeRightMouseDown || [event type] == NSEventTypeRightMouseUp) {
        btn = MBT_RIGHT;
    }

    cefHandler->cefBrowser->GetHost()->SendMouseClickEvent(me, btn, isUp, 1);
}

- (void)sendCefMouseMove:(NSPoint)local event:(NSEvent*)event mouseLeave:(BOOL)leave {
    if (!cefHandler->cefBrowser) return;

    CefMouseEvent me;
    me.x = (int)local.x;
    me.y = (int)local.y;
    me.modifiers = [self modifiersForEvent:event];

    cefHandler->cefBrowser->GetHost()->SendMouseMoveEvent(me, leave);
}

#pragma mark - Mouse Event Forwarding from RustRenderView

- (void)forwardMouseDown:(NSEvent*)event point:(NSPoint)point {
    NSPoint local;
    if ([self isPointInWebView:point localPoint:&local]) {
        NSLog(@"forwardMouseDown: view=(%.1f, %.1f) -> cef=(%.1f, %.1f)", point.x, point.y, local.x, local.y);
        [self sendCefMouseClick:local event:event isUp:NO];

        // Mark that we started dragging in WebView - continue forwarding events
        // even if mouse moves outside bounds (for text selection, scrollbar drag, etc.)
        _isDraggingInWebView = YES;

        // Ensure the offscreen CEF browser has keyboard focus so it can
        // show the text caret and receive key events for the active field.
        if (cefHandler && cefHandler->cefBrowser && cefHandler->cefBrowser.get()) {
            cefHandler->cefBrowser->GetHost()->SetFocus(true);
        }
        _cefHasFocus = YES;
    } else {
        _isDraggingInWebView = NO;
        // Click outside the WebView: drop CEF focus so the caret is hidden
        // when interacting with the rest of the Rune UI.
        if (cefHandler && cefHandler->cefBrowser && cefHandler->cefBrowser.get()) {
            cefHandler->cefBrowser->GetHost()->SetFocus(false);
        }
        _cefHasFocus = NO;
    }
}

- (void)forwardKeyDown:(NSEvent*)event {
    if (!_cefHasFocus) {
        return;
    }
    if (!(cefHandler && cefHandler->cefBrowser && cefHandler->cefBrowser.get())) {
        return;
    }

    CefKeyEvent kev;
    memset(&kev, 0, sizeof(kev));
    kev.size = sizeof(kev);
    kev.modifiers = (uint32_t)[self modifiersForEvent:event];
    kev.native_key_code = (int)[event keyCode];
    kev.windows_key_code = kev.native_key_code;
    kev.is_system_key = 0;
    kev.focus_on_editable_field = 1;

    // Raw key down
    kev.type = KEYEVENT_RAWKEYDOWN;
    cefHandler->cefBrowser->GetHost()->SendKeyEvent(kev);

    // Character input for text fields
    NSString* chars = [event characters];
    if (chars.length > 0) {
        unichar ch = [chars characterAtIndex:0];
        kev.type = KEYEVENT_CHAR;
        kev.character = ch;
        kev.unmodified_character = ch;
        cefHandler->cefBrowser->GetHost()->SendKeyEvent(kev);
    }
}

- (void)forwardKeyUp:(NSEvent*)event {
    if (!_cefHasFocus) {
        return;
    }
    if (!(cefHandler && cefHandler->cefBrowser && cefHandler->cefBrowser.get())) {
        return;
    }

    CefKeyEvent kev;
    memset(&kev, 0, sizeof(kev));
    kev.size = sizeof(kev);
    kev.modifiers = (uint32_t)[self modifiersForEvent:event];
    kev.native_key_code = (int)[event keyCode];
    kev.windows_key_code = kev.native_key_code;
    kev.is_system_key = 0;
    kev.focus_on_editable_field = 1;
    kev.type = KEYEVENT_KEYUP;
    cefHandler->cefBrowser->GetHost()->SendKeyEvent(kev);
}

- (void)forwardMouseUp:(NSEvent*)event point:(NSPoint)point {
    NSPoint local;
    // Always send mouse up if we were dragging in WebView, even if mouse is now outside
    if (_isDraggingInWebView) {
        // Calculate local coords relative to WebView even if outside bounds
        local.x = point.x - _lastWebViewRect.origin.x;
        local.y = point.y - _lastWebViewRect.origin.y;
        [self sendCefMouseClick:local event:event isUp:YES];
        _isDraggingInWebView = NO;
    } else if ([self isPointInWebView:point localPoint:&local]) {
        [self sendCefMouseClick:local event:event isUp:YES];
    }
}

- (void)forwardMouseMove:(NSEvent*)event point:(NSPoint)point {
    NSPoint local;
    BOOL inWebView = [self isPointInWebView:point localPoint:&local];

    if (inWebView) {
        if (!_mouseInWebView) {
            // Mouse entered WebView
            _mouseInWebView = YES;
        }
        [self sendCefMouseMove:local event:event mouseLeave:NO];
    } else if (_mouseInWebView) {
        // Mouse left WebView - send leave event
        _mouseInWebView = NO;
        [self sendCefMouseMove:NSMakePoint(0, 0) event:event mouseLeave:YES];
    }
}

- (void)forwardMouseDrag:(NSEvent*)event point:(NSPoint)point {
    NSPoint local;
    // Continue forwarding drag events if we started dragging in WebView,
    // even if mouse has moved outside bounds (for text selection, etc.)
    if (_isDraggingInWebView) {
        // Calculate local coords relative to WebView even if outside bounds
        local.x = point.x - _lastWebViewRect.origin.x;
        local.y = point.y - _lastWebViewRect.origin.y;
        [self sendCefMouseMove:local event:event mouseLeave:NO];
    } else if ([self isPointInWebView:point localPoint:&local]) {
        [self sendCefMouseMove:local event:event mouseLeave:NO];
    }
}

- (void)forwardScroll:(NSEvent*)event point:(NSPoint)point {
    if (!(cefHandler && cefHandler->cefBrowser && cefHandler->cefBrowser.get())) {
        return;
    }

    NSPoint local;
    BOOL inWebView = [self isPointInWebView:point localPoint:&local];

    // Also allow scroll events if we have focus (for momentum scrolling after
    // the cursor might have drifted slightly outside bounds)
    if (!inWebView && !_cefHasFocus) {
        return;
    }

    // If outside WebView but has focus, calculate coords relative to WebView
    if (!inWebView) {
        local.x = point.x - _lastWebViewRect.origin.x;
        local.y = point.y - _lastWebViewRect.origin.y;
    }

    CefMouseEvent me;
    me.x = (int)local.x;
    me.y = (int)local.y;
    me.modifiers = [self modifiersForEvent:event];

    // Map macOS scroll deltas to CEF's expected units.
    // CEF typically expects values similar to wheel "ticks" (e.g. 120 per step).
    // Trackpads report small floating-point deltas, so scale them up.
    CGFloat dx = [event scrollingDeltaX];
    CGFloat dy = [event scrollingDeltaY];

    // Use a smaller scale factor for precise scrolling (trackpad) to make
    // scrolling feel more natural and responsive.
    const CGFloat scale = [event hasPreciseScrollingDeltas] ? 1.0 : 40.0;

    // On macOS with natural scrolling, scrollingDeltaY is positive when
    // content moves up (two-finger swipe up). CEF expects positive deltaY
    // to scroll *down* (content moves down), so invert the sign.
    int deltaX = (int)llround(dx * scale);
    int deltaY = (int)llround(-dy * scale);

    cefHandler->cefBrowser->GetHost()->SendMouseWheelEvent(me, deltaX, deltaY);
}

@end

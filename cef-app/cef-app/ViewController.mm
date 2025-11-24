// ViewController.mm - CEF browser with Rust/wgpu rendering

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
#import "../cef_demo.h"

// Forward declarations
@class RustRenderView;

// CEF Handler - receives browser callbacks and forwards OnPaint to Rust
class CefHandler : public CefClient, public CefRenderHandler, public CefLifeSpanHandler {
public:
    CefRefPtr<CefBrowser> cefBrowser;
    RustRenderView* __weak view;

    CefHandler(RustRenderView* renderView) : view(renderView) {}

    virtual CefRefPtr<CefRenderHandler> GetRenderHandler() override {
        return this;
    }

    virtual CefRefPtr<CefLifeSpanHandler> GetLifeSpanHandler() override {
        return this;
    }

    virtual void OnAfterCreated(CefRefPtr<CefBrowser> browser) override {
        this->cefBrowser = browser;
    }

    virtual void GetViewRect(CefRefPtr<CefBrowser> browser, CefRect& rect) override;
    virtual bool GetScreenInfo(CefRefPtr<CefBrowser> browser, CefScreenInfo& screenInfo) override;
    virtual void OnPaint(CefRefPtr<CefBrowser> browser,
                         PaintElementType type,
                         const RectList& dirtyRects,
                         const void* buffer,
                         int width,
                         int height) override;

private:
    IMPLEMENT_REFCOUNTING(CefHandler);
};

// Custom view with CAMetalLayer for Rust/wgpu rendering
@interface RustRenderView : NSView
@property (nonatomic) CGSize viewSize;
@property (nonatomic) float scaleFactor;
@property (nonatomic, weak) id resizeDelegate;
- (void)onCefPaint:(const void*)buffer width:(int)width height:(int)height;
@end

@protocol RustRenderViewResizeDelegate
- (void)renderViewDidResize;
@end

@implementation RustRenderView {
    CAMetalLayer* _metalLayer;
    CVDisplayLinkRef _displayLink;
    BOOL _initialized;
}

+ (Class)layerClass {
    return [CAMetalLayer class];
}

- (instancetype)initWithFrame:(NSRect)frameRect {
    self = [super initWithFrame:frameRect];
    if (self) {
        [self setupLayer];
    }
    return self;
}

- (instancetype)initWithCoder:(NSCoder *)coder {
    self = [super initWithCoder:coder];
    if (self) {
        [self setupLayer];
    }
    return self;
}

- (void)setupLayer {
    self.wantsLayer = YES;
    self.layerContentsRedrawPolicy = NSViewLayerContentsRedrawDuringViewResize;

    _metalLayer = [CAMetalLayer layer];
    _metalLayer.device = MTLCreateSystemDefaultDevice();
    _metalLayer.pixelFormat = MTLPixelFormatBGRA8Unorm;
    _metalLayer.framebufferOnly = NO;
    self.layer = _metalLayer;

    _initialized = NO;
}

- (void)viewDidMoveToWindow {
    [super viewDidMoveToWindow];

    if (self.window && !_initialized) {
        // Initialize Rust renderer
        _scaleFactor = self.window.backingScaleFactor;
        CGSize size = self.bounds.size;
        uint32_t width = (uint32_t)(size.width * _scaleFactor);
        uint32_t height = (uint32_t)(size.height * _scaleFactor);

        _metalLayer.contentsScale = _scaleFactor;
        _metalLayer.drawableSize = CGSizeMake(width, height);
        _viewSize = size;

        if (cef_demo_init(width, height, _scaleFactor, (__bridge void*)_metalLayer)) {
            NSLog(@"Rust renderer initialized: %dx%d scale=%.1f", width, height, _scaleFactor);
            _initialized = YES;

            // Start display link
            CVDisplayLinkCreateWithActiveCGDisplays(&_displayLink);
            CVDisplayLinkSetOutputCallback(_displayLink, &displayLinkCallback, (__bridge void*)self);
            CVDisplayLinkStart(_displayLink);
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
    @autoreleasepool {
        // Render directly - wgpu/Metal is thread-safe
        cef_demo_render();
    }
    return kCVReturnSuccess;
}

- (void)setFrameSize:(NSSize)newSize {
    [super setFrameSize:newSize];

    if (_initialized && newSize.width > 0 && newSize.height > 0) {
        _scaleFactor = self.window.backingScaleFactor;
        uint32_t width = (uint32_t)(newSize.width * _scaleFactor);
        uint32_t height = (uint32_t)(newSize.height * _scaleFactor);

        _metalLayer.drawableSize = CGSizeMake(width, height);
        _viewSize = newSize;

        cef_demo_resize(width, height);

        // Notify delegate (ViewController) to resize CEF browser
        if ([self.resizeDelegate respondsToSelector:@selector(renderViewDidResize)]) {
            [self.resizeDelegate renderViewDidResize];
        }
    }
}

- (void)onCefPaint:(const void*)buffer width:(int)width height:(int)height {
    if (_initialized && buffer && width > 0 && height > 0) {
        cef_demo_upload_pixels((const uint8_t*)buffer, width, height, width * 4);
    }
}

- (void)dealloc {
    if (_displayLink) {
        CVDisplayLinkStop(_displayLink);
        CVDisplayLinkRelease(_displayLink);
    }
    cef_demo_shutdown();
}

- (BOOL)acceptsFirstResponder {
    return YES;
}

@end

// CefHandler method implementations
void CefHandler::GetViewRect(CefRefPtr<CefBrowser> browser, CefRect& rect) {
    RustRenderView* v = view;
    if (v) {
        float scale = v.scaleFactor;
        CGSize size = v.viewSize;
        rect.Set(0, 0, (int)(size.width), (int)(size.height));
    }
}

bool CefHandler::GetScreenInfo(CefRefPtr<CefBrowser> browser, CefScreenInfo& screenInfo) {
    RustRenderView* v = view;
    if (v) {
        screenInfo.device_scale_factor = v.scaleFactor;
        return true;
    }
    return false;
}

void CefHandler::OnPaint(CefRefPtr<CefBrowser> browser,
                         PaintElementType type,
                         const RectList& dirtyRects,
                         const void* buffer,
                         int width,
                         int height) {
    RustRenderView* v = view;
    if (v) {
        [v onCefPaint:buffer width:width height:height];
    }
}

// ViewController
@interface ViewController() <RustRenderViewResizeDelegate>
@property (nonatomic, strong) RustRenderView* renderView;
@property (nonatomic, strong) NSTimer* resizeDebounceTimer;
@end

@implementation ViewController {
    CefRefPtr<CefHandler> cefHandler;
}

typedef enum MouseEventKind : NSUInteger {
    kUp,
    kDown,
    kMove
} MouseEventKind;

- (void)viewDidLoad {
    [super viewDidLoad];

    // Create render view
    _renderView = [[RustRenderView alloc] initWithFrame:self.view.bounds];
    _renderView.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
    _renderView.resizeDelegate = self;
    [self.view addSubview:_renderView];

    // Create CEF handler
    cefHandler = new CefHandler(_renderView);

    // Create CEF browser (will be ready after context initialization)
    CefWindowInfo cefWindowInfo;
    cefWindowInfo.SetAsWindowless([self.view.window windowRef]);
    CefBrowserSettings cefBrowserSettings;
    CefBrowserHost::CreateBrowser(
        cefWindowInfo,
        cefHandler,
        "about:blank",
        cefBrowserSettings,
        nullptr,
        nullptr);
}

- (int)getModifiersForEvent:(NSEvent*)event {
    int modifiers = 0;

    if ([event modifierFlags] & NSEventModifierFlagControl)
        modifiers |= EVENTFLAG_CONTROL_DOWN;
    if ([event modifierFlags] & NSEventModifierFlagShift)
        modifiers |= EVENTFLAG_SHIFT_DOWN;
    if ([event modifierFlags] & NSEventModifierFlagOption)
        modifiers |= EVENTFLAG_ALT_DOWN;
    if ([event modifierFlags] & NSEventModifierFlagCommand)
        modifiers |= EVENTFLAG_COMMAND_DOWN;
    if ([event modifierFlags] & NSEventModifierFlagCapsLock)
        modifiers |= EVENTFLAG_CAPS_LOCK_ON;

    switch ([event type]) {
        case NSEventTypeLeftMouseDragged:
        case NSEventTypeLeftMouseUp:
        case NSEventTypeLeftMouseDown:
            modifiers |= EVENTFLAG_LEFT_MOUSE_BUTTON;
            break;
        case NSEventTypeRightMouseDragged:
        case NSEventTypeRightMouseUp:
        case NSEventTypeRightMouseDown:
            modifiers |= EVENTFLAG_RIGHT_MOUSE_BUTTON;
            break;
        case NSEventTypeOtherMouseDragged:
        case NSEventTypeOtherMouseUp:
        case NSEventTypeOtherMouseDown:
            modifiers |= EVENTFLAG_MIDDLE_MOUSE_BUTTON;
            break;
        default:
            break;
    }

    return modifiers;
}

- (NSPoint)getClickPointForEvent:(NSEvent*)event {
    NSPoint windowLocal = [event locationInWindow];
    NSPoint contentLocal = [_renderView convertPoint:windowLocal fromView:nil];

    NSPoint point;
    point.x = contentLocal.x;
    point.y = [_renderView frame].size.height - contentLocal.y;  // Flip y.
    return point;
}

- (void)mouseEvent:(MouseEventKind)mouseEventKind
                at:(NSPoint)point
         modifiers:(int)modifiers
{
    CefRefPtr<CefBrowser> browser = cefHandler->cefBrowser;
    if (!browser || !browser.get()) {
        return;
    }

    CefMouseEvent mouseEvent;
    mouseEvent.x = point.x;
    mouseEvent.y = point.y;
    mouseEvent.modifiers = modifiers;

    switch(mouseEventKind){
        case MouseEventKind::kDown:
            browser->GetHost()->SendMouseClickEvent(
                mouseEvent,
                MBT_LEFT,
                false,
                1);
            break;
        case MouseEventKind::kUp:
            browser->GetHost()->SendMouseClickEvent(
                mouseEvent,
                MBT_LEFT,
                true,
                1);
            break;
        case MouseEventKind::kMove:
            browser->GetHost()->SendMouseMoveEvent(mouseEvent, false);
            break;
    }
}

- (void)viewDidLayout {
    [self notifyCefResize];
}

- (void)renderViewDidResize {
    // Immediately notify CEF of size change
    CefRefPtr<CefBrowser> browser = cefHandler->cefBrowser;
    if (browser && browser.get()) {
        browser->GetHost()->WasResized();
    }

    // Debounce the invalidate (repaint request) to avoid overwhelming CEF
    [self.resizeDebounceTimer invalidate];
    self.resizeDebounceTimer = [NSTimer scheduledTimerWithTimeInterval:0.05
                                                                target:self
                                                              selector:@selector(forceRepaint)
                                                              userInfo:nil
                                                               repeats:NO];
}

- (void)forceRepaint {
    CefRefPtr<CefBrowser> browser = cefHandler->cefBrowser;
    if (browser && browser.get()) {
        browser->GetHost()->Invalidate(PET_VIEW);
    }
}

- (void)notifyCefResize {
    CefRefPtr<CefBrowser> browser = cefHandler->cefBrowser;
    if (!browser || !browser.get()) {
        return;
    }
    browser->GetHost()->WasResized();
    browser->GetHost()->Invalidate(PET_VIEW);
}

- (void)mouseDown:(NSEvent *)event {
    [self
        mouseEvent:MouseEventKind::kDown
        at:[self getClickPointForEvent:event]
        modifiers:[self getModifiersForEvent:event]
    ];
}

- (void)mouseMoved:(NSEvent *)event {
    [self
        mouseEvent:MouseEventKind::kMove
        at:[self getClickPointForEvent:event]
        modifiers:[self getModifiersForEvent:event]
    ];
}

- (void)mouseDragged:(NSEvent *)event {
    [self
        mouseEvent:MouseEventKind::kMove
        at:[self getClickPointForEvent:event]
        modifiers:[self getModifiersForEvent:event]
    ];
}

- (void)mouseUp:(NSEvent *)event {
    [self
        mouseEvent:MouseEventKind::kUp
        at:[self getClickPointForEvent:event]
        modifiers:[self getModifiersForEvent:event]
    ];
}

@end

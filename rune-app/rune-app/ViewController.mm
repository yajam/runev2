// ViewController.mm - Rune Scene rendering with Rust/wgpu

#import <Cocoa/Cocoa.h>
#import <Metal/Metal.h>
#import <QuartzCore/CAMetalLayer.h>
#import <CoreVideo/CoreVideo.h>
#import "ViewController.h"
#import "../rune_ffi.h"

// Custom view with CAMetalLayer for Rust/wgpu rendering
@interface RuneRenderView : NSView
@property (nonatomic) CGSize viewSize;
@property (nonatomic) float scaleFactor;
@property (nonatomic, weak) id resizeDelegate;
@end

@protocol RuneRenderViewResizeDelegate
- (void)renderViewDidResize;
@end

@implementation RuneRenderView {
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

        // Get the package path (can be configured via command line or hardcoded)
        NSString* packagePath = nil;
        NSArray* args = [[NSProcessInfo processInfo] arguments];
        for (NSUInteger i = 1; i < args.count; i++) {
            NSString* arg = args[i];
            if ([arg hasPrefix:@"--package="]) {
                packagePath = [arg substringFromIndex:10];
                break;
            }
        }

        const char* pathCStr = packagePath ? [packagePath UTF8String] : NULL;

        if (rune_init(width, height, _scaleFactor, (__bridge void*)_metalLayer, pathCStr)) {
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
        rune_render();
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

        rune_resize(width, height);

        // Notify delegate
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
    rune_shutdown();
}

- (BOOL)acceptsFirstResponder {
    return YES;
}

- (NSPoint)getClickPointForEvent:(NSEvent*)event {
    NSPoint windowLocal = [event locationInWindow];
    NSPoint contentLocal = [self convertPoint:windowLocal fromView:nil];

    NSPoint point;
    point.x = contentLocal.x * _scaleFactor;
    point.y = ([self frame].size.height - contentLocal.y) * _scaleFactor;
    return point;
}

- (void)mouseDown:(NSEvent *)event {
    NSPoint point = [self getClickPointForEvent:event];
    rune_mouse_click(point.x, point.y, true);
}

- (void)mouseUp:(NSEvent *)event {
    NSPoint point = [self getClickPointForEvent:event];
    rune_mouse_click(point.x, point.y, false);
}

- (void)mouseMoved:(NSEvent *)event {
    NSPoint point = [self getClickPointForEvent:event];
    rune_mouse_move(point.x, point.y);
}

- (void)mouseDragged:(NSEvent *)event {
    NSPoint point = [self getClickPointForEvent:event];
    rune_mouse_move(point.x, point.y);
}

- (void)keyDown:(NSEvent *)event {
    rune_key_event([event keyCode], true);
}

- (void)keyUp:(NSEvent *)event {
    rune_key_event([event keyCode], false);
}

@end

// ViewController
@interface ViewController() <RuneRenderViewResizeDelegate>
@property (nonatomic, strong) RuneRenderView* renderView;
@end

@implementation ViewController

- (void)loadView {
    // Create an empty view as the base
    self.view = [[NSView alloc] initWithFrame:NSMakeRect(0, 0, 1280, 720)];
}

- (void)viewDidLoad {
    [super viewDidLoad];

    // Create render view
    _renderView = [[RuneRenderView alloc] initWithFrame:self.view.bounds];
    _renderView.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
    _renderView.resizeDelegate = self;
    [self.view addSubview:_renderView];
}

- (void)renderViewDidResize {
    rune_request_redraw();
}

@end

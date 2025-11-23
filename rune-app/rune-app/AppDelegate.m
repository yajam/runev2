// AppDelegate.m - Rune Scene application delegate

#import "AppDelegate.h"
#import "ViewController.h"

@implementation AppDelegate {
    NSWindow* _window;
    ViewController* _viewController;
}

- (void)applicationDidFinishLaunching:(NSNotification *)notification {
    // Create main window
    NSRect frame = NSMakeRect(100, 100, 1280, 720);
    NSWindowStyleMask style = NSWindowStyleMaskTitled |
                              NSWindowStyleMaskClosable |
                              NSWindowStyleMaskMiniaturizable |
                              NSWindowStyleMaskResizable;

    _window = [[NSWindow alloc] initWithContentRect:frame
                                          styleMask:style
                                            backing:NSBackingStoreBuffered
                                              defer:NO];

    [_window setTitle:@"Rune Scene"];
    [_window setMinSize:NSMakeSize(640, 480)];

    // Create view controller
    _viewController = [[ViewController alloc] init];
    [_window setContentViewController:_viewController];

    // Show window
    [_window makeKeyAndOrderFront:nil];
    [NSApp activateIgnoringOtherApps:YES];
}

- (BOOL)applicationShouldTerminateAfterLastWindowClosed:(NSApplication *)sender {
    return YES;
}

@end
